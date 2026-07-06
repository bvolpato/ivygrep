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

Measure post-background hash relevance:
    scripts/eval_relevance.py --enhance-hash

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


@dataclass(frozen=True)
class SearchOutput:
    paths: list[str]
    hits: list[dict[str, Any]]
    sources: set[str]


AUDIT_SATISFIED = "satisfied"
AUDIT_CANDIDATE_BUDGET = "candidate_budget"
AUDIT_FIRST_USEFUL_LOW = "first_useful_low"
AUDIT_AFTER_FILTERING = "after_filtering"
AUDIT_BEFORE_FUSION = "before_fusion"
AUDIT_NO_PRIMARY = "no_primary_judgment"
AUDIT_NOT_RUN = "not_audited"


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


def parse_search_output(stdout: str) -> SearchOutput:
    parsed = json.loads(stdout)
    if not isinstance(parsed, list):
        return SearchOutput(paths=[], hits=[], sources=set())
    paths: list[str] = []
    seen: set[str] = set()
    hits: list[dict[str, Any]] = []
    sources: set[str] = set()
    for item in parsed:
        if isinstance(item, dict):
            p = item.get("file_path")
            if isinstance(p, str) and p not in seen:
                seen.add(p)
                paths.append(p)
            item_hits = item.get("hits", [])
            if isinstance(item_hits, list):
                for hit in item_hits:
                    if isinstance(hit, dict):
                        hits.append(hit)
                        hit_sources = hit.get("sources", [])
                        if isinstance(hit_sources, list):
                            sources.update(
                                source for source in hit_sources if isinstance(source, str)
                            )
    return SearchOutput(paths=paths, hits=hits, sources=sources)


def neural_execution_status(hits: list[dict[str, Any]]) -> bool | None:
    if not hits:
        return None
    return any(hit.get("neural_executed") is True for hit in hits)


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


def relevant_judgment_matches(paths: list[str], judgments: list[Judgment]) -> set[int]:
    matches: set[int] = set()
    for idx, judgment in enumerate(judgments):
        if judgment.grade >= RELEVANT_GRADE and any(
            fnmatch.fnmatchcase(path, judgment.pattern) for path in paths
        ):
            matches.add(idx)
    return matches


def relevant_recall(paths: list[str], judgments: list[Judgment]) -> float:
    primary = [idx for idx, judgment in enumerate(judgments) if judgment.grade >= RELEVANT_GRADE]
    if not primary:
        return 0.0
    return len(relevant_judgment_matches(paths, judgments)) / len(primary)


def first_relevant_rank(paths: list[str], judgments: list[Judgment]) -> int | None:
    primary = [judgment for judgment in judgments if judgment.grade >= RELEVANT_GRADE]
    if not primary:
        return None
    for index, path in enumerate(paths, start=1):
        if any(fnmatch.fnmatchcase(path, judgment.pattern) for judgment in primary):
            return index
    return None


def classify_audit_stage(
    final_first_rank: int | None,
    deep_first_rank: int | None,
    candidate_recall: float,
    satisfaction_k: int,
    judgments: list[Judgment],
) -> str:
    if not any(judgment.grade >= RELEVANT_GRADE for judgment in judgments):
        return AUDIT_NO_PRIMARY
    if final_first_rank is not None and final_first_rank <= satisfaction_k:
        return AUDIT_SATISFIED
    if final_first_rank is not None:
        return AUDIT_FIRST_USEFUL_LOW
    if deep_first_rank is not None:
        if deep_first_rank <= satisfaction_k:
            return AUDIT_CANDIDATE_BUDGET
        return AUDIT_FIRST_USEFUL_LOW
    if candidate_recall > 0.0:
        return AUDIT_AFTER_FILTERING
    return AUDIT_BEFORE_FUSION


