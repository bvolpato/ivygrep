#!/usr/bin/env python3
"""Render embedding candidate matrices into public machine-readable evidence."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
from html import escape
import hashlib
import json
from pathlib import Path
import subprocess


METRICS = (
    "ndcg_at_10",
    "mrr_at_10",
    "recall_at_20",
    "warm_latency_p95_ms",
    "neural_enhancement_ms",
    "peak_child_rss_bytes",
    "index_size_bytes",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_revision(root: Path) -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()


def parse_evidence(values: list[str]) -> dict[str, Path]:
    evidence = {}
    for value in values:
        profile, separator, path = value.partition("=")
        if not separator or not profile or not path:
            raise ValueError(f"expected PROFILE=PATH, got {value!r}")
        evidence[profile] = Path(path)
    return evidence


def matrix_candidate(profile: str, path: Path, metadata: dict) -> dict:
    matrix = json.loads(path.read_text(encoding="utf-8"))
    models = matrix.get("neural_models", [])
    if len(models) != 1 or models[0].get("profile") != profile:
        raise ValueError(f"{path}: expected neural profile {profile}")
    metrics = matrix["summary"]["neural"]["metrics"]
    return {
        **metadata,
        "profile": profile,
        "model": models[0],
        "evaluation": "complete-screening-matrix",
        "queries": matrix["queries"],
        "tasks": matrix["tasks"],
        "repetitions": matrix["repetitions"],
        "metrics": {name: metrics[name] for name in METRICS},
        "task_ndcg_at_10": {
            task: matrix["task_summary"][task]["neural"]["ndcg_at_10"]
            for task in matrix["tasks"]
        },
        "binary": matrix["results"][0]["binary"],
        "ivygrep_commit": matrix["ivygrep_commit"],
        "harness_sha256": matrix["harness_sha256"],
        "evidence_sha256": sha256_file(path),
    }


def partial_candidate(
    profile: str,
    path: Path,
    metadata: dict,
    ivygrep_commit: str,
    evaluator_sha256: str,
) -> dict:
    result = json.loads(path.read_text(encoding="utf-8"))
    model = result["index_configuration"]["neural_model"]
    if model.get("profile") != profile:
        raise ValueError(f"{path}: expected neural profile {profile}")
    return {
        **metadata,
        "profile": profile,
        "model": model,
        "evaluation": "resource-stop-after-one-task",
        "queries": result["queries"],
        "tasks": [result["dataset"]],
        "repetitions": 1,
        "metrics": {
            name: {"mean": result[name]}
            for name in METRICS
        },
        "task_ndcg_at_10": {
            result["dataset"]: {
                "mean": result["ndcg_at_10"],
                "standard_deviation": 0.0,
                "coefficient_of_variation": 0.0,
                "minimum": result["ndcg_at_10"],
                "maximum": result["ndcg_at_10"],
            }
        },
        "binary": result["binary"],
        "ivygrep_commit": ivygrep_commit,
        "harness_sha256": {
            "eval_code_retrieval.py": evaluator_sha256,
        },
        "evidence_sha256": sha256_file(path),
    }


def build_report(
    root: Path,
    manifest: dict,
    matrices: dict[str, Path],
    partials: dict[str, Path],
) -> dict:
    candidates = []
    evaluated = set(matrices) | set(partials)
    for profile, path in matrices.items():
        candidates.append(
            matrix_candidate(profile, path, manifest["candidates"][profile])
        )
    selected = next(
        (
            candidate
            for candidate in candidates
            if candidate["status"] == "selected-default"
        ),
        None,
    )
    if selected is None:
        raise ValueError("the selected default must have a complete screening matrix")
    evidence_commit = selected["ivygrep_commit"]
    binary_sha256 = selected["binary"]["sha256"]
    evaluator_sha256 = selected["harness_sha256"]["eval_code_retrieval.py"]
    current_evaluator_sha256 = sha256_file(root / "scripts" / "eval_code_retrieval.py")
    if current_evaluator_sha256 != evaluator_sha256:
        raise ValueError("partial evidence evaluator does not match the selected matrix")
    for profile, path in partials.items():
        candidate = partial_candidate(
            profile,
            path,
            manifest["candidates"][profile],
            evidence_commit,
            evaluator_sha256,
        )
        if candidate["binary"]["sha256"] != binary_sha256:
            raise ValueError(f"{path}: binary does not match the selected matrix")
        candidates.append(candidate)
    for candidate in candidates:
        if candidate["ivygrep_commit"] != evidence_commit:
            raise ValueError("evaluated candidates must use one ivygrep commit")
        if candidate["binary"]["sha256"] != binary_sha256:
            raise ValueError("evaluated candidates must use one ivygrep binary")
    for name, metadata in manifest["candidates"].items():
        if name not in evaluated:
            candidates.append({**metadata, "profile": name, "evaluation": "not-run"})
    candidates.sort(
        key=lambda item: (
            item["status"] != "selected-default",
            item["status"] == "excluded",
            item["profile"],
        )
    )
    return {
        "schema_version": 2,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "ivygrep_commit": evidence_commit,
        "renderer_commit": git_revision(root),
        "binary_sha256": binary_sha256,
        "screening_budget": manifest["screening_budget"],
        "selection": "static-retrieval-v1",
        "candidates": candidates,
    }


def format_bytes(value: float) -> str:
    size = float(value)
    for unit in ("B", "KiB", "MiB", "GiB"):
        if size < 1024 or unit == "GiB":
            return f"{size:.2f} {unit}"
        size /= 1024
    raise AssertionError("unreachable")


def markdown(report: dict) -> str:
    lines = [
        "# Portable embedding model bake-off",
        "",
        "Generated from pinned public CoIR samples. No private corpus, local path, "
        "hostname, query text, or source text is retained.",
        "",
        f"- Commit: `{report['ivygrep_commit']}`",
        f"- Binary SHA-256: `{report['binary_sha256']}`",
        f"- Selected default: `{report['selection']}`",
        "",
        "| Profile | Status | nDCG@10 | MRR@10 | R@20 | Warm p95 | Neural build | Peak RSS | Index size |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for candidate in report["candidates"]:
        metrics = candidate.get("metrics")
        if metrics:
            values = (
                f"{metrics['ndcg_at_10']['mean']:.4f}",
                f"{metrics['mrr_at_10']['mean']:.4f}",
                f"{metrics['recall_at_20']['mean']:.4f}",
                f"{metrics['warm_latency_p95_ms']['mean']:.2f} ms",
                f"{metrics['neural_enhancement_ms']['mean']:.2f} ms",
                format_bytes(metrics["peak_child_rss_bytes"]["mean"]),
                format_bytes(metrics["index_size_bytes"]["mean"]),
            )
        else:
            values = ("-",) * 7
        lines.append(
            "| "
            + " | ".join(
                (candidate["profile"], candidate["status"], *values)
            )
            + " |"
        )
    lines.extend(
        (
            "",
            "## Decision",
            "",
            "The static retrieval profile is the portable Pareto winner and the only "
            "candidate promoted through the complete screening matrix. Transformer "
            "candidates that crossed a laptop screening limit were stopped after one "
            "completed task, so their partial results are not aggregate quality claims.",
            "",
            "The selected model was promoted to the full 1,000-query public matrix; "
            "screening-only results are not used as headline quality claims.",
            "",
            "## Reproduce",
            "",
            "```bash",
            "uv run scripts/export_public_retrieval.py --profile model-bakeoff \\",
            "  --output /tmp/ivygrep-model-bakeoff-datasets",
            "IVYGREP_MODEL_PROFILE=static uv run scripts/run_public_benchmark_matrix.py \\",
            "  --profile model-bakeoff --modes neural --runs 1 \\",
            "  --datasets-root /tmp/ivygrep-model-bakeoff-datasets \\",
            "  --work-root /tmp/ivygrep-model-bakeoff-static \\",
            "  --output /tmp/ivygrep-model-bakeoff-static.json",
            "```",
            "",
        )
    )
    return "\n".join(lines)


def html(report: dict) -> str:
    rows = []
    for candidate in report["candidates"]:
        metrics = candidate.get("metrics")
        ndcg = f"{metrics['ndcg_at_10']['mean']:.4f}" if metrics else "-"
        latency = (
            f"{metrics['warm_latency_p95_ms']['mean']:.2f} ms" if metrics else "-"
        )
        rss = (
            format_bytes(metrics["peak_child_rss_bytes"]["mean"]) if metrics else "-"
        )
        rows.append(
            "<tr>"
            f"<td><code>{escape(candidate['profile'])}</code></td>"
            f"<td>{escape(candidate['status'])}</td>"
            f"<td>{ndcg}</td><td>{latency}</td><td>{escape(rss)}</td>"
            f"<td>{escape(candidate['reason'])}</td>"
            "</tr>"
        )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>ivygrep Embedding Model Bake-off</title>
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
      <div class="report-links"><a href="index.html">Reports</a><a href="embedding-model-bakeoff.json">Raw JSON</a><a href="https://github.com/bvolpato/ivygrep/blob/main/docs/benchmarks/embedding-model-bakeoff.md">Source</a></div>
    </nav>
    <section class="report-hero">
      <div class="report-eyebrow">Portable Model Evidence</div>
      <h1>Embedding model bake-off</h1>
      <p>Public screening evidence selected <code>{escape(report["selection"])}</code> for the full retrieval matrix.</p>
    </section>
    <section class="report-card">
      <h2>Candidate results</h2>
      <div class="report-table-wrap"><table class="report-table">
        <thead><tr><th>Profile</th><th>Status</th><th>nDCG@10</th><th>Warm p95</th><th>Peak RSS</th><th>Decision</th></tr></thead>
        <tbody>{"".join(rows)}</tbody>
      </table></div>
    </section>
    <section class="report-card">
      <h2>Claim boundary</h2>
      <p>The static profile is the only candidate that completed the screening matrix within the declared laptop budget. Resource-stopped rows are single-task observations, not aggregate quality claims.</p>
    </section>
  </main>
</body>
</html>
"""


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--candidates",
        type=Path,
        default=root / "benchmarks" / "public" / "model_candidates.json",
    )
    parser.add_argument("--matrix", action="append", default=[])
    parser.add_argument("--partial", action="append", default=[])
    parser.add_argument("--json", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    parser.add_argument("--html", type=Path, required=True)
    args = parser.parse_args()

    manifest = json.loads(args.candidates.read_text(encoding="utf-8"))
    report = build_report(
        root,
        manifest,
        parse_evidence(args.matrix),
        parse_evidence(args.partial),
    )
    args.json.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    args.markdown.write_text(markdown(report), encoding="utf-8")
    args.html.write_text(html(report), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
