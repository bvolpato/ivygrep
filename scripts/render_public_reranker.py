#!/usr/bin/env python3
"""Render learned-reranker training and integrated benchmark evidence."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
from html import escape
import hashlib
import json
from pathlib import Path

import train_public_reranker as trainer


QUALITY_METRICS = ("ndcg_at_10", "mrr_at_10", "precision_at_5", "recall_at_20")
LATENCY_METRICS = ("warm_latency_p50_ms", "warm_latency_p95_ms")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def metric(matrix: dict, name: str) -> float:
    return float(matrix["summary"]["neural"]["metrics"][name]["mean"])


def integrated_evaluation(baseline_path: Path, learned_path: Path) -> tuple[dict, dict]:
    """The deterministic-against-learned comparison of two matrices, and the
    learned matrix. Nothing here depends on a model file."""
    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    learned = json.loads(learned_path.read_text(encoding="utf-8"))
    if baseline["ivygrep_commit"] != learned["ivygrep_commit"]:
        raise ValueError("integrated matrices must use the same ivygrep commit")
    if baseline["tasks"] != learned["tasks"] or baseline["queries"] != learned["queries"]:
        raise ValueError("integrated matrices must cover the same tasks and queries")
    baseline_binary = baseline["results"][0]["binary"]
    learned_binary = learned["results"][0]["binary"]
    if baseline_binary != learned_binary:
        raise ValueError("integrated matrices must use the same binary")
    baseline_modes = {
        result["index_configuration"].get("reranker_mode")
        for result in baseline["results"]
    }
    learned_modes = {
        result["index_configuration"].get("reranker_mode")
        for result in learned["results"]
    }
    if baseline_modes != {"deterministic"}:
        raise ValueError("baseline matrix did not disable the learned reranker")
    if learned_modes != {"learned"}:
        raise ValueError("learned matrix did not enable the learned reranker")

    metrics = {}
    for name in (*QUALITY_METRICS, *LATENCY_METRICS):
        before = metric(baseline, name)
        after = metric(learned, name)
        metrics[name] = {
            "deterministic": before,
            "learned": after,
            "absolute_delta": after - before,
            "relative_delta": after / before - 1.0 if before else 0.0,
        }
    tasks = {}
    for task in learned["tasks"]:
        task_metrics = {}
        for name in QUALITY_METRICS:
            before = float(baseline["task_summary"][task]["neural"][name]["mean"])
            after = float(learned["task_summary"][task]["neural"][name]["mean"])
            task_metrics[name] = {
                "deterministic": before,
                "learned": after,
                "absolute_delta": after - before,
            }
        tasks[task] = task_metrics

    quality_passed = (
        metrics["ndcg_at_10"]["relative_delta"] >= 0.05
        or metrics["mrr_at_10"]["relative_delta"] >= 0.05
    )
    task_passed = all(
        values["ndcg_at_10"]["absolute_delta"] >= -0.02
        and values["mrr_at_10"]["absolute_delta"] >= -0.02
        for values in tasks.values()
    )
    latency_passed = metrics["warm_latency_p95_ms"]["absolute_delta"] < 75.0
    return (
        {
            "profile": learned["profile"],
            "queries": learned["queries"],
            "tasks": tasks,
            "metrics": metrics,
            "gate": {
                "minimum_relative_ndcg_or_mrr_gain": 0.05,
                "maximum_absolute_task_loss": 0.02,
                "maximum_added_warm_p95_ms": 75.0,
                "quality_passed": quality_passed,
                "per_task_passed": task_passed,
                "latency_passed": latency_passed,
                "passed": quality_passed and task_passed and latency_passed,
            },
            "deterministic_evidence_sha256": sha256_file(baseline_path),
            "learned_evidence_sha256": sha256_file(learned_path),
        },
        learned,
    )


def build_report(model_path: Path, baseline_path: Path, learned_path: Path) -> dict:
    model = json.loads(model_path.read_text(encoding="utf-8"))
    model_sha256 = sha256_file(model_path)
    # A report is evidence for one ranking function. The offline record must
    # have been computed for the weights in the model file, and the learned
    # matrix must have run the bytes of that file.
    evaluation = model.get("evaluation") or {}
    if evaluation.get("weights_sha256") != trainer.model_weights_sha256(
        model["feature_schema"], model["weights"]
    ):
        raise ValueError(
            "the model's evaluation record was not computed for its weights; "
            "run train_public_reranker.py --reevaluate"
        )
    integrated, learned = integrated_evaluation(baseline_path, learned_path)
    executed = {
        result["index_configuration"].get("reranker_model_sha256")
        for result in learned["results"]
    }
    if executed != {model_sha256}:
        raise ValueError(
            "the learned matrix did not run this model file: results report "
            f"{sorted(map(str, executed))}, the file is {model_sha256}"
        )
    return {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "ivygrep_commit": learned["ivygrep_commit"],
        "binary": learned["results"][0]["binary"],
        "runtime": learned["runtime"],
        "model": {
            "model_id": model["model_id"],
            "schema_version": model["schema_version"],
            "feature_count": len(model["feature_schema"]),
            "training": model["training"],
            "offline_evaluation": evaluation,
            "sha256": model_sha256,
        },
        "integrated_evaluation": integrated,
    }


def percent(value: float) -> str:
    return f"{value * 100:+.2f}%"


def render_markdown(report: dict) -> str:
    integrated = report["integrated_evaluation"]
    metrics = integrated["metrics"]
    offline = report["model"]["offline_evaluation"]
    task_cap = (
        "every task stayed within the two-point loss cap."
        if offline["gate"]["per_task_passed"]
        else "at least one task lost more than the two-point cap."
    )
    task_rows = "\n".join(
        f"| {task} | {values['ndcg_at_10']['deterministic']:.4f} | "
        f"{values['ndcg_at_10']['learned']:.4f} | "
        f"{values['ndcg_at_10']['absolute_delta']:+.4f} | "
        f"{values['mrr_at_10']['absolute_delta']:+.4f} |"
        for task, values in integrated["tasks"].items()
    )
    return f"""# Public learned reranker

