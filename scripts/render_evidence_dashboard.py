#!/usr/bin/env python3
"""Render the evidence dashboard from retained machine-readable artifacts."""

from __future__ import annotations

import argparse
from html import escape
import hashlib
import json
from pathlib import Path
import subprocess


REPOSITORY = "https://github.com/bvolpato/ivygrep"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def publication_commit(root: Path, path: Path) -> str:
    result = subprocess.run(
        ["git", "log", "-1", "--format=%H", "--", str(path.relative_to(root))],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    commit = result.stdout.strip()
    if not commit:
        raise ValueError(f"{path} has no publication commit")
    return commit


def metric_stats(summary: dict, mode: str, metric: str) -> dict | None:
    value = summary.get(mode, {}).get("metrics", {}).get(metric)
    if not isinstance(value, dict):
        return None
    return {
        key: value.get(key)
        for key in (
            "mean",
            "standard_deviation",
            "coefficient_of_variation",
            "minimum",
            "maximum",
        )
    }


def mean_metric(summary: dict, mode: str, metric: str) -> float | None:
    value = metric_stats(summary, mode, metric)
    return value.get("mean") if value else None


def daemon_summary(path: Path) -> dict:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or line.startswith("iteration\t"):
            continue
        fields = line.split("\t")
        if len(fields) >= 7 and fields[5] in {"baseline", "keep"}:
            rows.append((fields[1], float(fields[2])))
    return {
        "source_commit": rows[-1][0] if rows else None,
        "retained_warm_p95_ms": rows[-1][1] if rows else None,
        "retained_iterations": max(0, len(rows) - 1),
    }


def summarize(evidence_id: str, path: Path) -> tuple[str | None, dict]:
    if path.suffix == ".tsv":
        summary = daemon_summary(path)
        return summary.pop("source_commit"), summary
    if path.suffix not in {".json"}:
        text = path.read_text(encoding="utf-8")
        targets = {
            target
            for target in (
                "x86_64-unknown-linux-musl",
                "aarch64-unknown-linux-musl",
                "x86_64-apple-darwin",
                "aarch64-apple-darwin",
                "x86_64-pc-windows-msvc",
            )
            if target in text
        }
        required_tokens = (
            "artifact-acceptance:",
            "needs: artifact-acceptance",
            "scripts/verify_release_artifact.py",
            "scripts/e2e_x86_baseline.sh",
            "scripts/e2e_cached_model.sh",
        )
        return None, {
            "artifact_acceptance": all(token in text for token in required_tokens),
            "release_targets": len(targets),
            "sbom": "anchore/sbom-action@" in text,
            "provenance": "actions/attest@" in text,
        }
    document = json.loads(path.read_text(encoding="utf-8"))
    source_commit = document.get("ivygrep_commit")
    if evidence_id.startswith("public-retrieval"):
        mode = "neural" if "neural" in document.get("summary", {}) else "hybrid"
        summary = document.get("summary", {})
        modes = {
            mode_name: {
                metric: metric_stats(summary, mode_name, metric)
                for metric in (
                    "ndcg_at_10",
                    "mrr_at_10",
                    "warm_latency_p95_ms",
                    "index_ms",
                    "index_size_bytes",
                    "peak_child_rss_bytes",
                )
            }
            for mode_name in summary
        }
        return source_commit, {
            "profile": document.get("profile"),
            "queries": document.get("queries"),
            "repetitions": document.get("repetitions"),
            "mode": mode,
            "modes": modes,
            "tasks": document.get("tasks"),
            "runtime": document.get("runtime"),
            "models": document.get("neural_models", []),
            "ndcg_at_10": mean_metric(summary, mode, "ndcg_at_10"),
            "mrr_at_10": mean_metric(summary, mode, "mrr_at_10"),
            "warm_latency_p95_ms": mean_metric(
                summary, mode, "warm_latency_p95_ms"
            ),
            "index_size_bytes": mean_metric(summary, mode, "index_size_bytes"),
            "peak_child_rss_bytes": mean_metric(
                summary, mode, "peak_child_rss_bytes"
            ),
        }
    if evidence_id == "embedding-selection":
        return source_commit, {
            "selection": document.get("selection"),
            "candidates": len(document.get("candidates", [])),
        }
    if evidence_id == "learned-reranker":
        evaluation = document["integrated_evaluation"]
        return source_commit, {
            "passed": evaluation["gate"]["passed"],
            "queries": evaluation["queries"],
            "ndcg_at_10_delta": evaluation["metrics"]["ndcg_at_10"][
                "absolute_delta"
            ],
            "mrr_at_10_delta": evaluation["metrics"]["mrr_at_10"][
                "absolute_delta"
            ],
            "warm_p95_delta_ms": evaluation["metrics"]["warm_latency_p95_ms"][
                "absolute_delta"
            ],
        }
    if evidence_id == "million-scale":
        quality = document["quality"]
        indexing = document["indexing"]
        paired = document["paired_queries"]
        return quality.get("current_commit"), {
            "passed": document["gate"]["passed"],
            "queries": quality["queries"],
            "ndcg_at_10": quality["metrics"]["ndcg_at_10"]["current"],
            "ndcg_at_10_delta": quality["metrics"]["ndcg_at_10"][
                "absolute_delta"
            ],
            "warm_p95_ms": paired["current_p95_ms"],
            "warm_speedup": paired["speedup"],
            "index_size_bytes": indexing["index_size_bytes"]["current"],
            "index_size_ratio": indexing["index_size_bytes"]["ratio"],
            "peak_rss_bytes": indexing["peak_rss_bytes"]["current"],
            "peak_disk_bytes": indexing.get("peak_disk_bytes", {}).get("current"),
            "baseline": {
                "quality_commit": quality["baseline_commit"],
                "latency_commit": paired["baseline_commit"],
                "index_commit": indexing["baseline_commit"],
                "ndcg_at_10": quality["metrics"]["ndcg_at_10"]["baseline"],
                "warm_p95_ms": paired["baseline_p95_ms"],
                "index_size_bytes": indexing["index_size_bytes"]["baseline"],
                "peak_rss_bytes": indexing["peak_rss_bytes"]["baseline"],
                "peak_disk_bytes": indexing.get("peak_disk_bytes", {}).get(
                    "baseline"
                ),
                "chunks_per_second": indexing["chunks_per_second"]["baseline"],
            },
            "current": {
                "quality_commit": quality["current_commit"],
                "latency_commit": paired["current_commit"],
                "index_commit": indexing["current_commit"],
                "ndcg_at_10": quality["metrics"]["ndcg_at_10"]["current"],
                "warm_p95_ms": paired["current_p95_ms"],
                "index_size_bytes": indexing["index_size_bytes"]["current"],
                "peak_rss_bytes": indexing["peak_rss_bytes"]["current"],
                "peak_disk_bytes": indexing.get("peak_disk_bytes", {}).get(
                    "current"
                ),
                "chunks_per_second": indexing["chunks_per_second"]["current"],
            },
            "quality_variance": {
                "baseline_standard_deviation": quality["metrics"]["ndcg_at_10"].get(
                    "baseline_standard_deviation"
                ),
                "current_standard_deviation": quality["metrics"]["ndcg_at_10"].get(
                    "current_standard_deviation"
                ),
            },
            "latency_variance": paired["p95_ratio"],
            "runtime": document.get("runtime"),
            "binaries": document.get("binaries"),
            "corpus": document.get("corpus"),
            "model": quality.get("model"),
            "repetitions": quality.get("repetitions"),
            "manifest_sha256": quality.get("manifest_sha256"),
            "harness_sha256": quality.get("harness_sha256"),
        }
    return source_commit, {}


def evidence_point(
    item: dict,
    *,
    series: str,
    value: float | int | None,
    unit: str,
    variance: dict | None,
    context: dict,
    source_commit: str | None = None,
) -> dict:
    return {
        "series": series,
        "value": value,
        "unit": unit,
        "variance": variance
        or {
            "status": "unavailable",
            "reason": "The retained artifact is a single deterministic run.",
        },
        "context": context,
        "source_commit": source_commit or item.get("source_commit"),
        "publication_commit": item["publication_commit"],
        "evidence_id": item["id"],
        "immutable_url": item["immutable_url"],
    }


def build_histories(evidence: list[dict], release_history: dict) -> dict:
    by_id = {item["id"]: item for item in evidence}
    million_item = by_id["million-scale"]
    million = million_item["summary"]
    runtime = million.get("runtime")
    common_context = {
        "hardware": runtime,
        "corpus": million.get("corpus"),
        "model": million.get("model"),
        "repetitions": million.get("repetitions"),
        "binaries": million.get("binaries"),
        "manifest_sha256": million.get("manifest_sha256"),
        "harness_sha256": million.get("harness_sha256"),
    }
    baseline = million["baseline"]
    current = million["current"]
    quality_variance = million["quality_variance"]
    histories = {
        "quality": [
            evidence_point(
                million_item,
                series="semantic-retrieval/neural/ndcg_at_10",
                value=baseline["ndcg_at_10"],
                unit="score",
                variance={
                    "standard_deviation": quality_variance[
                        "baseline_standard_deviation"
                    ]
                },
                context=common_context,
                source_commit=baseline["quality_commit"],
            ),
            evidence_point(
                million_item,
                series="semantic-retrieval/neural/ndcg_at_10",
                value=current["ndcg_at_10"],
                unit="score",
                variance={
                    "standard_deviation": quality_variance[
                        "current_standard_deviation"
                    ]
                },
                context=common_context,
                source_commit=current["quality_commit"],
            ),
        ],
        "latency": [
            evidence_point(
                million_item,
                series="million-chunk/warm-distinct-p95",
                value=baseline["warm_p95_ms"],
                unit="ms",
                variance=million["latency_variance"],
                context=common_context,
                source_commit=baseline["latency_commit"],
            ),
            evidence_point(
                million_item,
                series="million-chunk/warm-distinct-p95",
                value=current["warm_p95_ms"],
                unit="ms",
                variance=million["latency_variance"],
                context=common_context,
                source_commit=current["latency_commit"],
            ),
        ],
        "indexing": [],
        "memory": [],
        "index_size": [],
        "binary_size": [],
        "archive_size": [],
    }
    for name, key, unit, history_name in (
        ("million-chunk/chunks-per-second", "chunks_per_second", "chunks/s", "indexing"),
        ("million-chunk/peak-rss", "peak_rss_bytes", "bytes", "memory"),
        ("million-chunk/final-index", "index_size_bytes", "bytes", "index_size"),
    ):
        for version in (baseline, current):
            histories[history_name].append(
                evidence_point(
                    million_item,
                    series=name,
                    value=version[key],
                    unit=unit,
                    variance=None,
                    context=common_context,
                    source_commit=version["index_commit"],
                )
            )

    for release in release_history["releases"]:
        for archive in release["archives"]:
            context = {
                "tag": release["tag"],
                "published_at": release["published_at"],
                "target": archive["target"],
                "release_url": release["release_url"],
            }
            histories["archive_size"].append(
                {
                    "series": f"release/{archive['target']}",
                    "value": archive["size_bytes"],
                    "unit": "bytes",
                    "variance": {"status": "not-applicable"},
                    "context": context,
                    "immutable_url": archive["download_url"],
                }
            )
            histories["binary_size"].append(
                {
                    "series": f"release/{archive['target']}",
                    "value": archive.get("binary_size_bytes"),
                    "unit": "bytes",
                    "variance": {"status": "not-applicable"},
                    "status": (
                        "available"
                        if archive.get("binary_size_bytes") is not None
                        else "unavailable"
                    ),
                    "context": context,
                    "immutable_url": archive["download_url"],
                }
            )
    return histories


def build_dashboard(root: Path, manifest_path: Path, claims_path: Path) -> dict:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    claims = json.loads(claims_path.read_text(encoding="utf-8"))
    evidence = []
    for item in manifest["evidence"]:
        path = root / item["path"]
        source_commit, summary = summarize(item["id"], path)
        published = publication_commit(root, path)
        evidence.append(
            {
                **item,
                "sha256": sha256_file(path),
                "source_commit": source_commit,
                "publication_commit": published,
                "immutable_url": f"{REPOSITORY}/blob/{published}/{item['path']}",
                "summary": summary,
            }
        )

    release_history_path = root / manifest["release_history"]
    release_history = json.loads(
        release_history_path.read_text(encoding="utf-8")
    )
    release_history_commit = publication_commit(root, release_history_path)
    by_id = {item["id"]: item for item in evidence}
    million = by_id["million-scale"]["summary"]
    release_gate = by_id["release-workflow"]["summary"]["artifact_acceptance"]
    portability = claims["claims"]["portable"]
    portable_supported = (
        portability["status"] == "qualified"
        and release_gate
        and by_id["release-workflow"]["summary"]["release_targets"]
        == portability["required_release_targets"]
        and by_id["release-workflow"]["summary"]["sbom"]
        and by_id["release-workflow"]["summary"]["provenance"]
    )
    pareto_advantage = (
        million.get("warm_speedup", 0) >= 2.0
        or million.get("index_size_ratio", 1.0) <= 0.60
    )
    top_tier_quality = any(
        comparison["class"] == "semantic-retrieval"
        and comparison["status"] == "available"
        and comparison.get("top_tier", False)
        for comparison in claims["comparisons"]
    )
    claim_status = {
        "portable": {
            **portability,
            "supported": portable_supported,
        },
        "competitive": {
            **claims["claims"]["competitive"],
            "supported": False,
        },
        "state_of_the_art": {
            **claims["claims"]["state_of_the_art"],
            "pareto_advantage": pareto_advantage,
            "top_tier_public_quality": top_tier_quality,
            "supported": pareto_advantage and top_tier_quality,
        },
    }
    return {
        "schema_version": 1,
        "evidence": evidence,
        "release_history": release_history,
        "release_history_artifact": {
            "path": manifest["release_history"],
            "sha256": sha256_file(release_history_path),
            "publication_commit": release_history_commit,
            "immutable_url": (
                f"{REPOSITORY}/blob/{release_history_commit}/"
                f"{manifest['release_history']}"
            ),
        },
        "histories": build_histories(evidence, release_history),
        "comparisons": claims["comparisons"],
        "claims": claim_status,
    }


def format_number(value: float | int | None, digits: int = 2) -> str:
    return "unavailable" if value is None else f"{value:.{digits}f}"


def format_history_value(point: dict) -> str:
    value = point.get("value")
    if value is None:
        return "unavailable"
    if point["unit"] == "bytes":
        return f"{value / (1024 * 1024):.2f} MiB"
    if point["unit"] == "score":
        return f"{value:.4f}"
    return f"{value:.2f} {point['unit']}"


def format_variance(variance: dict) -> str:
    if variance.get("standard_deviation") is not None:
        return f"sd {variance['standard_deviation']:.4f}"
    if variance.get("ci95_lower") is not None:
        return (
            f"ratio CI95 {variance['ci95_lower']:.3f}-"
            f"{variance['ci95_upper']:.3f}"
        )
    status = variance.get("status")
    if status:
        return status
    return "recorded"


def render_markdown(dashboard: dict) -> str:
    by_id = {item["id"]: item for item in dashboard["evidence"]}
    retrieval = by_id["public-retrieval-compact-current"]["summary"]
    million = by_id["million-scale"]["summary"]
    reranker = by_id["learned-reranker"]["summary"]
    daemon = by_id["daemon-cache"]["summary"]
    claims = dashboard["claims"]
    releases = dashboard["release_history"]["releases"]
    latest_release = releases[0] if releases else None
    release_text = (
        f"{latest_release['tag']} with "
        f"{len(latest_release['archives'])} archives"
        if latest_release
        else "unavailable"
    )
    comparison_rows = "\n".join(
        f"| {item['class']} | {item['status']} | {item['reason']} |"
        for item in dashboard["comparisons"]
    )
    history_rows = "\n".join(
        f"| {name.replace('_', ' ')} | {len(points)} | "
        f"{sum(point.get('status') == 'unavailable' for point in points)} |"
        for name, points in dashboard["histories"].items()
    )
    evidence_links = "\n".join(
        f"- [{item['label']}]({item['immutable_url']}) "
        f"(`{item['sha256'][:16]}...`)"
        for item in dashboard["evidence"]
    )
    release_history_artifact = dashboard["release_history_artifact"]
    evidence_links += (
        "\n- [Release artifact history]"
        f"({release_history_artifact['immutable_url']}) "
        f"(`{release_history_artifact['sha256'][:16]}...`)"
    )
    point_rows = []
    for family, points in dashboard["histories"].items():
        for point in points:
            revision = (
                point.get("source_commit")
                or point.get("context", {}).get("tag")
                or "unavailable"
            )
            point_rows.append(
                f"| {family.replace('_', ' ')} | {point['series']} | "
                f"{revision[:12]} | {format_history_value(point)} | "
                f"{format_variance(point['variance'])} | "
                f"[source]({point['immutable_url']}) |"
            )
    rendered_points = "\n".join(point_rows)
    return f"""# Evidence dashboard

This page is generated from retained machine-readable benchmark and release
artifacts. Every evidence link is pinned to the commit that published its bytes.

| Area | Latest retained result |
|---|---|
| Public neural retrieval | nDCG@10 {format_number(retrieval['ndcg_at_10'], 4)}, MRR@10 {format_number(retrieval['mrr_at_10'], 4)}, {retrieval['queries']} queries x {retrieval['repetitions']} runs |
| Learned reranker | gate {"passed" if reranker["passed"] else "failed"}, nDCG@10 delta {format_number(reranker["ndcg_at_10_delta"], 4)} |
| Million-chunk latency | {format_number(million["warm_p95_ms"])} ms warm p95, {format_number(million["warm_speedup"])}x baseline |
| Million-chunk footprint | {million["index_size_bytes"]} bytes, ratio {format_number(million["index_size_ratio"], 3)} |
| Daemon cache | {format_number(daemon["retained_warm_p95_ms"])} ms retained warm p95 |
| Release archive history | {release_text} |

## Versioned histories

| Metric family | Retained points | Unavailable points |
|---|---:|---:|
{history_rows}

Each point in `evidence-dashboard.json` includes its unit, comparison series,
hardware/corpus/model context, variance or an explicit variance-unavailable
reason, source commit, and immutable artifact URL.

| Family | Comparable series | Revision/tag | Value | Variance | Artifact |
|---|---|---|---:|---|---|
{rendered_points}

## Claim status

- Portable: **{"supported" if claims["portable"]["supported"] else "not supported"}** under the qualified five-target artifact definition.
- Competitive: **not claimed** without a controlled comparable-system result.
- State of the art: **not claimed**. Pareto evidence is
  {"present" if claims["state_of_the_art"]["pareto_advantage"] else "absent"},
  while a top-tier comparable public result is
  {"present" if claims["state_of_the_art"]["top_tier_public_quality"] else "unavailable"}.

## Comparable-system evidence

| Class | Status | Reason |
|---|---|---|
{comparison_rows}

Regressions and unavailable comparisons remain listed; the renderer never
deletes them to improve the presentation.

## Immutable source artifacts

{evidence_links}

Raw machine-readable dashboard:
[`evidence-dashboard.json`](evidence-dashboard.json).
"""


def render_html(dashboard: dict, markdown: str) -> str:
    evidence_rows = "".join(
        "<tr>"
        f"<td>{escape(item['label'])}</td>"
        f"<td>{escape(item['kind'])}</td>"
        f"<td><a href=\"{escape(item['immutable_url'])}\">{escape(item['publication_commit'][:12])}</a></td>"
        f"<td><code>{escape(item['sha256'][:16])}</code></td>"
        "</tr>"
        for item in dashboard["evidence"]
    )
    release_history_artifact = dashboard["release_history_artifact"]
    evidence_rows += (
        "<tr><td>Release artifact history</td><td>release</td>"
        f"<td><a href=\"{escape(release_history_artifact['immutable_url'])}\">"
        f"{escape(release_history_artifact['publication_commit'][:12])}</a></td>"
        f"<td><code>{escape(release_history_artifact['sha256'][:16])}</code></td>"
        "</tr>"
    )
    comparison_rows = "".join(
        "<tr>"
        f"<td>{escape(item['class'])}</td>"
        f"<td>{escape(item['status'])}</td>"
        f"<td>{escape(item['reason'])}</td>"
        "</tr>"
        for item in dashboard["comparisons"]
    )
    history_rows = "".join(
        "<tr>"
        f"<td>{escape(name.replace('_', ' '))}</td>"
        f"<td>{len(points)}</td>"
        f"<td>{sum(point.get('status') == 'unavailable' for point in points)}</td>"
        "</tr>"
        for name, points in dashboard["histories"].items()
    )
    point_rows = []
    for family, points in dashboard["histories"].items():
        for point in points:
            revision = (
                point.get("source_commit")
                or point.get("context", {}).get("tag")
                or "unavailable"
            )
            point_rows.append(
                "<tr>"
                f"<td>{escape(family.replace('_', ' '))}</td>"
                f"<td>{escape(point['series'])}</td>"
                f"<td>{escape(revision[:12])}</td>"
                f"<td>{escape(format_history_value(point))}</td>"
                f"<td>{escape(format_variance(point['variance']))}</td>"
                f"<td><a href=\"{escape(point['immutable_url'])}\">source</a></td>"
                "</tr>"
            )
    rendered_points = "".join(point_rows)
    claims = dashboard["claims"]
    million = next(
        item["summary"]
        for item in dashboard["evidence"]
        if item["id"] == "million-scale"
    )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>ivygrep evidence dashboard</title>
  <meta name="description" content="Immutable quality, latency, footprint, portability, and release evidence for ivygrep.">
  <link rel="stylesheet" href="../style.css">
  <link rel="stylesheet" href="report.css">
</head>
<body class="report-page">
  <main class="report-shell relative z-10">
    <nav class="report-nav"><a class="report-brand" href="index.html">ivygrep benchmarks</a><div class="report-links"><a href="evidence-dashboard.json">Raw JSON</a><a href="claims-policy.md">Claim policy</a></div></nav>
    <section class="report-hero"><div class="report-eyebrow">Generated evidence</div><h1>Claim dashboard</h1><p>Commit-pinned artifacts, visible regressions, and explicit unavailable comparisons.</p></section>
    <section class="report-grid">
      <article class="report-card"><h2>Warm speedup</h2><div class="metric-value">{million['warm_speedup']:.2f}x</div></article>
      <article class="report-card"><h2>Index footprint</h2><div class="metric-value">{(1.0 - million['index_size_ratio']) * 100:.1f}% smaller</div></article>
      <article class="report-card"><h2>Portable</h2><div class="metric-value">{"qualified" if claims['portable']['supported'] else "not proven"}</div></article>
      <article class="report-card"><h2>SOTA</h2><div class="metric-value">{"supported" if claims['state_of_the_art']['supported'] else "not claimed"}</div></article>
    </section>
    <section class="report-card"><h2>Immutable evidence</h2><div class="table-wrap"><table><thead><tr><th>Evidence</th><th>Kind</th><th>Publication</th><th>SHA-256</th></tr></thead><tbody>{evidence_rows}</tbody></table></div></section>
    <section class="report-card"><h2>Versioned histories</h2><p>Every point carries unit, context, variance status, and immutable source metadata in the raw JSON.</p><div class="table-wrap"><table><thead><tr><th>Metric family</th><th>Points</th><th>Unavailable</th></tr></thead><tbody>{history_rows}</tbody></table></div></section>
    <section class="report-card"><h2>History points</h2><div class="table-wrap"><table><thead><tr><th>Family</th><th>Comparable series</th><th>Revision/tag</th><th>Value</th><th>Variance</th><th>Artifact</th></tr></thead><tbody>{rendered_points}</tbody></table></div></section>
    <section class="report-card"><h2>Comparable systems</h2><div class="table-wrap"><table><thead><tr><th>Class</th><th>Status</th><th>Reason</th></tr></thead><tbody>{comparison_rows}</tbody></table></div></section>
  </main>
  <!-- Markdown source length: {len(markdown)} -->
</body>
</html>
"""


def render_policy(dashboard: dict) -> str:
    claims = dashboard["claims"]
    return f"""# Evidence claim policy

The dashboard applies these definitions mechanically:

- **Portable:** {claims['portable']['definition']}
- **Competitive:** {claims['competitive']['definition']}
- **State of the art:** {claims['state_of_the_art']['definition']}

Current status:

- Portable: {"supported" if claims['portable']['supported'] else "not supported"}
- Competitive: not claimed
- State of the art: not claimed

Exact search is compared only with exact-search systems. Semantic retrieval is
compared only with local semantic systems under a comparable corpus, model
budget, hardware class, and resource budget. Missing comparisons remain
`unavailable`; they are not inferred from unrelated benchmark numbers.
"""


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=root / "benchmarks" / "evidence" / "manifest.json",
    )
    parser.add_argument(
        "--claims",
        type=Path,
        default=root / "benchmarks" / "evidence" / "claims.json",
    )
    parser.add_argument(
        "--json",
        type=Path,
        default=root / "docs" / "benchmarks" / "evidence-dashboard.json",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        default=root / "docs" / "benchmarks" / "evidence-dashboard.md",
    )
    parser.add_argument(
        "--html",
        type=Path,
        default=root / "docs" / "benchmarks" / "evidence-dashboard.html",
    )
    parser.add_argument(
        "--policy",
        type=Path,
        default=root / "docs" / "benchmarks" / "claims-policy.md",
    )
    args = parser.parse_args()
    dashboard = build_dashboard(root, args.manifest, args.claims)
    markdown = render_markdown(dashboard)
    for path in (args.json, args.markdown, args.html, args.policy):
        path.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(
        json.dumps(dashboard, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    args.markdown.write_text(markdown, encoding="utf-8")
    args.html.write_text(render_html(dashboard, markdown), encoding="utf-8")
    args.policy.write_text(render_policy(dashboard), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
