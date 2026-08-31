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


def best_quality_mode(matrix: dict) -> str:
    return max(matrix["modes"], key=lambda mode: metric(matrix, mode, "ndcg_at_10"))


def relative_change(current: float, baseline: float) -> float:
    if baseline == 0:
        raise ValueError("cannot calculate relative change from zero")
    return (current - baseline) / baseline


def validate_comparison(matrix: dict, baseline: dict) -> None:
    if matrix["profile"] != baseline["profile"]:
        raise ValueError("benchmark profiles do not match")
    if matrix["queries"] != baseline["queries"]:
        raise ValueError("benchmark query counts do not match")
    if matrix["tasks"] != baseline["tasks"]:
        raise ValueError("benchmark tasks do not match")


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


def report_slug(matrix: dict) -> str:
    if matrix["profile"] == "public-core":
        return "public-code-retrieval"
    return f"public-{matrix['profile']}"


def default_output_name(matrix: dict) -> str:
    return f"{report_slug(matrix)}-results.json"


def query_evidence_label(matrix: dict) -> str:
    if matrix["profile"] == "public-core":
        return "regression queries"
    if matrix["profile"] == "reranker-fit":
        return "checkout-reference model-fit queries"
    audit = matrix.get("fit_query_audit") or {}
    reference = audit.get("reference", {}) if audit.get("schema_version") == 2 else {}
    if reference.get("verified") is True and audit.get("overlap_queries") == 0:
        return "checkout-reference fit-disjoint queries (executed model unverified)"
    if reference.get("verified") is True:
        return "public regression queries (checkout-reference fit IDs)"
    return "public queries (executed model unverified)"


def query_evidence_note(matrix: dict) -> str:
    if matrix["profile"] == "public-core":
        return (
            "Public-core is regression evidence, not an unseen-query generalization set: "
            "it overlaps the checkout-reference reranker's fit data. "
            "Fit-ID applicability to the executed model is unverified without model-checksum attestation. "
            "Its query set and acceptance gates are unchanged."
        )
    audit = matrix.get("fit_query_audit") or {}
    reference = audit.get("reference", {}) if audit.get("schema_version") == 2 else {}
    applicability = (
        "Executed-model fit disjointness is unverified: no model-checksum attestation binds the binary to the reference. "
        "Matching model IDs do not identify model bytes. This diagnostic does not replace the public-core release gate."
    )
    if reference.get("verified") is True:
        return f"The checkout-reference fit ledger overlaps {audit['overlap_queries']} query IDs. {applicability}"
    return f"This artifact does not record verified checkout-reference fit-ID disjointness. {applicability}"


def dataset_scope_note(matrix: dict) -> str:
    """Describe sampling and licensing without implying redistribution rights."""
    records = {}
    for result in matrix.get("results", []):
        task = result.get("dataset")
        provenance = result.get("dataset_provenance")
        if task and isinstance(provenance, dict):
            records.setdefault(task, provenance)
    sampled = []
    undeclared = []
    for task in matrix.get("tasks", []):
        provenance = records.get(task, {})
        sample = provenance.get("sample") or {}
        if sample.get("query_limit") or sample.get("corpus_limit"):
            sampled.append(
                f"{task} ({sample.get('query_limit', '?')} queries, "
                f"{sample.get('corpus_limit', '?')} documents)"
            )
        if provenance.get("license") == "not-declared-in-dataset-card":
            undeclared.append(task)
    notes = [
        "Artifacts use pinned public datasets and retain source revisions and checksums in raw JSON.",
    ]
    if sampled:
        notes.append("Deterministic samples: " + ", ".join(sampled) + ".")
    if undeclared:
        notes.append(
            "Dataset cards do not declare licenses for: "
            + ", ".join(undeclared)
            + ". Treat downloaded corpora as evaluation inputs; do not redistribute them without checking upstream terms."
        )
    return " ".join(notes)


