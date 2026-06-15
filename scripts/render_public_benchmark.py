#!/usr/bin/env python3
"""Render public benchmark JSON as Markdown and static HTML."""

from __future__ import annotations

import argparse
from html import escape
import json
from pathlib import Path


def metric(matrix: dict, mode: str, name: str) -> float:
    return matrix["summary"][mode]["metrics"][name]["mean"]


def metric_stat(matrix: dict, mode: str, name: str, statistic: str) -> float:
    return matrix["summary"][mode]["metrics"][name][statistic]


def task_metric(matrix: dict, task: str, mode: str, name: str) -> float:
    return matrix["task_summary"][task][mode][name]["mean"]


def format_ms(value: float) -> str:
    return f"{value:.2f} ms"


def format_bytes(value: float) -> str:
    units = ("B", "KiB", "MiB", "GiB", "TiB")
    size = float(value)
    for unit in units:
        if size < 1024 or unit == units[-1]:
            return f"{size:.2f} {unit}"
        size /= 1024
    raise AssertionError("unreachable")


def markdown(matrix: dict) -> str:
    lines = [
        "# Public code-retrieval benchmark",
        "",
        "This report is generated from pinned public CoIR datasets. It contains no "
        "hostnames, user paths, private repository names, or source text.",
        "",
        f"- Commit: `{matrix['ivygrep_commit']}`",
        f"- Profile: `{matrix['profile']}`",
        f"- Tasks: {len(matrix['tasks'])}",
        f"- Languages: {len(matrix.get('languages', []))}",
        f"- Held-out queries: {matrix['queries']}",
        f"- Repetitions: {matrix['repetitions']}",
        "",
        "## Aggregate results",
        "",
        "| Mode | nDCG@10 | MRR@10 | P@5 | R@20 | Warm p95 | Index time | Index size | Peak RSS |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for mode in matrix["modes"]:
        lines.append(
            "| "
            + " | ".join(
                (
                    mode,
                    f"{metric(matrix, mode, 'ndcg_at_10'):.4f}",
                    f"{metric(matrix, mode, 'mrr_at_10'):.4f}",
                    f"{metric(matrix, mode, 'precision_at_5'):.4f}",
                    f"{metric(matrix, mode, 'recall_at_20'):.4f}",
                    format_ms(metric(matrix, mode, "warm_latency_p95_ms")),
                    format_ms(metric(matrix, mode, "index_ms")),
                    format_bytes(metric(matrix, mode, "index_size_bytes")),
                    format_bytes(metric(matrix, mode, "peak_child_rss_bytes")),
                )
            )
            + " |"
        )
    lines.extend(
        (
            "",
            "## Run variance",
            "",
            "| Mode | nDCG@10 stddev | nDCG CV | Warm p95 stddev | Warm p95 CV |",
            "| --- | ---: | ---: | ---: | ---: |",
        )
    )
    for mode in matrix["modes"]:
        lines.append(
            "| "
            + " | ".join(
                (
                    mode,
                    f"{metric_stat(matrix, mode, 'ndcg_at_10', 'standard_deviation'):.4f}",
                    f"{metric_stat(matrix, mode, 'ndcg_at_10', 'coefficient_of_variation'):.2%}",
                    format_ms(
                        metric_stat(
                            matrix,
                            mode,
                            "warm_latency_p95_ms",
                            "standard_deviation",
                        )
                    ),
                    f"{metric_stat(matrix, mode, 'warm_latency_p95_ms', 'coefficient_of_variation'):.2%}",
                )
            )
            + " |"
        )
    if matrix.get("task_summary"):
        lines.extend(
            (
                "",
                "## Per-task quality",
                "",
                "| Task | Mode | nDCG@10 | MRR@10 | R@20 |",
                "| --- | --- | ---: | ---: | ---: |",
            )
        )
        for task in matrix["tasks"]:
            for mode in matrix["modes"]:
                lines.append(
                    "| "
                    + " | ".join(
                        (
                            task,
                            mode,
                            f"{task_metric(matrix, task, mode, 'ndcg_at_10'):.4f}",
                            f"{task_metric(matrix, task, mode, 'mrr_at_10'):.4f}",
                            f"{task_metric(matrix, task, mode, 'recall_at_20'):.4f}",
                        )
                    )
                    + " |"
                )
    lines.extend(
        (
            "",
            "Variance is recorded in the machine-readable JSON as population "
            "standard deviation, coefficient of variation, minimum, and maximum.",
            "",
            "## Interpretation",
            "",
            "These numbers establish a reproducible baseline; they are not a "
            "state-of-the-art claim. Exact-search systems are only comparable on "
            "exact-query workloads, while this matrix evaluates code information "
            "retrieval using held-out natural-language and code-to-code queries.",
            "",
            "## Reproduce",
            "",
            "```bash",
            "uv run scripts/run_public_benchmark_matrix.py \\",
            "  --profile public-core \\",
            "  --modes lexical,hash,hybrid \\",
            "  --runs 3 \\",
            "  --datasets-root /tmp/ivygrep-public-datasets \\",
            "  --work-root /tmp/ivygrep-public-results \\",
            "  --output public-code-retrieval-results.json",
            "```",
            "",
            "Use `--modes lexical,hash,hybrid,neural` with a default-feature "
            "build to exercise every ivygrep retrieval mode. Neural runs fail "
            "if model vectors are unavailable instead of silently reporting a "
            "hash fallback.",
            "",
            "The `full` profile contains every pinned CoIR task and language "
            "subtask. Dataset cards remain the authority for licensing; the "
            "exporter records whether a card declares a license.",
            "",
        )
    )
    return "\n".join(lines)


def html(matrix: dict) -> str:
    rows = []
    for mode in matrix["modes"]:
        rows.append(
            "<tr>"
            f"<td><code>{escape(mode)}</code></td>"
            f"<td>{metric(matrix, mode, 'ndcg_at_10'):.4f}</td>"
            f"<td>{metric(matrix, mode, 'mrr_at_10'):.4f}</td>"
            f"<td>{metric(matrix, mode, 'precision_at_5'):.4f}</td>"
            f"<td>{metric(matrix, mode, 'recall_at_20'):.4f}</td>"
            f"<td>{escape(format_ms(metric(matrix, mode, 'warm_latency_p95_ms')))}</td>"
            f"<td>{escape(format_bytes(metric(matrix, mode, 'index_size_bytes')))}</td>"
            "</tr>"
        )
    task_rows = []
    for task in matrix.get("tasks", []):
        for mode in matrix.get("modes", []):
            if task not in matrix.get("task_summary", {}):
                continue
            task_rows.append(
                "<tr>"
                f"<td><code>{escape(task)}</code></td>"
                f"<td><code>{escape(mode)}</code></td>"
                f"<td>{task_metric(matrix, task, mode, 'ndcg_at_10'):.4f}</td>"
                f"<td>{task_metric(matrix, task, mode, 'mrr_at_10'):.4f}</td>"
                f"<td>{task_metric(matrix, task, mode, 'recall_at_20'):.4f}</td>"
                "</tr>"
            )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>ivygrep Public Code-Retrieval Benchmark</title>
    <meta name="description" content="Pinned public CoIR quality, latency, indexing, memory, and index-size evidence.">
    <link rel="stylesheet" href="../style.css">
    <link rel="stylesheet" href="report.css">
    <link rel="icon" type="image/svg+xml" href="../assets/icon.svg">
</head>
<body class="report-page">
    <div class="bg-fx"></div>
    <div class="bg-fx-glow"></div>
    <main class="report-shell relative z-10">
        <nav class="report-nav">
            <a class="report-brand" href="../"><img src="../assets/icon.svg" alt="ivygrep"><span>ivygrep benchmarks</span></a>
            <div class="report-links"><a href="index.html">Reports</a><a href="public-code-retrieval-results.json">Raw JSON</a><a href="https://github.com/bvolpato/ivygrep/blob/main/docs/benchmarks/public-code-retrieval.md">Source</a></div>
        </nav>
        <section class="report-hero">
            <div class="report-eyebrow">Pinned Public Evidence</div>
            <h1>Code-retrieval quality and cost</h1>
            <p>{matrix["queries"]} held-out queries across {len(matrix["tasks"])} public CoIR tasks, repeated {matrix["repetitions"]} times. No private corpus or local path is included.</p>
        </section>
        <section class="report-grid">
            <div class="report-stat"><strong>{matrix["queries"]}</strong><span>held-out queries</span></div>
            <div class="report-stat"><strong>{len(matrix["tasks"])}</strong><span>public tasks</span></div>
            <div class="report-stat"><strong>{len(matrix.get("languages", []))}</strong><span>languages</span></div>
            <div class="report-stat"><strong>{matrix["repetitions"]}</strong><span>repetitions</span></div>
        </section>
        <section class="report-card">
            <h2>Aggregate results</h2>
            <div class="report-table-wrap"><table class="report-table">
                <thead><tr><th>Mode</th><th>nDCG@10</th><th>MRR@10</th><th>P@5</th><th>R@20</th><th>Warm p95</th><th>Index size</th></tr></thead>
                <tbody>{"".join(rows)}</tbody>
            </table></div>
            <p>Population variance, phase timings, peak RSS, binary identity, dataset revisions, and checksums are retained in the raw JSON.</p>
        </section>
        <section class="report-card">
            <h2>Per-task quality</h2>
            <div class="report-table-wrap"><table class="report-table">
                <thead><tr><th>Task</th><th>Mode</th><th>nDCG@10</th><th>MRR@10</th><th>R@20</th></tr></thead>
                <tbody>{"".join(task_rows)}</tbody>
            </table></div>
            <p>Every retained task remains visible so aggregate improvements cannot hide regressions.</p>
        </section>
        <section class="report-card">
            <h2>Claim boundary</h2>
            <p>This is a reproducible baseline, not a state-of-the-art claim. Exact-search tools are only compared on exact-query workloads; this matrix covers held-out natural-language and code-to-code retrieval. Neural and external embedding upper bounds use the same matrix when their profiles are evaluated.</p>
        </section>
    </main>
</body>
</html>
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    parser.add_argument("--html", type=Path, required=True)
    args = parser.parse_args()
    matrix = json.loads(args.input.read_text(encoding="utf-8"))
    args.markdown.write_text(markdown(matrix), encoding="utf-8")
    args.html.write_text(html(matrix), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
