#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Publish aggregate MemoryQuest retrieval evidence and HTML report."""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import subprocess
from collections import defaultdict
from datetime import datetime, timezone
from html import escape
from pathlib import Path

QUALITY_METRICS = (
    "ndcg_at_10",
    "mrr_at_10",
    "precision_at_5",
    "recall_at_5",
    "recall_at_10",
    "recall_at_20",
    "exact_at_5",
    "exact_at_10",
    "exact_at_20",
    "no_hit_rate",
)
RESOURCE_METRICS = (
    "index_ms",
    "hash_enhancement_ms",
    "neural_enhancement_ms",
    "index_size_bytes",
    "peak_child_rss_bytes",
    "daemon_startup_ms",
    "neural_model_ready_ms",
    "warm_latency_p50_ms",
    "warm_latency_p95_ms",
)
PUBLISHED_REFERENCE = {
    "source": "https://arxiv.org/pdf/2605.14177",
    "directly_comparable": False,
    "query_only_fact_recall": 0.58,
    "pgr_tot_gpt4o_recall": 0.723,
    "pgr_tot_gpt4o_exact": 0.326,
    "pgr_tot_deepseek_recall": 0.748,
    "pgr_tot_deepseek_exact": 0.348,
    "differences": [
        "published systems retrieve LLM-extracted atomic facts, not raw sessions",
        "PGR retrieves about 35 facts across multiple generated probes",
        "published recall uses GPT-5.2 semantic judging",
        "PGR averages 4.79 query-time LLM calls on MemoryQuest",
        "ivygrep uses fixed local probes with no query-time LLM",
    ],
}


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
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return completed.stdout.strip()