def corpus_sampling(matrix: dict) -> list[dict]:
    """Per-task corpus size actually indexed versus the full upstream corpus."""
    records = {}
    for result in matrix.get("results", []):
        task = result.get("dataset")
        provenance = result.get("dataset_provenance")
        if task and isinstance(provenance, dict):
            records.setdefault(task, provenance)
    rows = []
    for task in matrix.get("tasks", []):
        provenance = records.get(task, {})
        counts = provenance.get("counts") or {}
        sample = provenance.get("sample") or {}
        indexed = counts.get("corpus")
        full = counts.get("source_corpus")
        if indexed is None and sample.get("corpus_limit"):
            indexed = sample["corpus_limit"]
        if indexed is None:
            continue
        rows.append(
            {
                "task": task,
                "indexed": int(indexed),
                "full": int(full) if full is not None else None,
                "sampled": full is not None and int(indexed) < int(full),
            }
        )
    return rows


def corpus_sampling_note(matrix: dict) -> str:
    """Headline-level disclosure: which corpora were sampled and how much was dropped."""
    rows = corpus_sampling(matrix)
    if not rows:
        return "Corpus sampling: provenance counts unavailable; see raw JSON."
    sampled = [row for row in rows if row["sampled"]]
    if not sampled:
        return "Corpus sampling: none. Every task indexes its full upstream corpus."
    limits = sorted({row["indexed"] for row in sampled})
    limit_text = (
        f"{limits[0]:,} documents" if len(limits) == 1 else f"{limits[0]:,}-{limits[-1]:,} documents"
    )
    parts = [
        f"{row['task']} {row['full']:,} documents ({row['indexed']:,} indexed)"
        for row in sampled
    ]
    note = (
        f"Corpus sampling: {len(sampled)} of {len(rows)} corpora sampled to {limit_text} "
        "(qrel documents retained, remainder dropped deterministically). Full CoIR corpora: "
        + ", ".join(parts)
        + "."
    )
    unsampled = [row for row in rows if not row["sampled"]]
    if unsampled:
        note += " Indexed in full: " + ", ".join(
            f"{row['task']} ({row['indexed']:,})" for row in unsampled
        ) + "."
    note += " Scores on sampled corpora are not comparable to full-corpus CoIR leaderboard numbers."
    return note


