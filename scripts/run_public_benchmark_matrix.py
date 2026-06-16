#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "datasets>=3,<5",
# ]
# ///
"""Prepare and run the pinned public code-retrieval benchmark matrix."""

from __future__ import annotations

import argparse
from collections import defaultdict
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess
import sys

import eval_code_retrieval
import export_public_retrieval


QUALITY_METRICS = (
    "ndcg_at_10",
    "mrr_at_10",
    "precision_at_5",
    "recall_at_20",
    "no_hit_rate",
    "support_file_spam_rate_at_10",
)
LATENCY_METRICS = (
    "cold_latency_p50_ms",
    "cold_latency_p95_ms",
    "warm_latency_p50_ms",
    "warm_latency_p95_ms",
)
RESOURCE_METRICS = (
    "index_ms",
    "hash_enhancement_ms",
    "neural_enhancement_ms",
    "daemon_startup_ms",
    "neural_model_ready_ms",
    "index_size_bytes",
    "peak_child_rss_bytes",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_revision(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    return completed.stdout.strip()


def benchmark_revision(root: Path, source_commit: str | None) -> str:
    return source_commit or git_revision(root)


def parse_modes(value: str) -> list[str]:
    modes = [mode.strip() for mode in value.split(",") if mode.strip()]
    allowed = {"lexical", "hash", "hybrid", "neural"}
    unknown = sorted(set(modes) - allowed)
    if unknown:
        raise ValueError(f"unknown retrieval modes: {', '.join(unknown)}")
    if not modes:
        raise ValueError("at least one retrieval mode is required")
    return list(dict.fromkeys(modes))


def build_binary(root: Path, modes: list[str]) -> None:
    command = ["cargo", "build", "--release", "--locked", "--bin", "ig"]
    if "neural" not in modes:
        command.insert(-2, "--no-default-features")
    subprocess.run(command, cwd=root, check=True)


def export_datasets(
    manifest: dict,
    profile: str,
    output: Path,
    tasks: list[str],
) -> list[dict]:
    exported = []
    task_options = manifest["profiles"][profile].get("task_options", {})
    for task in tasks:
        options = task_options.get(task, {})
        exported.append(
            export_public_retrieval.export_task(
                task,
                manifest["tasks"][task],
                output,
                sample_queries=options.get("sample_queries"),
                sample_corpus=options.get("sample_corpus"),
                seed=options.get("seed", 20260615),
                query_partition=options.get("query_partition"),
            )
        )
    validate_profile_query_count(manifest, profile, exported)
    return exported


def validate_profile_query_count(
    manifest: dict, profile: str, provenances: list[dict]
) -> None:
    minimum = manifest["profiles"][profile]["minimum_queries"]
    query_count = sum(item["counts"]["queries"] for item in provenances)
    if query_count < minimum:
        raise ValueError(f"profile {profile} has {query_count} queries, below {minimum}")


def run_evaluation(
    root: Path,
    dataset: Path,
    binary: Path,
    mode: str,
    output: Path,
) -> dict:
    command = [
        sys.executable,
        str(root / "scripts" / "eval_code_retrieval.py"),
        "--dataset",
        str(dataset),
        "--binary",
        str(binary),
        "--mode",
        mode,
        "--output",
        str(output),
    ]
    subprocess.run(
        command,
        cwd=root,
        stdout=subprocess.DEVNULL,
        check=True,
    )
    return json.loads(output.read_text(encoding="utf-8"))


def mean_and_variance(values: list[float]) -> dict:
    mean = statistics.fmean(values)
    standard_deviation = statistics.pstdev(values) if len(values) > 1 else 0.0
    return {
        "mean": mean,
        "standard_deviation": standard_deviation,
        "coefficient_of_variation": (standard_deviation / mean if mean else 0.0),
        "minimum": min(values),
        "maximum": max(values),
    }


def percentile(values: list[float], percentile_value: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = math.ceil(percentile_value * len(ordered)) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def aggregate_runs(results: list[dict], modes: list[str], runs: int) -> dict:
    by_mode_run: dict[tuple[str, int], list[dict]] = defaultdict(list)
    for result in results:
        by_mode_run[(result["mode"], result["run"])].append(result)

    mode_summary = {}
    for mode in modes:
        run_aggregates = []
        for run_number in range(1, runs + 1):
            entries = by_mode_run[(mode, run_number)]
            total_queries = sum(entry["queries"] for entry in entries)
            aggregate = {
                "run": run_number,
                "queries": total_queries,
                "index_ms": sum(entry["index_ms"] for entry in entries),
                "hash_enhancement_ms": sum(
                    entry["hash_enhancement_ms"] for entry in entries
                ),
                "neural_enhancement_ms": sum(
                    entry["neural_enhancement_ms"] for entry in entries
                ),
                "daemon_startup_ms": sum(
                    entry["daemon_startup_ms"] for entry in entries
                ),
                "neural_model_ready_ms": sum(
                    entry["neural_model_ready_ms"] for entry in entries
                ),
                "index_size_bytes": sum(entry["index_size_bytes"] for entry in entries),
                "peak_child_rss_bytes": max(
                    (entry["peak_child_rss_bytes"] or 0 for entry in entries),
                    default=0,
                ),
            }
            for metric in QUALITY_METRICS:
                aggregate[metric] = (
                    sum(entry[metric] * entry["queries"] for entry in entries)
                    / total_queries
                )
            cold_latencies = [
                detail["cold_latency_ms"]
                for entry in entries
                for detail in entry["details"]
                if detail["cold_latency_ms"] is not None
            ]
            warm_latencies = [
                detail["warm_latency_ms"]
                for entry in entries
                for detail in entry["details"]
            ]
            aggregate.update(
                {
                    "cold_latency_p50_ms": percentile(cold_latencies, 0.50),
                    "cold_latency_p95_ms": percentile(cold_latencies, 0.95),
                    "warm_latency_p50_ms": percentile(warm_latencies, 0.50),
                    "warm_latency_p95_ms": percentile(warm_latencies, 0.95),
                }
            )
            run_aggregates.append(aggregate)

        mode_summary[mode] = {
            "runs": run_aggregates,
            "metrics": {
                metric: mean_and_variance(
                    [float(item[metric]) for item in run_aggregates]
                )
                for metric in (
                    *QUALITY_METRICS,
                    *LATENCY_METRICS,
                    *RESOURCE_METRICS,
                )
            },
        }
    return mode_summary


def summarize_tasks(results: list[dict]) -> dict:
    grouped: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for result in results:
        grouped[(result["dataset"], result["mode"])].append(result)
    summary = {}
    for (dataset, mode), entries in sorted(grouped.items()):
        summary.setdefault(dataset, {})[mode] = {
            metric: mean_and_variance([float(entry[metric] or 0) for entry in entries])
            for metric in (
                *QUALITY_METRICS,
                *LATENCY_METRICS,
                *RESOURCE_METRICS,
            )
        }
    return summary


def publication_result(result: dict) -> dict:
    return {key: value for key, value in result.items() if key != "details"}


def validate_reused_result(
    result: dict,
    dataset: Path,
    mode: str,
    binary_sha256: str,
) -> None:
    provenance = json.loads((dataset / "provenance.json").read_text(encoding="utf-8"))
    if result.get("dataset") != dataset.name or result.get("mode") != mode:
        raise ValueError(f"reused result does not match {dataset.name}/{mode}")
    if result.get("binary", {}).get("sha256") != binary_sha256:
        raise ValueError(
            f"reused result has a different binary for {dataset.name}/{mode}"
        )
    if result.get("dataset_provenance", {}).get("checksums") != provenance["checksums"]:
        raise ValueError(
            f"reused result has different dataset bytes for {dataset.name}"
        )


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=root / "benchmarks" / "public" / "manifest.json",
    )
    parser.add_argument("--profile", default="public-core")
    parser.add_argument("--datasets-root", type=Path, required=True)
    parser.add_argument("--work-root", type=Path, required=True)
    parser.add_argument(
        "--binary", type=Path, default=root / "target" / "release" / "ig"
    )
    parser.add_argument("--modes", default="lexical,hash,hybrid")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--skip-export", action="store_true")
    parser.add_argument(
        "--source-commit",
        help="Commit used to build --binary; defaults to the current checkout.",
    )
    parser.add_argument(
        "--reuse-results",
        action="store_true",
        help="reuse completed per-task result files from --work-root",
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    if args.runs < 1:
        raise ValueError("--runs must be positive")
    manifest = export_public_retrieval.load_manifest(args.manifest)
    tasks = export_public_retrieval.selected_tasks(manifest, args.profile, [])
    modes = parse_modes(args.modes)
    args.datasets_root.mkdir(parents=True, exist_ok=True)
    args.work_root.mkdir(parents=True, exist_ok=True)

    if not args.skip_export:
        export_datasets(manifest, args.profile, args.datasets_root, tasks)
    if not args.skip_build:
        build_binary(root, modes)
    binary = args.binary.resolve()
    if not binary.is_file():
        raise FileNotFoundError(binary)
    binary_sha256 = sha256_file(binary)

    dataset_paths = [args.datasets_root / task for task in tasks]
    provenances = [
        json.loads((dataset / "provenance.json").read_text(encoding="utf-8"))
        for dataset in dataset_paths
    ]
    validate_profile_query_count(manifest, args.profile, provenances)
    subprocess.run(
        [
            sys.executable,
            str(root / "scripts" / "check_retrieval_benchmark_leakage.py"),
            *[str(path) for path in dataset_paths],
        ],
        cwd=root,
        check=True,
    )

    results = []
    for run_number in range(1, args.runs + 1):
        for task, dataset in zip(tasks, dataset_paths, strict=True):
            for mode in modes:
                result_path = args.work_root / f"{task}-{mode}-run-{run_number}.json"
                if args.reuse_results and result_path.is_file():
                    result = json.loads(result_path.read_text(encoding="utf-8"))
                    validate_reused_result(
                        result,
                        dataset,
                        mode,
                        binary_sha256,
                    )
                else:
                    result = run_evaluation(root, dataset, binary, mode, result_path)
                result["run"] = run_number
                results.append(result)

    matrix = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "ivygrep_commit": benchmark_revision(root, args.source_commit),
        "manifest_sha256": sha256_file(args.manifest),
        "runtime": eval_code_retrieval.runtime_metadata(),
        "harness_sha256": {
            path.name: sha256_file(path)
            for path in (
                root / "scripts" / "eval_code_retrieval.py",
                root / "scripts" / "export_public_retrieval.py",
                root / "scripts" / "run_public_benchmark_matrix.py",
            )
        },
        "profile": args.profile,
        "tasks": tasks,
        "modes": modes,
        "repetitions": args.runs,
        "neural_models": [
            json.loads(identity)
            for identity in sorted(
                {
                    json.dumps(
                        result["index_configuration"]["neural_model"],
                        sort_keys=True,
                    )
                    for result in results
                    if result.get("index_configuration", {}).get("neural_model")
                }
            )
        ],
        "queries": sum(item["counts"]["queries"] for item in provenances),
        "languages": sorted(
            {
                language
                for provenance in provenances
                for language in provenance["languages"]
                if language
            },
            key=str.lower,
        ),
        "summary": aggregate_runs(results, modes, args.runs),
        "task_summary": summarize_tasks(results),
        "results": [publication_result(result) for result in results],
        "raw_result_files": [
            {
                "name": f"{result['dataset']}-{result['mode']}-run-{result['run']}.json",
                "sha256": sha256_file(
                    args.work_root
                    / f"{result['dataset']}-{result['mode']}-run-{result['run']}.json"
                ),
            }
            for result in results
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(matrix, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "profile": matrix["profile"],
                "tasks": len(tasks),
                "queries": matrix["queries"],
                "modes": modes,
                "repetitions": args.runs,
                "output": args.output.name,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