def audit_stage_counts(rows: list[dict[str, Any]]) -> dict[str, int]:
    stages = [
        AUDIT_SATISFIED,
        AUDIT_CANDIDATE_BUDGET,
        AUDIT_FIRST_USEFUL_LOW,
        AUDIT_AFTER_FILTERING,
        AUDIT_BEFORE_FUSION,
        AUDIT_NO_PRIMARY,
        AUDIT_NOT_RUN,
    ]
    return {stage: sum(1 for row in rows if row.get("audit_stage") == stage) for stage in stages}


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
    tier = ap.add_mutually_exclusive_group()
    tier.add_argument(
        "--enhance-hash",
        action="store_true",
        help="build background hash ANN vectors before queries",
    )
    tier.add_argument(
        "--neural",
        action="store_true",
        help="use neural embeddings (default: foreground lexical/hash)",
    )
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--details", action="store_true")
    ap.add_argument("--json", action="store_true", help="emit aggregate metrics as JSON only")
    audit = ap.add_mutually_exclusive_group()
    audit.add_argument(
        "--audit-recall",
        action="store_true",
        default=True,
        help="run broad source/deep probes to classify relevance misses",
    )
    audit.add_argument(
        "--no-audit-recall",
        action="store_false",
        dest="audit_recall",
        help="skip candidate-recall audit probes",
    )
    ap.add_argument(
        "--satisfaction-k",
        type=int,
        default=1,
        help="first relevant result rank considered useful",
    )
    ap.add_argument("--check", action="store_true", help="exit non-zero if below thresholds")
    ap.add_argument("--min-mrr", type=float, default=0.0)
    ap.add_argument("--min-p1", type=float, default=0.0, help="min precision@1")
    ap.add_argument("--min-recall5", type=float, default=0.0)
    args = ap.parse_args()
    if args.satisfaction_k < 1:
        raise SystemExit("--satisfaction-k must be >= 1")

    repo = args.repo.resolve()
    cases = load_cases(args.queries)
    binary = args.binary.resolve()

    env = os.environ.copy()
    env["IVYGREP_NO_AUTOSPAWN"] = "1"
    # Explicit relevance tiers must finish deterministically even when the
    # production background enhancer would pause under machine load.
    env.setdefault("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0")
    env.setdefault("RUST_BACKTRACE", "1")

    # Always build unless explicitly skipped, so the eval never runs against a
    # stale binary from an older commit. Cargo makes this a no-op when the
    # binary is already up to date.
    if not args.skip_build:
        run(["cargo", "build", "--release", "--locked", "--bin", "ig"], cwd=REPO_ROOT, env=env)
    if not binary.exists():
        raise SystemExit(f"missing binary at {binary} (build first or pass --binary)")

    mode = [] if args.neural else ["--hash"]
    query_mode = ["--force-neural"] if args.neural else mode

    with tempfile.TemporaryDirectory(prefix="ivygrep-eval-") as home:
        env["IVYGREP_HOME"] = home
        run(
            [str(binary), "--add", str(repo), "--force", "--json", "--no-watch", *mode],
            cwd=REPO_ROOT,
            env=env,
        )

        if args.enhance_hash:
            run(
                [str(binary), "--enhance-hash-internal", str(repo)],
                cwd=REPO_ROOT,
                env=env,
            )
        elif args.neural:
            # Build neural vectors up front (foreground, no daemon) so the query
            # path actually exercises them. Without this the queries run before
            # background enhancement exists, silently measuring the hash
            # fallback instead of the neural relevance we intend to gate on.
            run(
                [str(binary), "--enhance-internal", str(repo)],
                cwd=REPO_ROOT,
                env=env,
            )
            # Confirm neural vectors actually exist so a future fallback cannot
            # be reported as neural relevance. Fail loudly.
            status = run(
                [str(binary), "--status", "--json"], cwd=REPO_ROOT, env=env
            )
            try:
                workspaces = json.loads(status)
            except json.JSONDecodeError:
                workspaces = []
            if not any(ws.get("has_neural_vectors") for ws in workspaces):
                raise SystemExit(
                    "--neural requested but no neural vectors were built for "
                    f"{repo} (neural model unavailable?). Refusing to report the "
                    "hash fallback as 'neural'."
                )

        rows: list[dict[str, Any]] = []
        neural_queries_with_results = 0
        neural_queries_executed = 0
        neural_queries_unobservable = 0
        t0 = time.perf_counter()
        for case in cases:
            stdout = run(
                [str(binary), *query_mode, "--json", "--no-watch", "-n", str(args.limit),
                 case.query, str(repo)],
                cwd=REPO_ROOT,
                env=env,
            )
            search_output = parse_search_output(stdout)
            paths = search_output.paths
            hits = search_output.hits
            sources = search_output.sources
            execution_status = neural_execution_status(hits)
            neural_executed = execution_status is True
            if "neural" in sources:
                neural_queries_with_results += 1
            if execution_status is True:
                neural_queries_executed += 1
            elif execution_status is None:
                neural_queries_unobservable += 1
            grades = graded(paths, case.judgments)
            ideal = [j.grade for j in case.judgments]
            final_first_rank = first_relevant_rank(paths, case.judgments)
            deep_first_rank = None
            deep_recall = 0.0
            candidate_recall = relevant_recall(paths, case.judgments)
            candidate_audit_sources = ["final"]
            audit_stage = (
                AUDIT_SATISFIED
                if final_first_rank is not None and final_first_rank <= args.satisfaction_k
                else AUDIT_NOT_RUN
            )
            audit_hits = {"final": len(paths)}
            if args.audit_recall:
                deep_output = parse_search_output(
                    run(
                        [
                            str(binary),
                            *query_mode,
                            "--json",
                            "--no-watch",
                            "--no-limit",
                            case.query,
                            str(repo),
                        ],
                        cwd=REPO_ROOT,
                        env=env,
                    )
                )
                deep_first_rank = first_relevant_rank(deep_output.paths, case.judgments)
                deep_recall = relevant_recall(deep_output.paths, case.judgments)
                audit_hits["hybrid-deep"] = len(deep_output.paths)
                audit_path_union = set(paths) | set(deep_output.paths)
                source_runs = {
                    "lexical": ["--lexical-only"],
                    "literal": ["--literal"],
                }
                candidate_audit_sources = []
                source_recalls = {
                    "final": candidate_recall,
                    "hybrid-deep": deep_recall,
                }
                for source_name, source_mode in source_runs.items():
                    source_output = parse_search_output(
                        run(
                            [
                                str(binary),
                                *source_mode,
                                "--json",
                                "--no-watch",
                                "--no-limit",
                                case.query,
                                str(repo),
                            ],
                            cwd=REPO_ROOT,
                            env=env,
                        )
                    )
                    source_recall = relevant_recall(source_output.paths, case.judgments)
                    source_recalls[source_name] = source_recall
                    audit_hits[source_name] = len(source_output.paths)
                    audit_path_union.update(source_output.paths)
                candidate_recall = relevant_recall(sorted(audit_path_union), case.judgments)
                candidate_audit_sources = [
                    name for name, source_recall in source_recalls.items() if source_recall > 0.0
                ]
                audit_stage = classify_audit_stage(
                    final_first_rank,
                    deep_first_rank,
                    candidate_recall,
                    args.satisfaction_k,
                    case.judgments,
                )
            row = {
                "id": case.id,
                "p1": precision_at(grades, 1),
                "p5": precision_at(grades, 5),
                "mrr": mrr(grades),
                "recall5": recall_at(paths, case.judgments, 5),
                "ndcg10": ndcg_at(grades, ideal, 10),
                "satisfied": final_first_rank is not None
                and final_first_rank <= args.satisfaction_k,
                "first_relevant_rank": final_first_rank,
                "audit_stage": audit_stage,
                "hybrid_deep_first_relevant_rank": deep_first_rank,
                "hybrid_deep_recall": deep_recall,
                "candidate_recall": candidate_recall,
                "candidate_audit_sources": candidate_audit_sources,
                "audit_hits": audit_hits,
                "top3": paths[:3],
                "hits": len(paths),
                "retrieval_sources": sorted(sources),
                "neural_executed": neural_executed,
                "neural_execution_observable": execution_status is not None,
            }
            rows.append(row)
            if args.details:
                print(
                    f"  {case.id:<28} p1={row['p1']:.0f} mrr={row['mrr']:.3f} "
                    f"recall5={row['recall5']:.2f} ndcg10={row['ndcg10']:.3f} "
                    f"first={row['first_relevant_rank']} audit={row['audit_stage']} "
                    f"deep_first={row['hybrid_deep_first_relevant_rank']} "
                    f"candidate_recall={row['candidate_recall']:.2f} top3={row['top3']}"
                )

        stage_counts = audit_stage_counts(rows)
        agg = {
            "mode": (
                "neural"
                if args.neural
                else "hash-enriched"
                if args.enhance_hash
                else "foreground"
            ),
            "queries": len(rows),
            "mean_p1": mean([r["p1"] for r in rows]),
            "mean_p5": mean([r["p5"] for r in rows]),
            "mean_mrr": mean([r["mrr"] for r in rows]),
            "mean_recall5": mean([r["recall5"] for r in rows]),
            "mean_ndcg10": mean([r["ndcg10"] for r in rows]),
            "satisfaction_k": args.satisfaction_k,
            "satisfaction_rate": mean([1.0 if r["satisfied"] else 0.0 for r in rows]),
            "mean_hybrid_deep_recall": mean([r["hybrid_deep_recall"] for r in rows]),
            "mean_candidate_recall": mean([r["candidate_recall"] for r in rows]),
            "audit_stage_counts": stage_counts,
            "candidate_recall_misses": [
                r["id"] for r in rows if r["audit_stage"] == AUDIT_BEFORE_FUSION
            ],
            "filter_misses": [
                r["id"] for r in rows if r["audit_stage"] == AUDIT_AFTER_FILTERING
            ],
            "low_rank_misses": [
                r["id"] for r in rows if r["audit_stage"] == AUDIT_FIRST_USEFUL_LOW
            ],
            "candidate_budget_misses": [
                r["id"] for r in rows if r["audit_stage"] == AUDIT_CANDIDATE_BUDGET
            ],
            "no_hit_queries": sum(1 for r in rows if r["hits"] == 0),
            "neural_queries_with_results": neural_queries_with_results,
            "neural_queries_executed": neural_queries_executed,
            "neural_queries_unobservable": neural_queries_unobservable,
            "elapsed_ms": (time.perf_counter() - t0) * 1000.0,
        }
        if args.neural:
            missing = [
                row["id"]
                for row in rows
                if row["neural_execution_observable"]
                and not row["neural_executed"]
            ]
        else:
            missing = []
        if missing:
            raise SystemExit(
                "neural evaluation did not execute neural retrieval for: "
                + ", ".join(missing)
            )

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
            f"  satisfy@{agg['satisfaction_k']} = {agg['satisfaction_rate']:.3f}\n"
            f"  candidate recall = {agg['mean_candidate_recall']:.3f}\n"
            f"  audit stages = {agg['audit_stage_counts']}\n"
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
