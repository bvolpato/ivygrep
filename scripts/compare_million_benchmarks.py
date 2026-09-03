#!/usr/bin/env python3
"""Compare frozen public million-chunk benchmark artifacts."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import random
import statistics

QUERY_PATHS = (
    "process_cold",
    "cli_warm_distinct",
    "warm_distinct",
    "cache_replay",
    "filtered",
    "concurrent",
)

SAMPLED_PROCESS_METRICS = frozenset(
    {"peak_rss_bytes", "cpu_ms", "filesystem_read_bytes", "filesystem_write_bytes"}
)


def sampled_metric(metrics: dict, name: str) -> float | None:
    # Older artifacts did not record sample counts; retain their reported values.
    if name in SAMPLED_PROCESS_METRICS and metrics.get("resource_samples") == 0:
        return None
    return metrics.get(name)


def percentile(values: list[float], quantile: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * quantile) - 1))
    return ordered[index]


def bootstrap_p95_ratio(
    baseline: list[float],
    current: list[float],
    repetitions: int = 5_000,
) -> dict:
    rng = random.Random(20260615)
    ratios = []
    for _ in range(repetitions):
        baseline_sample = [rng.choice(baseline) for _ in baseline]
        current_sample = [rng.choice(current) for _ in current]
        baseline_p95 = percentile(baseline_sample, 0.95)
        current_p95 = percentile(current_sample, 0.95)
        ratios.append(current_p95 / baseline_p95)
    return {
        "observed": percentile(current, 0.95) / percentile(baseline, 0.95),
        "ci95_lower": percentile(ratios, 0.025),
        "ci95_upper": percentile(ratios, 0.975),
        "bootstrap_repetitions": repetitions,
    }


def bootstrap_median_ratio(
    baseline: list[float],
    current: list[float],
    repetitions: int = 5_000,
) -> dict:
    rng = random.Random(20260615)
    ratios = []
    for _ in range(repetitions):
        baseline_sample = [rng.choice(baseline) for _ in baseline]
        current_sample = [rng.choice(current) for _ in current]
        ratios.append(
            statistics.median(current_sample) / statistics.median(baseline_sample)
        )
    return {
        "observed": statistics.median(current) / statistics.median(baseline),
        "ci95_lower": percentile(ratios, 0.025),
        "ci95_upper": percentile(ratios, 0.975),
        "bootstrap_repetitions": repetitions,
    }


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def compare_runs(
    baseline_runs: list[dict],
    current_runs: list[dict],
    significant_regression_ratio: float,
    required_warm_ratio: float | None,
    required_index_ratio: float | None,
    maximum_quality_loss: float,
    maximum_index_size_ratio: float = 1.05,
) -> dict:
    if not baseline_runs or not current_runs:
        raise ValueError("at least one baseline and current run is required")
    comparable_query_paths = [
        path
        for path in QUERY_PATHS
        if all(path in run["queries"] for run in baseline_runs + current_runs)
    ]
    query_latency_ratios = {}
    query_quality_losses = {}
    for path in comparable_query_paths:
        baseline_queries = [run["queries"][path] for run in baseline_runs]
        current_queries = [run["queries"][path] for run in current_runs]
        latency = bootstrap_p95_ratio(
            [
                sample
                for query in baseline_queries
                for sample in query["latency_samples_ms"]
            ],
            [
                sample
                for query in current_queries
                for sample in query["latency_samples_ms"]
            ],
        )
        latency["significant_regression"] = (
            latency["ci95_lower"] > significant_regression_ratio
        )
        query_latency_ratios[path] = latency
        query_quality_losses[path] = statistics.median(
            [query["expected_recall_at_20"] for query in baseline_queries]
        ) - statistics.median(
            [query["expected_recall_at_20"] for query in current_queries]
        )

    latency = query_latency_ratios["warm_distinct"]

    baseline_throughputs = [
        run["index"]["chunks_per_second"]
        for run in baseline_runs
        if run["index"]["chunks_per_second"] is not None
    ]
    current_throughputs = [
        run["index"]["chunks_per_second"]
        for run in current_runs
        if run["index"]["chunks_per_second"] is not None
    ]
    index_ratio = (
        bootstrap_median_ratio(baseline_throughputs, current_throughputs)
        if baseline_throughputs and current_throughputs
        else None
    )
    if index_ratio is not None:
        index_ratio["significant_regression"] = (
            index_ratio["ci95_upper"] < 1.0 / significant_regression_ratio
        )
    quality_loss = query_quality_losses["warm_distinct"]
    baseline_sizes = [
        run["index"]["size_bytes"]
        for run in baseline_runs
        if run["index"].get("size_bytes") is not None
    ]
    current_sizes = [
        run["index"]["size_bytes"]
        for run in current_runs
        if run["index"].get("size_bytes") is not None
    ]
    index_size_ratio = (
        statistics.median(current_sizes) / statistics.median(baseline_sizes)
        if baseline_sizes and current_sizes
        else None
    )

    def resource_ratio(name: str) -> float | None:
        baseline = [
            sampled_metric(run["index"].get("metrics", {}), name)
            for run in baseline_runs
        ]
        current = [
            sampled_metric(run["index"].get("metrics", {}), name)
            for run in current_runs
        ]
        # An incomplete run set cannot support a comparative resource claim.
        if (
            not baseline or not current
            or any(value is None for value in baseline + current)
        ):
            return None
        denominator = statistics.median(baseline)
        return statistics.median(current) / denominator if denominator else None

    failures = []
    for path, comparison in query_latency_ratios.items():
        if comparison["significant_regression"]:
            failures.append(
                f"{path.replace('_', ' ')} p95 has a statistically significant regression"
            )
    if index_ratio is not None and index_ratio["significant_regression"]:
        failures.append(
            "fresh-index throughput has a statistically significant regression"
        )
    if required_warm_ratio is not None and latency["observed"] > required_warm_ratio:
        failures.append(
            f"warm p95 ratio {latency['observed']:.3f} exceeds "
            f"required {required_warm_ratio:.3f}"
        )
    if required_index_ratio is not None and (
        index_ratio is None or index_ratio["observed"] < required_index_ratio
    ):
        failures.append(
            f"index throughput ratio "
            f"{index_ratio['observed'] if index_ratio else 0.0:.3f} is below "
            f"required {required_index_ratio:.3f}"
        )
    for path, path_quality_loss in query_quality_losses.items():
        if path_quality_loss > maximum_quality_loss:
            failures.append(
                f"{path.replace('_', ' ')} expected recall@20 loss "
                f"{path_quality_loss:.4f} exceeds allowed {maximum_quality_loss:.4f}"
            )
    if index_size_ratio is not None and index_size_ratio > maximum_index_size_ratio:
        failures.append(
            f"index size ratio {index_size_ratio:.3f} exceeds "
            f"allowed {maximum_index_size_ratio:.3f}"
        )

    return {
        "schema_version": 1,
        "baseline_commit": baseline_runs[0]["ivygrep_commit"],
        "current_commit": current_runs[0]["ivygrep_commit"],
        "baseline_runs": len(baseline_runs),
        "current_runs": len(current_runs),
        "warm_distinct_p95_ratio": latency,
        "query_path_p95_ratios": query_latency_ratios,
        "index_throughput_ratio": index_ratio,
        "index_size_ratio": index_size_ratio,
        "peak_disk_ratio": resource_ratio("peak_disk_bytes"),
        "peak_rss_ratio": resource_ratio("peak_rss_bytes"),
        "expected_recall_at_20_loss": quality_loss,
        "query_path_expected_recall_at_20_losses": query_quality_losses,
        "passed": not failures,
        "failures": failures,
    }


def compare(
    baseline: dict,
    current: dict,
    significant_regression_ratio: float,
    required_warm_ratio: float | None,
    required_index_ratio: float | None,
    maximum_quality_loss: float,
    maximum_index_size_ratio: float = 1.05,
) -> dict:
    return compare_runs(
        [baseline],
        [current],
        significant_regression_ratio,
        required_warm_ratio,
        required_index_ratio,
        maximum_quality_loss,
        maximum_index_size_ratio,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, nargs="+", required=True)
    parser.add_argument("--current", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--significant-regression-ratio", type=float, default=1.15)
    parser.add_argument("--require-warm-p95-ratio", type=float)
    parser.add_argument("--require-index-throughput-ratio", type=float)
    parser.add_argument("--maximum-quality-loss", type=float, default=0.0)
    parser.add_argument("--maximum-index-size-ratio", type=float, default=1.05)
    args = parser.parse_args()

    result = compare_runs(
        [load(path) for path in args.baseline],
        [load(path) for path in args.current],
        args.significant_regression_ratio,
        args.require_warm_p95_ratio,
        args.require_index_throughput_ratio,
        args.maximum_quality_loss,
        args.maximum_index_size_ratio,
    )
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
