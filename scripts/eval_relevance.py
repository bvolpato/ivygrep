#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Relevance evaluation harness / gate for ivygrep.

Indexes a repo, runs a set of labeled intent-style queries, and reports
precision@k, recall@k, MRR, and nDCG. Self-contained by default: it evaluates
ivygrep's own source tree using tests/fixtures/ivygrep_relevance_queries.json,
so it needs no external checkout and can run in CI.

Use as a measurement:
    scripts/eval_relevance.py

Use as a gate (non-zero exit if below thresholds):
    scripts/eval_relevance.py --check --min-mrr 0.55 --min-p1 0.50

Each query lists judgments (path glob -> grade). grade>=2 counts as a
"relevant" result for precision/MRR; grade>=1 contributes to nDCG.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import math
import os
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_QUERIES = REPO_ROOT / "tests" / "fixtures" / "ivygrep_relevance_queries.json"
RELEVANT_GRADE = 2  # grade at or above which a hit counts as "relevant"


@dataclass(frozen=True)
class Judgment:
    pattern: str
    grade: int


@dataclass(frozen=True)
class QueryCase:
    id: str
    query: str
    judgments: list[Judgment]


def run(cmd: list[str], *, cwd: Path, env: dict[str, str]) -> str:
    result = subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=True)
    if result.returncode != 0:
        raise SystemExit(
            f"command failed ({result.returncode}): {' '.join(cmd)}\n{result.stderr}"
        )
    return result.stdout


def load_cases(path: Path) -> list[QueryCase]:
    raw = json.loads(path.read_text())
    cases = []
    for item in raw.get("queries", []):
        judgments = [
            Judgment(pattern=j["pattern"], grade=int(j["grade"]))
            for j in item.get("judgments", [])
        ]
        if not judgments:
            raise SystemExit(f"query {item.get('id')} has no judgments")
        cases.append(QueryCase(id=item["id"], query=item["query"], judgments=judgments))
    if not cases:
        raise SystemExit(f"no query cases in {path}")
    return cases


def ranked_paths(stdout: str) -> list[str]:
    parsed = json.loads(stdout)
    if not isinstance(parsed, list):
        return []
    paths: list[str] = []
    seen: set[str] = set()
    for item in parsed:
        if isinstance(item, dict):
            p = item.get("file_path")
            if isinstance(p, str) and p not in seen:
                seen.add(p)
                paths.append(p)
    return paths


def graded(paths: list[str], judgments: list[Judgment]) -> list[int]:
    """Grade each ranked path by its best-matching, not-yet-consumed judgment."""
    used: set[int] = set()
    grades: list[int] = []
    for path in paths:
        best_idx, best_grade = None, 0
        for idx, j in enumerate(judgments):
            if idx in used:
                continue
            if fnmatch.fnmatchcase(path, j.pattern) and j.grade > best_grade:
                best_idx, best_grade = idx, j.grade
        if best_idx is not None:
            used.add(best_idx)
        grades.append(best_grade)
    return grades


def precision_at(grades: list[int], k: int) -> float:
    return sum(1 for g in grades[:k] if g >= RELEVANT_GRADE) / k


def mrr(grades: list[int]) -> float:
    for idx, g in enumerate(grades):
        if g >= RELEVANT_GRADE:
            return 1.0 / (idx + 1)
    return 0.0


def recall_at(paths: list[str], judgments: list[Judgment], k: int) -> float:
    primary = [j for j in judgments if j.grade >= RELEVANT_GRADE]
    if not primary:
        return 0.0
    matched = sum(
        1
        for j in primary
        if any(fnmatch.fnmatchcase(p, j.pattern) for p in paths[:k])
    )
    return matched / len(primary)


def dcg(grades: list[int]) -> float:
    return sum(((2**g) - 1) / math.log2(i + 2) for i, g in enumerate(grades))


def ndcg_at(grades: list[int], ideal: list[int], k: int) -> float:
    best = dcg(sorted(ideal, reverse=True)[:k])
    return dcg(grades[:k]) / best if best > sys.float_info.epsilon else 0.0