The embedded `{report['model']['model_id']}` model is trained only from pinned public
retrieval traces. The deterministic ranker remains available with
`IVYGREP_RERANKER=deterministic`.

## Integrated result

- Commit: `{report['ivygrep_commit']}`
- Binary SHA-256: `{report['binary']['sha256']}`
- Held-out queries: {integrated['queries']}
- nDCG@10: {metrics['ndcg_at_10']['deterministic']:.4f} -> {metrics['ndcg_at_10']['learned']:.4f} ({percent(metrics['ndcg_at_10']['relative_delta'])})
- MRR@10: {metrics['mrr_at_10']['deterministic']:.4f} -> {metrics['mrr_at_10']['learned']:.4f} ({percent(metrics['mrr_at_10']['relative_delta'])})
- Warm p50 delta: {metrics['warm_latency_p50_ms']['absolute_delta']:+.2f} ms
- Warm p95 delta: {metrics['warm_latency_p95_ms']['absolute_delta']:+.2f} ms
- Acceptance gate: **{'PASS' if integrated['gate']['passed'] else 'FAIL'}**

## Per-task quality

| Task | deterministic nDCG@10 | learned nDCG@10 | nDCG delta | MRR delta |
| --- | ---: | ---: | ---: | ---: |
{task_rows}

## Offline transfer check

The model artifact also records {offline['queries']} held-out public queries
across {len(offline['tasks'])} tasks, evaluated for the weights it ships. Aggregate
nDCG@10 changed by {percent(offline['relative_ndcg'])} and MRR@10 by
{percent(offline['relative_mrr'])}; {task_cap}
The offline acceptance gate {'passed' if offline['gate']['passed'] else 'failed'}.