def markdown(matrix: dict, baseline: dict | None = None) -> str:
    lines = [
        "# Public code-retrieval benchmark",
        "",
        "Pinned public CoIR datasets. Report excludes hostnames, user paths, "
        "private repository names, and source text.",
        "",
        f"- Commit: `{matrix['ivygrep_commit']}`",
        f"- Profile: `{matrix['profile']}`",
        f"- Tasks: {len(matrix['tasks'])}",
        f"- Languages: {len(matrix.get('languages', []))}",
        f"- {query_evidence_label(matrix).capitalize()}: {matrix['queries']}",
        f"- Repetitions: {matrix['repetitions']}",
        f"- {corpus_sampling_note(matrix)}",
        f"- Dataset scope: {dataset_scope_note(matrix)}",
    ]
    if matrix.get("aggregation_provenance"):
        aggregation = matrix["aggregation_provenance"]
        lines.extend(
            [
                f"- Aggregation checkout: `{aggregation['source_commit']}` (separate from original execution)",
                f"- Aggregated at: `{aggregation['generated_at']}`",
            ]
        )
    if matrix.get("query_text_limit") is not None:
        lines.append(f"- Query text limit: {matrix['query_text_limit']} characters")
    lines.extend(
        (
            "",
            "## Aggregate results",
            "",
            "| Mode | nDCG@10 | MRR@10 | P@5 | R@20 | Warm p95 | Index time | Index size | Peak RSS |",
            "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
        )
    )
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
    if baseline is not None:
        validate_comparison(matrix, baseline)
        baseline_mode = best_quality_mode(baseline)
        current_mode = best_quality_mode(matrix)
        lines.extend(
            (
                "",
                "## Change from frozen baseline",
                "",
                f"Baseline commit `{baseline['ivygrep_commit']}` mode "
                f"`{baseline_mode}` is compared with current mode `{current_mode}`.",
                "",
                "| Metric | Baseline | Current | Relative change |",
                "| --- | ---: | ---: | ---: |",
            )
        )
        for name, label in (
            ("ndcg_at_10", "nDCG@10"),
            ("mrr_at_10", "MRR@10"),
            ("precision_at_5", "P@5"),
            ("recall_at_20", "R@20"),
        ):
            baseline_value = metric(baseline, baseline_mode, name)
            current_value = metric(matrix, current_mode, name)
            lines.append(
                f"| {label} | {baseline_value:.4f} | {current_value:.4f} | "
                f"{relative_change(current_value, baseline_value):+.2%} |"
            )
        lines.extend(
            (
                "",
                "| Task | Baseline nDCG@10 | Current nDCG@10 | Absolute change |",
                "| --- | ---: | ---: | ---: |",
            )
        )
        for task in matrix["tasks"]:
            baseline_value = task_metric(baseline, task, baseline_mode, "ndcg_at_10")
            current_value = task_metric(matrix, task, current_mode, "ndcg_at_10")
            lines.append(
                f"| {task} | {baseline_value:.4f} | {current_value:.4f} | "
                f"{current_value - baseline_value:+.4f} |"
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
    reproduce_command = [
        "uv run scripts/run_public_benchmark_matrix.py \\",
        f"  --profile {matrix['profile']} \\",
        f"  --modes {','.join(matrix['modes'])} \\",
        f"  --runs {matrix['repetitions']} \\",
        "  --datasets-root /tmp/ivygrep-public-datasets \\",
        "  --work-root /tmp/ivygrep-public-results \\",
        f"  --output {default_output_name(matrix)}",
    ]
    if matrix.get("query_text_limit") is not None:
        reproduce_command.insert(
            -1,
            f"  --max-query-chars {matrix['query_text_limit']} \\",
        )
    lines.extend(
        (
            "",
            "Variance is recorded in the machine-readable JSON as population "
            "standard deviation, coefficient of variation, minimum, and maximum.",
            "",
            "## Scope",
            "",
            "Matrix covers natural-language and code-to-code retrieval. "
            + query_evidence_note(matrix)
            + " "
            "Exact-search tools require a separate exact-query workload.",
            "",
            "## Reproduce",
            "",
            "```bash",
            *reproduce_command,
            "```",
            "",
            "Use `--modes lexical,hash,hybrid,blended,neural` with a "
            "default-feature build to exercise every retrieval mode. `blended` "
            "measures normal production routing with neural vectors available; "
            "`neural` forces neural retrieval and fails if vectors are unavailable.",
            "",
            "The `full` profile contains every pinned CoIR task and language "
            "subtask. Dataset cards remain the authority for licensing; the "
            "exporter records whether a card declares a license.",
            "",
        )
    )
    return "\n".join(lines)


def html(matrix: dict, baseline: dict | None = None) -> str:
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
    comparison = ""
    if baseline is not None:
        validate_comparison(matrix, baseline)
        baseline_mode = best_quality_mode(baseline)
        current_mode = best_quality_mode(matrix)
        ndcg_change = relative_change(
            metric(matrix, current_mode, "ndcg_at_10"),
            metric(baseline, baseline_mode, "ndcg_at_10"),
        )
        mrr_change = relative_change(
            metric(matrix, current_mode, "mrr_at_10"),
            metric(baseline, baseline_mode, "mrr_at_10"),
        )
        comparison = f"""        <section class="report-card">
            <h2>Change from frozen baseline</h2>
            <p><code>{escape(current_mode)}</code> improves nDCG@10 by {ndcg_change:+.2%} and MRR@10 by {mrr_change:+.2%} over <code>{escape(baseline_mode)}</code> at commit <code>{escape(baseline["ivygrep_commit"][:12])}</code>. The raw JSON retains every task and run.</p>
        </section>"""
    sampling_rows = corpus_sampling(matrix)
    sampled_count = sum(row["sampled"] for row in sampling_rows)
    sampling_stat = (
        f"""            <div class="report-stat"><strong>{sampled_count}/{len(sampling_rows)}</strong><span>corpora sampled</span></div>
"""
        if sampling_rows
        else ""
    )
    query_text_limit = matrix.get("query_text_limit")
    query_limit_stat = (
        f"""            <div class="report-stat"><strong>{escape(str(query_text_limit))}</strong><span>query char limit</span></div>
"""
        if query_text_limit is not None
        else ""
    )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>ivygrep Public Code-Retrieval Benchmark</title>
    <meta name="description" content="Public CoIR quality, latency, indexing, memory, and index-size results.">
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
            <div class="report-links"><a href="index.html">Reports</a><a href="{escape(default_output_name(matrix))}">Raw JSON</a></div>
        </nav>
        <section class="report-hero">
            <div class="report-eyebrow">Public benchmark</div>
            <h1>Code-retrieval quality and cost</h1>
            <p>Profile <code>{escape(matrix["profile"])}</code>: {matrix["queries"]} {escape(query_evidence_label(matrix))} across {len(matrix["tasks"])} public CoIR tasks, repeated {matrix["repetitions"]} times. No private corpus or local path is included.</p>
            <p><strong>{escape(corpus_sampling_note(matrix))}</strong></p>
        </section>
        <section class="report-grid">
            <div class="report-stat"><strong>{matrix["queries"]}</strong><span>{escape(query_evidence_label(matrix))}</span></div>
            <div class="report-stat"><strong>{len(matrix["tasks"])}</strong><span>public tasks</span></div>
            <div class="report-stat"><strong>{len(matrix.get("languages", []))}</strong><span>languages</span></div>
            <div class="report-stat"><strong>{matrix["repetitions"]}</strong><span>repetitions</span></div>
{sampling_stat.rstrip()}
{query_limit_stat.rstrip()}
        </section>
        <section class="report-card">
            <h2>Aggregate results</h2>
            <div class="report-table-wrap"><table class="report-table">
                <thead><tr><th>Mode</th><th>nDCG@10</th><th>MRR@10</th><th>P@5</th><th>R@20</th><th>Warm p95</th><th>Index size</th></tr></thead>
                <tbody>{"".join(rows)}</tbody>
            </table></div>
            <p>Population variance, phase timings, peak RSS, binary identity, dataset revisions, and checksums are retained in the raw JSON.</p>
        </section>
        {comparison}
        <section class="report-card">
            <h2>Per-task quality</h2>
            <div class="report-table-wrap"><table class="report-table">
                <thead><tr><th>Task</th><th>Mode</th><th>nDCG@10</th><th>MRR@10</th><th>R@20</th></tr></thead>
                <tbody>{"".join(task_rows)}</tbody>
            </table></div>
            <p>Every retained task remains visible so aggregate improvements cannot hide regressions.</p>
        </section>
        <section class="report-card">
            <h2>Scope</h2>
            <p>Matrix covers natural-language and code-to-code retrieval. Exact-search tools require a separate exact-query workload.</p>
            <p>{escape(query_evidence_note(matrix))}</p>
            <p>{escape(corpus_sampling_note(matrix))}</p>
            <p>{escape(dataset_scope_note(matrix))}</p>
        </section>
    </main>
</body>
</html>
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--html", type=Path, required=True)
    args = parser.parse_args()
    matrix = json.loads(args.input.read_text(encoding="utf-8"))
    baseline = (
        json.loads(args.baseline.read_text(encoding="utf-8"))
        if args.baseline
        else None
    )
    args.html.write_text(html(matrix, baseline), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