def mean(xs: list[float]) -> float:
    return sum(xs) / len(xs) if xs else 0.0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo", type=Path, default=REPO_ROOT, help="repo to index/search")
    ap.add_argument("--queries", type=Path, default=DEFAULT_QUERIES)
    ap.add_argument("--binary", type=Path, default=REPO_ROOT / "target" / "release" / "ig")
    ap.add_argument("--limit", type=int, default=20)
    ap.add_argument("--neural", action="store_true", help="use neural embeddings (default: hash)")
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--details", action="store_true")
    ap.add_argument("--json", action="store_true", help="emit aggregate metrics as JSON only")
    ap.add_argument("--check", action="store_true", help="exit non-zero if below thresholds")
    ap.add_argument("--min-mrr", type=float, default=0.0)
    ap.add_argument("--min-p1", type=float, default=0.0, help="min precision@1")
    ap.add_argument("--min-recall5", type=float, default=0.0)
    args = ap.parse_args()

    repo = args.repo.resolve()
    cases = load_cases(args.queries)
    binary = args.binary.resolve()

    env = os.environ.copy()
    env["IVYGREP_NO_AUTOSPAWN"] = "1"
    env.setdefault("RUST_BACKTRACE", "1")

    # Always build unless explicitly skipped, so the eval never runs against a
    # stale binary from an older commit. Cargo makes this a no-op when the
    # binary is already up to date.
    if not args.skip_build:
        run(["cargo", "build", "--release", "--locked", "--bin", "ig"], cwd=REPO_ROOT, env=env)
    if not binary.exists():
        raise SystemExit(f"missing binary at {binary} (build first or pass --binary)")

    mode = [] if args.neural else ["--hash"]

    with tempfile.TemporaryDirectory(prefix="ivygrep-eval-") as home:
        env["IVYGREP_HOME"] = home
        run(
            [str(binary), "--add", str(repo), "--force", "--json", "--no-watch", *mode],
            cwd=REPO_ROOT,
            env=env,
        )

        if args.neural:
            # Build neural vectors up front (foreground, no daemon) so the query
            # path actually exercises them. Without this the queries run before
            # background enhancement exists, silently measuring the hash
            # fallback instead of the neural relevance we intend to gate on.
            run(
                [str(binary), "--enhance-internal", str(repo)],
                cwd=REPO_ROOT,
                env=env,
            )

        rows: list[dict[str, Any]] = []
        t0 = time.perf_counter()
        for case in cases:
            stdout = run(
                [str(binary), *mode, "--json", "--no-watch", "-n", str(args.limit),
                 case.query, str(repo)],
                cwd=REPO_ROOT,
                env=env,
            )
            paths = ranked_paths(stdout)
            grades = graded(paths, case.judgments)
            ideal = [j.grade for j in case.judgments]
            row = {
                "id": case.id,
                "p1": precision_at(grades, 1),
                "p5": precision_at(grades, 5),
                "mrr": mrr(grades),
                "recall5": recall_at(paths, case.judgments, 5),
                "ndcg10": ndcg_at(grades, ideal, 10),
                "top3": paths[:3],
                "hits": len(paths),
            }
            rows.append(row)
            if args.details:
                print(
                    f"  {case.id:<28} p1={row['p1']:.0f} mrr={row['mrr']:.3f} "
                    f"recall5={row['recall5']:.2f} ndcg10={row['ndcg10']:.3f} "
                    f"top3={row['top3']}"
                )

        agg = {
            "mode": "neural" if args.neural else "hash",
            "queries": len(rows),
            "mean_p1": mean([r["p1"] for r in rows]),
            "mean_p5": mean([r["p5"] for r in rows]),
            "mean_mrr": mean([r["mrr"] for r in rows]),
            "mean_recall5": mean([r["recall5"] for r in rows]),
            "mean_ndcg10": mean([r["ndcg10"] for r in rows]),
            "no_hit_queries": sum(1 for r in rows if r["hits"] == 0),
            "elapsed_ms": (time.perf_counter() - t0) * 1000.0,
        }

    if args.json:
        print(json.dumps(agg, sort_keys=True))
    else:
        print(
            f"\nrelevance ({agg['mode']}, {agg['queries']} queries):\n"
            f"  precision@1 = {agg['mean_p1']:.3f}\n"
            f"  precision@5 = {agg['mean_p5']:.3f}\n"
            f"  MRR         = {agg['mean_mrr']:.3f}\n"
            f"  recall@5    = {agg['mean_recall5']:.3f}\n"
            f"  nDCG@10     = {agg['mean_ndcg10']:.3f}\n"
            f"  no-hit      = {agg['no_hit_queries']}/{agg['queries']}"
        )

    if args.check:
        failures = []
        if agg["mean_mrr"] < args.min_mrr:
            failures.append(f"MRR {agg['mean_mrr']:.3f} < {args.min_mrr}")
        if agg["mean_p1"] < args.min_p1:
            failures.append(f"precision@1 {agg['mean_p1']:.3f} < {args.min_p1}")
        if agg["mean_recall5"] < args.min_recall5:
            failures.append(f"recall@5 {agg['mean_recall5']:.3f} < {args.min_recall5}")
        if failures:
            print("\nFAIL: " + "; ".join(failures), file=sys.stderr)
            return 1
        print("\nPASS: relevance thresholds met")
    return 0


if __name__ == "__main__":
    sys.exit(main())