Raw evidence: [`public-reranker-results.json`](public-reranker-results.json),
[`public-reranker-deterministic-results.json`](public-reranker-deterministic-results.json),
and [`public-reranker-learned-results.json`](public-reranker-learned-results.json).
"""


def render_html(report: dict) -> str:
    integrated = report["integrated_evaluation"]
    metrics = integrated["metrics"]
    task_rows = "".join(
        "<tr>"
        f"<td>{escape(task)}</td>"
        f"<td>{values['ndcg_at_10']['deterministic']:.4f}</td>"
        f"<td>{values['ndcg_at_10']['learned']:.4f}</td>"
        f"<td>{values['ndcg_at_10']['absolute_delta']:+.4f}</td>"
        f"<td>{values['mrr_at_10']['absolute_delta']:+.4f}</td>"
        "</tr>"
        for task, values in integrated["tasks"].items()
    )
    gate = "PASS" if integrated["gate"]["passed"] else "FAIL"
    version = str(report.get("binary", {}).get("version", "")).removeprefix("ivygrep ").strip()
    evidence_label = (
        f"Historical regression evidence (v{version})" if version else "Historical regression evidence"
    )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Public learned reranker - ivygrep</title>
  <meta name="description" content="Historical learned-reranker regression results on public-core CoIR tasks.">
  <link rel="canonical" href="https://bvolpato.github.io/ivygrep/benchmarks/public-reranker.html">
  <link rel="stylesheet" href="../style.css">
  <link rel="stylesheet" href="report.css">
  <link rel="icon" type="image/svg+xml" href="../assets/icon.svg">
</head>
<body class="report-page">
  <main class="report-shell relative z-10">
    <nav class="report-nav"><a class="report-brand" href="index.html"><img src="../assets/icon.svg" alt="ivygrep"><span>ivygrep benchmarks</span></a><div class="report-links"><a href="index.html">Reports</a><a href="public-reranker-results.json">Raw JSON</a><a href="https://github.com/bvolpato/ivygrep">GitHub</a></div></nav>
    <section class="report-hero"><div class="report-eyebrow">{escape(evidence_label)}</div><h1>Bounded Learned Reranker</h1><p>A 41-feature linear model trained from pinned public traces, embedded in the binary with a deterministic fallback.</p></section>
    <section class="report-grid">
      <article class="report-card"><h2>nDCG@10</h2><div class="metric-value">{percent(metrics['ndcg_at_10']['relative_delta'])}</div><p>{metrics['ndcg_at_10']['deterministic']:.4f} -> {metrics['ndcg_at_10']['learned']:.4f}</p></article>
      <article class="report-card"><h2>MRR@10</h2><div class="metric-value">{percent(metrics['mrr_at_10']['relative_delta'])}</div><p>{metrics['mrr_at_10']['deterministic']:.4f} -> {metrics['mrr_at_10']['learned']:.4f}</p></article>
      <article class="report-card"><h2>Acceptance</h2><div class="metric-value">{gate}</div><p>Quality, per-task loss, and latency gates.</p></article>
    </section>
    <section class="report-card"><h2>Per-task quality</h2><div class="table-wrap"><table><thead><tr><th>Task</th><th>Deterministic nDCG</th><th>Learned nDCG</th><th>nDCG delta</th><th>MRR delta</th></tr></thead><tbody>{task_rows}</tbody></table></div></section>
    <section class="report-card"><h2>Latency and portability</h2><p>Warm p50 delta: {metrics['warm_latency_p50_ms']['absolute_delta']:+.2f} ms. Warm p95 delta: {metrics['warm_latency_p95_ms']['absolute_delta']:+.2f} ms. Set <code>IVYGREP_RERANKER=deterministic</code> to retain the zero-model fallback.</p></section>
    <section class="report-card"><h2>Identity</h2><p>Commit <code>{report['ivygrep_commit']}</code><br>Binary <code>{report['binary']['sha256']}</code><br>Model <code>{report['model']['model_id']}</code></p></section>
  </main>
</body>
</html>
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--deterministic", type=Path, required=True)
    parser.add_argument("--learned", type=Path, required=True)
    parser.add_argument("--json", type=Path, required=True)
    parser.add_argument("--html", type=Path, required=True)
    args = parser.parse_args()
    report = build_report(args.model, args.deterministic, args.learned)
    args.json.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    args.html.write_text(render_html(report), encoding="utf-8")
    if not report["integrated_evaluation"]["gate"]["passed"]:
        raise SystemExit("learned reranker acceptance gate failed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