def load_jsonl(path: Path) -> list[dict]:
    with path.open(encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def mean_metrics(details: list[dict]) -> dict[str, float]:
    return {
        metric: statistics.fmean(float(item[metric]) for item in details)
        for metric in QUALITY_METRICS
        if metric != "no_hit_rate"
    }


def user_metrics(result: dict, users: dict[str, str]) -> tuple[dict, dict]:
    grouped: dict[str, list[dict]] = defaultdict(list)
    for detail in result["details"]:
        grouped[users[detail["query_id"]]].append(detail)
    per_user = {
        user: {"queries": len(details), **mean_metrics(details)}
        for user, details in sorted(grouped.items())
    }
    macro = {
        metric: statistics.fmean(
            float(metrics[metric]) for metrics in per_user.values()
        )
        for metric in QUALITY_METRICS
        if metric != "no_hit_rate"
    }
    return per_user, macro


def public_mode_result(result: dict, users: dict[str, str]) -> dict:
    per_user, macro = user_metrics(result, users)
    return {
        "mode": result["mode"],
        "queries": result["queries"],
        "binary": result["binary"],
        "runtime": result["runtime"],
        "quality": {metric: result[metric] for metric in QUALITY_METRICS},
        "per_user_macro_quality": macro,
        "resources": {metric: result[metric] for metric in RESOURCE_METRICS},
        "warm_query_path": result["warm_query_path"],
        "query_expansion": result["query_expansion"],
        "query_expansion_workers": result["query_expansion_workers"],
        "memory_expansion_disabled": result["memory_expansion_disabled"],
        "retrieval_provenance": result["retrieval_provenance"],
        "index_configuration": result["index_configuration"],
        "per_user": per_user,
    }


def validate_published_mode(mode: str, result: dict) -> None:
    expected_semantics = {
        "lexical": "lexical",
        "blended": "blended-routing",
        "neural": "forced-neural",
    }
    if result.get("mode") != mode:
        raise ValueError(f"{mode} result has mismatched mode")
    if result.get("query_expansion") != "none":
        raise ValueError(f"{mode} result uses benchmark-only query expansion")
    if result.get("memory_expansion_disabled"):
        raise ValueError(f"{mode} result disables default memory expansion")
    provenance = result.get("retrieval_provenance") or {}
    if provenance.get("mode_semantics") != expected_semantics[mode]:
        raise ValueError(f"{mode} result has unexpected mode semantics")
    if bool(provenance.get("force_neural")) != (mode == "neural"):
        raise ValueError(f"{mode} result has unexpected force-neural setting")


def metric_delta(current: float, baseline: float) -> dict[str, float | None]:
    return {
        "absolute": current - baseline,
        "relative_percent": (
            ((current / baseline) - 1.0) * 100.0 if baseline else None
        ),
    }


def build_publication(
    root: Path,
    dataset: Path,
    result_paths: list[Path],
    control_path: Path | None = None,
) -> dict:
    provenance = json.loads((dataset / "provenance.json").read_text(encoding="utf-8"))
    query_users = {
        str(item["_id"]): str(item["metadata"]["user"])
        for item in load_jsonl(dataset / "queries.jsonl")
    }
    results = {
        result["mode"]: result
        for path in result_paths
        for result in [json.loads(path.read_text(encoding="utf-8"))]
    }
    expected_modes = {"lexical", "blended", "neural"}
    if set(results) != expected_modes:
        raise ValueError(
            f"expected modes {sorted(expected_modes)}, got {sorted(results)}"
        )
    for mode, result in results.items():
        validate_published_mode(mode, result)
        if result["queries"] != provenance["counts"]["queries"]:
            raise ValueError(f"{mode} result query count does not match dataset")
        if result["dataset_provenance"]["checksums"] != provenance["checksums"]:
            raise ValueError(f"{mode} result uses different dataset bytes")
    binary_hashes = {result["binary"]["sha256"] for result in results.values()}
    if len(binary_hashes) != 1:
        raise ValueError("results use different ivygrep binaries")

    public_modes = {
        mode: public_mode_result(results[mode], query_users)
        for mode in ("lexical", "blended", "neural")
    }
    comparison = {
        mode: {
            metric: metric_delta(
                float(public_modes[mode]["quality"][metric]),
                float(public_modes["lexical"]["quality"][metric]),
            )
            for metric in (
                "ndcg_at_10",
                "mrr_at_10",
                "recall_at_10",
                "recall_at_20",
                "exact_at_10",
            )
        }
        for mode in ("blended", "neural")
    }
    research_experiments = {}
    if control_path is not None:
        control = json.loads(control_path.read_text(encoding="utf-8"))
        if control["mode"] != "blended":
            raise ValueError("single-query control must use blended mode")
        if control.get("query_expansion") != "none" or not control.get(
            "memory_expansion_disabled"
        ):
            raise ValueError("single-query control did not disable memory expansion")
        if control["queries"] != provenance["counts"]["queries"]:
            raise ValueError("single-query control query count does not match dataset")
        if control["dataset_provenance"]["checksums"] != provenance["checksums"]:
            raise ValueError("single-query control uses different dataset bytes")
        if control["binary"]["sha256"] not in binary_hashes:
            raise ValueError("single-query control uses different ivygrep binary")
        research_experiments["single_query_control"] = {
            "description": (
                "Default blended retrieval with automatic memory expansion disabled"
            ),
            "default": False,
            "quality": {
                metric: control[metric] for metric in QUALITY_METRICS
            },
            "resources": {
                metric: control[metric] for metric in RESOURCE_METRICS
            },
        }
    gap_analysis = {
        "default_recall_gain_from_10_to_20": (
            public_modes["blended"]["quality"]["recall_at_20"]
            - public_modes["blended"]["quality"]["recall_at_10"]
        ),
        "forced_neural_recall_at_20_gain_over_default": (
            public_modes["neural"]["quality"]["recall_at_20"]
            - public_modes["blended"]["quality"]["recall_at_20"]
        ),
        "default_queries_with_neural_execution": public_modes["blended"][
            "retrieval_provenance"
        ]["queries_with_neural_execution"],
        "queries": provenance["counts"]["queries"],
        "incomplete_query_rate_at_20": (
            1.0 - public_modes["blended"]["quality"]["exact_at_20"]
        ),
    }
    if "single_query_control" in research_experiments:
        control = research_experiments["single_query_control"]
        gap_analysis["automatic_expansion_recall_at_20_gain"] = (
            public_modes["blended"]["quality"]["recall_at_20"]
            - control["quality"]["recall_at_20"]
        )
        gap_analysis["automatic_expansion_exact_at_20_gain"] = (
            public_modes["blended"]["quality"]["exact_at_20"]
            - control["quality"]["exact_at_20"]
        )
    return {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "ivygrep_commit": git_revision(root),
        "runs": 1,
        "dataset": provenance,
        "harness_sha256": {
            name: sha256_file(root / "scripts" / name)
            for name in (
                "export_memoryquest.py",
                "eval_code_retrieval.py",
                "render_memory_benchmark.py",
            )
        },
        "modes": public_modes,
        "comparison_to_lexical": comparison,
        "gap_analysis": gap_analysis,
        "research_experiments": research_experiments,
        "published_reference": PUBLISHED_REFERENCE,
        "scope": {
            "retrieval_unit": "one Markdown file per conversation session",
            "query_scope": "one user's sessions dated at or before query date",
            "temporal_filter": "session date <= query date",
            "cutoff": 20,
            "quality_runs": 1,
            "reader_or_llm_judge": False,
        },
    }


def percent(value: float) -> str:
    return f"{value * 100:.1f}%"


def milliseconds(value: float) -> str:
    return f"{value:.2f} ms"


def render_html(publication: dict) -> str:
    dataset = publication["dataset"]
    counts = dataset["counts"]
    modes = publication["modes"]
    default = modes["blended"]
    control = publication["research_experiments"].get("single_query_control")
    rows = "".join(
        "<tr>"
        f"<td><code>{escape('forced neural' if mode == 'neural' else mode)}</code></td>"
        f"<td>{percent(result['quality']['recall_at_10'])}</td>"
        f"<td>{percent(result['quality']['recall_at_20'])}</td>"
        f"<td>{percent(result['quality']['exact_at_20'])}</td>"
        f"<td>{result['quality']['ndcg_at_10']:.4f}</td>"
        f"<td>{milliseconds(result['resources']['warm_latency_p95_ms'])}</td>"
        "</tr>"
        for mode, result in modes.items()
    )
    source_link = escape(dataset["source_repository"], quote=True)
    paper_link = escape(dataset["paper"], quote=True)
    license_link = escape(dataset["license_source"], quote=True)
    expansion_finding = ""
    control_reproduce = ""
    control_render_arg = ""
    if control is not None:
        expansion_finding = (
            "<p>Same-binary single-query control reaches "
            f"{percent(control['quality']['recall_at_20'])} recall@20 and "
            f"{percent(control['quality']['exact_at_20'])} complete recall@20 at "
            f"{milliseconds(control['resources']['warm_latency_p95_ms'])} p95. "
            "Automatic local probes add "
            f"{(default['quality']['recall_at_20'] - control['quality']['recall_at_20']) * 100:.1f} "
            "recall points and "
            f"{(default['quality']['exact_at_20'] - control['quality']['exact_at_20']) * 100:.1f} "
            "complete-recall points.</p>"
        )
        control_reproduce = (
            "\nuv run scripts/eval_code_retrieval.py "
            "--dataset /tmp/ivygrep-memoryquest --binary target/release/ig "
            "--mode blended --disable-memory-expansion "
            "--output /tmp/memoryquest-blended-control.json"
        )
        control_render_arg = (
            " --control-result /tmp/memoryquest-blended-control.json"
        )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>ivygrep Public Memory-Retrieval Benchmark</title>
    <meta name="description" content="Public MemoryQuest personal-memory retrieval quality, latency, and indexing evidence.">
    <link rel="stylesheet" href="../style.css">
    <link rel="stylesheet" href="report.css">
    <link rel="icon" type="image/svg+xml" href="../assets/icon.svg">
</head>
<body class="report-page">
    <div class="bg-fx"></div><div class="bg-fx-glow"></div>
    <main class="report-shell relative z-10">
        <nav class="report-nav">
            <a class="report-brand" href="../"><img src="../assets/icon.svg" alt="ivygrep"><span>ivygrep benchmarks</span></a>
            <div class="report-links"><a href="index.html">Reports</a><a href="public-memory-retrieval-results.json">Raw JSON</a></div>
        </nav>
        <section class="report-hero">
            <div class="report-eyebrow">Public benchmark</div>
            <h1>Preindexed notes and memory retrieval</h1>
            <p>MemoryQuest sessions become {counts["documents"]:,} local Markdown notes across {counts["users"]} users. {counts["queries"]} natural questions test retrieval of {counts["required_references"]:,} required memories.</p>
        </section>
        <section class="report-grid">
            <div class="report-stat"><strong>{percent(default["quality"]["recall_at_20"])}</strong><span>default recall@20</span></div>
            <div class="report-stat"><strong>{percent(default["quality"]["exact_at_20"])}</strong><span>complete recall@20</span></div>
            <div class="report-stat"><strong>{milliseconds(default["resources"]["warm_latency_p95_ms"])}</strong><span>warm default p95</span></div>
            <div class="report-stat"><strong>{counts["documents"]:,}</strong><span>preindexed sessions</span></div>
        </section>
        <section class="report-card">
            <h2>Aggregate results</h2>
            <div class="report-table-wrap"><table class="report-table">
                <thead><tr><th>Mode</th><th>Recall@10</th><th>Recall@20</th><th>Exact@20</th><th>nDCG@10</th><th>Warm p95</th></tr></thead>
                <tbody>{rows}</tbody>
            </table></div>
            <p><code>blended</code> is default daemon-backed semantic + lexical routing across CLI, MCP, Web, and TUI. When an implicit natural-language question initially returns overwhelmingly note-like files, default search runs three generic local memory probes concurrently and fuses file ranks. <code>forced neural</code> disables this adaptive path and runs neural retrieval for every query. <code>lexical</code> disables vectors.</p>
        </section>
        <section class="report-card">
            <h2>Method</h2>
            <p><a href="{source_link}">MemoryQuest</a> is a <a href="{paper_link}">Microsoft Research and University of Washington</a> benchmark for implicit, context-dependent personal-memory retrieval. Exporter creates one Markdown note per session and indexes all notes once. Each historical question searches only its user's sessions dated at or before query date, matching official temporal protocol.</p>
            <p>Only session date and raw conversation turns enter index. Topics, domains, required-memory flags, demographics, timelines, reasoning, and references stay outside indexed tree. All {counts["required_references"]:,} references resolve to labeled sessions.</p>
            <p>Dataset uses <a href="{license_link}">CC BY 4.0</a>; generated corpus is not redistributed.</p>
        </section>
        <section class="report-card">
            <h2>Published reference point</h2>
            <p><a href="https://arxiv.org/pdf/2605.14177">MemoryQuest paper</a> reports 58.0% recall for query-only fact retrieval, 72.3% recall / 32.6% exact for GPT-4o PGR-TOT, and 74.8% / 34.8% for iterative DeepSeek-V3.2 PGR-TOT.</p>
            <p>These are not direct leaderboard comparisons. Published systems retrieve LLM-extracted atomic facts, PGR gathers about 35 facts through multiple generated probes and 4.79 average query-time LLM calls, and GPT-5.2 judges whether references appear. ivygrep result uses top-20 raw session files, deterministic session labels, fixed local probes with no query-time LLM, and no LLM judge.</p>
        </section>
        <section class="report-card">
            <h2>Where recall is still lost</h2>
            <p>Depth is largest measured ranking gap: default recall rises from {percent(default["quality"]["recall_at_10"])} at 10 to {percent(default["quality"]["recall_at_20"])} at 20. MemoryQuest intentionally makes required memories semantically distant from query and requires three or four sessions per question.</p>
            <p>Forcing neural retrieval on every query reaches {percent(modes["neural"]["quality"]["recall_at_20"])}, {(default["quality"]["recall_at_20"] - modes["neural"]["quality"]["recall_at_20"]) * 100:.1f} percentage points below default. Default's gain comes from prospective rank fusion, not simply forcing existing vector pass. Remaining misses need better probe selection or structured fact memory.</p>
            {expansion_finding}
        </section>
        <section class="report-card">
            <h2>What this establishes</h2>
            <p>ivygrep can preindex note-like text once, then retrieve related memories from natural-language questions without hosted inference. Measurement covers session retrieval, not final answer correctness.</p>
            <p>Limits: conversations are synthetic, each released question needs three or four sessions, run uses one Linux host and one quality pass, and latency varies by hardware and load.</p>
        </section>
        <section class="report-card">
            <h2>Reproduce</h2>
            <pre><code>uv run scripts/export_memoryquest.py --output /tmp/ivygrep-memoryquest
uv run scripts/eval_code_retrieval.py --dataset /tmp/ivygrep-memoryquest --binary target/release/ig --mode lexical --output /tmp/memoryquest-lexical.json
uv run scripts/eval_code_retrieval.py --dataset /tmp/ivygrep-memoryquest --binary target/release/ig --mode blended --output /tmp/memoryquest-blended.json
uv run scripts/eval_code_retrieval.py --dataset /tmp/ivygrep-memoryquest --binary target/release/ig --mode neural --output /tmp/memoryquest-neural.json
{control_reproduce}
uv run scripts/render_memory_benchmark.py --dataset /tmp/ivygrep-memoryquest --result /tmp/memoryquest-lexical.json --result /tmp/memoryquest-blended.json --result /tmp/memoryquest-neural.json{control_render_arg} --output-json docs/benchmarks/public-memory-retrieval-results.json --output-html docs/benchmarks/public-memory-retrieval.html</code></pre>
        </section>
    </main>
</body>
</html>
"""


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--result", action="append", type=Path, required=True)
    parser.add_argument("--control-result", type=Path)
    parser.add_argument("--output-json", type=Path, required=True)
    parser.add_argument("--output-html", type=Path, required=True)
    args = parser.parse_args()
    publication = build_publication(
        root,
        args.dataset,
        args.result,
        args.control_result,
    )
    args.output_json.parent.mkdir(parents=True, exist_ok=True)
    args.output_html.parent.mkdir(parents=True, exist_ok=True)
    args.output_json.write_text(
        json.dumps(publication, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    args.output_html.write_text(render_html(publication), encoding="utf-8")
    print(
        json.dumps(
            {
                "queries": publication["dataset"]["counts"]["queries"],
                "documents": publication["dataset"]["counts"]["documents"],
                "default_recall_at_10": publication["modes"]["blended"]["quality"][
                    "recall_at_10"
                ],
                "output_json": str(args.output_json),
                "output_html": str(args.output_html),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
