#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Render repeated resource measurements without changing historical model screening."""

import argparse
from html import escape
import json
from pathlib import Path
import statistics


def read_report(path: Path) -> dict:
    report = json.loads(path.read_text())
    if report.get("status") != "complete":
        raise ValueError("resource report is incomplete")
    if not report["runs"] or any(run["index_size_bytes"] <= 0 for run in report["runs"]):
        raise ValueError("resource report has no index-size evidence")
    for profile in {run["profile"] for run in report["runs"]}:
        runs = [run for run in report["runs"] if run["profile"] == profile]
        if sorted(run["repetition"] for run in runs) != list(range(1, report["method"]["runs_per_profile"] + 1)):
            raise ValueError("resource report is missing repetitions")
    return report


def median_range(values: list[float], unit: str) -> str:
    scale = 1024 * 1024 if unit == "MiB" else 1
    values = [value / scale for value in values]
    return f"{statistics.median(values):.2f} {unit} ({min(values):.2f} to {max(values):.2f})"


def model_rows(report: dict) -> list[list[str]]:
    rows = []
    for profile in sorted({run["profile"] for run in report["runs"]}):
        runs = [run for run in report["runs"] if run["profile"] == profile]
        metrics = (
            ("Neural enhancement", [run["phases"]["neural_enhancement"]["elapsed_ms"] for run in runs], "ms"),
            ("Enhancement peak RSS", [run["phases"]["neural_enhancement"]["peak_rss_bytes"] for run in runs], "MiB"),
            ("Enhancement CPU", [run["phases"]["neural_enhancement"]["cpu_ms"] for run in runs], "ms"),
            ("Enhancement disk writes", [run["phases"]["neural_enhancement"]["filesystem_write_bytes"] for run in runs], "MiB"),
            ("Forced-neural p95", [run["forced_neural"]["p95_ms"] for run in runs], "ms"),
            ("Forced-neural p99", [run["forced_neural"]["p99_ms"] for run in runs], "ms"),
            ("Concurrent MCP search p95", [run["concurrent_mcp"]["hybrid_search"]["p95_ms"] for run in runs], "ms"),
            ("Concurrent MCP search p99", [run["concurrent_mcp"]["hybrid_search"]["p99_ms"] for run in runs], "ms"),
            ("Concurrent context pack p95", [run["concurrent_mcp"]["context_pack"]["p95_ms"] for run in runs], "ms"),
            ("Concurrent context pack p99", [run["concurrent_mcp"]["context_pack"]["p99_ms"] for run in runs], "ms"),
            ("Daemon peak RSS during load", [run["load_peak_rss_bytes"] for run in runs], "MiB"),
            ("Daemon RSS after idle pause", [run["idle_rss_bytes"] for run in runs], "MiB"),
            ("CPU during load", [run["load_cpu_ms"] for run in runs], "ms"),
            ("Disk writes during load", [run["load_filesystem_write_bytes"] for run in runs], "MiB"),
            ("Background indexes completed", [run["background_indexes"]["samples"] for run in runs], "runs"),
            ("Index size, including background workspace", [run["index_size_bytes"] for run in runs], "MiB"),
        )
        rows.extend([profile, metric, median_range(values, unit)] for metric, values, unit in metrics)
    return rows


def comparison_rows(baseline: dict, candidate: dict) -> list[list[str]]:
    if baseline["corpus"] != candidate["corpus"] or baseline["method"] != candidate["method"]:
        raise ValueError("comparison requires the same corpus and method")
    if baseline["harness_sha256"] != candidate["harness_sha256"]:
        raise ValueError("comparison requires the same harness")
    if baseline["runtime"] != candidate["runtime"]:
        raise ValueError("comparison requires the same runtime metadata")
    rows = []
    for profile in sorted({run["profile"] for run in candidate["runs"]}):
        left = [run for run in baseline["runs"] if run["profile"] == profile]
        right = [run for run in candidate["runs"] if run["profile"] == profile]
        if not left or len(left) != len(right):
            raise ValueError("comparison requires the same repetitions and profiles")
        for kind, metric in [("hybrid_search", "p95_ms"), ("hybrid_search", "p99_ms"),
                             ("context_pack", "p95_ms"), ("context_pack", "p99_ms")]:
            before = [run["concurrent_mcp"][kind][metric] for run in left]
            after = [run["concurrent_mcp"][kind][metric] for run in right]
            ratio = statistics.median(after) / statistics.median(before)
            label = {"hybrid_search": "Hybrid search", "context_pack": "Context pack"}[kind]
            rows.append([profile, f"{label} {metric.removesuffix('_ms')}", median_range(before, "ms"),
                         median_range(after, "ms"), f"{(ratio - 1) * 100:+.1f}%"])
        for label, unit, extract in (
            ("Forced-neural p95", "ms", lambda run: run["forced_neural"]["p95_ms"]),
            ("Forced-neural p99", "ms", lambda run: run["forced_neural"]["p99_ms"]),
            ("Lexical indexing", "ms", lambda run: run["phases"]["lexical_index"]["elapsed_ms"]),
            ("Neural enhancement", "ms", lambda run: run["phases"]["neural_enhancement"]["elapsed_ms"]),
            ("Enhancement peak RSS", "MiB", lambda run: run["phases"]["neural_enhancement"]["peak_rss_bytes"]),
            ("Daemon peak RSS", "MiB", lambda run: run["load_peak_rss_bytes"]),
            ("Daemon RSS after idle pause", "MiB", lambda run: run["idle_rss_bytes"]),
            ("CPU during load", "ms", lambda run: run["load_cpu_ms"]),
            ("Disk writes during load", "MiB", lambda run: run["load_filesystem_write_bytes"]),
            ("Background index p95", "ms", lambda run: run["background_indexes"]["p95_ms"]),
            ("Background indexes completed", "runs", lambda run: run["background_indexes"]["samples"]),
        ):
            before = [extract(run) for run in left]
            after = [extract(run) for run in right]
            base = statistics.median(before)
            change = f"{(statistics.median(after) / base - 1) * 100:+.1f}%" if base else "unavailable"
            rows.append([profile, label, median_range(before, unit), median_range(after, unit), change])
    return rows


def table(headers: list[str], rows: list[list[str]]) -> str:
    head = "".join(f"<th>{escape(value)}</th>" for value in headers)
    body = "".join("<tr>" + "".join(f"<td>{escape(value)}</td>" for value in row) + "</tr>" for row in rows)
    return f'<div class="report-table-wrap"><table class="report-table"><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>'


def render(report: dict, raw_name: str, baseline: dict | None = None, candidate: dict | None = None,
           diagnostics: dict | None = None, diagnostics_name: str | None = None) -> str:
    method = report["method"]
    model_summary = ""
    selected = [run for run in report["runs"] if run["profile"] == "potion-code-16m-v2"]
    former = [run for run in report["runs"] if run["profile"] == "static-retrieval-v1"]
    if selected and former:
        elapsed_ratio = statistics.median(run["phases"]["neural_enhancement"]["elapsed_ms"] for run in selected) / statistics.median(run["phases"]["neural_enhancement"]["elapsed_ms"] for run in former)
        rss_ratio = statistics.median(run["phases"]["neural_enhancement"]["peak_rss_bytes"] for run in selected) / statistics.median(run["phases"]["neural_enhancement"]["peak_rss_bytes"] for run in former)
        model_summary = f'<p>Compared with <code>static-retrieval-v1</code>, enhancement with <code>potion-code-16m-v2</code> changed elapsed time by {(elapsed_ratio - 1) * 100:+.1f}% and peak RSS by {(rss_ratio - 1) * 100:+.1f}%.</p>'
    comparison = ""
    diagnostic_section = ""
    if diagnostics is not None:
        if not diagnostics["method"]["diagnostics_enabled"]:
            raise ValueError("diagnostic report must enable diagnostics")
        if diagnostics["corpus"] != report["corpus"]:
            raise ValueError("diagnostics require the same corpus")
        if candidate is not None and diagnostics["binary"] != candidate["binary"]:
            raise ValueError("diagnostics require the candidate binary")
        rows = []
        for run in diagnostics["runs"]:
            stages = run["diagnostics"]["stages"]
            for stage, values in sorted(stages.items()):
                rows.append([str(run["repetition"]), stage, str(values["samples"]),
                             f'{values["p50_ms"]:.3f} ms', f'{values["p95_ms"]:.3f} ms'])
        recoveries = sum(run["diagnostics"]["semantic_recoveries"] for run in diagnostics["runs"])
        scans = sum(run["diagnostics"]["exact_scans"] for run in diagnostics["runs"])
        scanned = sum(run["diagnostics"]["scanned_keys"] for run in diagnostics["runs"])
        diagnostic_section = f'''<section class="report-card"><h2>Candidate stage timings</h2>
<p>A separate diagnostic run used {diagnostics["method"]["samples"]} samples per latency path and enabled debug logs. Its resource totals are separate from the comparison above.</p>
<p>Stages can be nested. Do not add their percentiles to estimate request p95.</p>
{table(["Run", "Stage", "Samples", "p50", "p95"], rows)}
<p>Semantic recoveries: {recoveries}. Exact scans: {scans}. Eligible SQLite keys visited for exact scoring: {scanned}.</p>
<p>This corpus does not measure recovery under heavy candidate rejection. The regression fixture covers eight orphan vectors that displace four visible keys.</p>
<p>In that fixture, overfetch recovers four visible keys without an exact scan. A request for five keys still scans the four eligible SQLite keys.</p>
<p><a href="{escape(diagnostics_name or 'resource-load-diagnostics.json')}">Diagnostic JSON</a> contains each stage sample and the binary identity.</p></section>'''
    if baseline is not None and candidate is not None:
        rows = comparison_rows(baseline, candidate)
        source_revision = candidate.get("build", {}).get("source_revision")
        source_identity = (f'<p>Candidate implementation: <a href="https://github.com/bvolpato/ivygrep/commit/{escape(source_revision)}"><code>{escape(source_revision)}</code></a>.</p>'
                           if source_revision else "")
        comparison = f'''<section class="report-card"><h2>Local source comparison</h2>
<p>Both builds use Linux x86_64 with glibc. The candidate includes the resource changes.</p>
<p>The baseline starts at <code>89ae90d</code>. The candidate starts at release commit <code>0e1e393</code>. The intervening commit did not change <code>src</code>.</p>
{source_identity}
<p>These builds are separate from the released musl binary above. Values show the median and range across repetitions.</p>
<p>Search changes include faster MCP index-readiness polling. Context changes reuse readers and parsed input across requests.</p>
{table(["Profile", "Metric", "Unchanged source", "Candidate", "Change"], rows)}
<p>This is a shared-host comparison. It does not prove the same change on every repository or host.</p>
<p>Binary SHA-256: baseline <code>{escape(baseline["binary"]["sha256"])}</code>, candidate <code>{escape(candidate["binary"]["sha256"])}</code>.</p>
<p><a href="resource-load-baseline.json">Baseline JSON</a> and <a href="resource-load-candidate.json">candidate JSON</a> contain every measured sample.</p></section>'''
    limitations = "".join(f"<li>{escape(value)}</li>" for value in method["limitations"])
    return f'''<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>ivygrep resource and latency measurements</title><link rel="stylesheet" href="../style.css"><link rel="stylesheet" href="report.css"></head>
<body class="report-page"><main class="report-shell">
<nav class="report-nav"><a class="report-brand" href="index.html">Benchmark reports</a><div class="report-links"><a href="{escape(raw_name)}">Raw JSON</a></div></nav>
<section class="report-hero"><h1>Model cost and concurrent latency</h1>
<p>The released <code>{escape(report["binary"]["version"])}</code> binary ran each model profile {method["runs_per_profile"]} times on the same source corpus.</p>
<p>Each run measured enhancement, forced-neural search, and {method["clients"]} MCP clients during indexing in another workspace.</p></section>
<section class="report-card"><h2>Released binary</h2>
{model_summary}
<p>Values show the median across repetitions. Parentheses show the minimum and maximum across repetitions.</p>
{table(["Profile", "Metric", "Median (range)"], model_rows(report))}</section>
{comparison}
{diagnostic_section}
<section class="report-card"><h2>What these measurements mean</h2>
<p>Each latency path has {method["samples"]} measured samples per run. Concurrent context packs use a {method["context_budget_tokens"]}-token budget.</p>
<p>The query-result cache is disabled. Forced-neural calls verify that the neural tier ran. MCP searches use normal hybrid routing.</p>
<p>The eight queries repeat. Neural query-vector and file-preview caches keep their default behavior.</p>
<p>MCP clients start fresh. Their first calls include index-readiness polling and watcher setup. These are end-to-end tool latencies.</p>
<p>The corpus starts as a Git repository without a commit. Its files are untracked, so context packs also process current changes.</p>
<p>Peak RSS for enhancement comes from per-child <code>wait4</code>. Daemon RSS is sampled every {method["rss_sample_interval_ms"]} ms.</p>
<p>Idle RSS follows a {method["idle_seconds"]}-second pause. CPU is process CPU time. Disk writes measure filesystem I/O, including index and log writes.</p>
<p>Load resource totals describe the daemon. They exclude MCP client processes. Enhancement totals describe the invoked CLI process.</p>
<p>The background loop completes a variable number of index runs. Load CPU and write totals also reflect that count.</p>
<p>The empirical p99 uses the nearest observed rank. This sample count cannot establish a stable production tail-latency guarantee.</p>
<ul>{limitations}</ul>
<p>These measurements do not replace retrieval-quality results. The <a href="embedding-model-bakeoff.html">model screening</a> used one repetition.</p>
<p>The source corpus contains {report["corpus"]["files"]} files and {report["corpus"]["source_bytes"]} bytes from revision <code>{escape(report["corpus"]["revision"])}</code>.</p>
<p>Binary SHA-256: <code>{escape(report["binary"]["sha256"])}</code>. Corpus SHA-256: <code>{escape(report["corpus"]["sha256"])}</code>.</p>
<p>Measured at {escape(report["measured_at"])}. The JSON records the harness hashes and every repetition.</p>
<p><a href="https://github.com/bvolpato/ivygrep/blob/main/scripts/bench_resource_load.py">Benchmark runner</a> and <a href="resource-load.md">reproduction commands</a>.</p>
</section></main></body></html>
'''


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--diagnostics", type=Path)
    args = parser.parse_args()
    if bool(args.baseline) != bool(args.candidate):
        parser.error("provide both baseline and candidate")
    report = read_report(args.input)
    baseline = read_report(args.baseline) if args.baseline else None
    candidate = read_report(args.candidate) if args.candidate else None
    diagnostics = read_report(args.diagnostics) if args.diagnostics else None
    args.output.write_text(render(report, args.input.name, baseline, candidate, diagnostics,
                                  args.diagnostics.name if args.diagnostics else None))


if __name__ == "__main__":
    main()
