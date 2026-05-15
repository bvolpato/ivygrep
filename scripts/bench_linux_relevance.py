#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Score ivygrep relevance on intent-style Linux kernel queries."""

from __future__ import annotations

import argparse
import fnmatch
import json
import math
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_KERNEL = Path("/home/bruno/githubworkspace/linux")
DEFAULT_HOME = Path("/tmp/ivygrep-linux-relevance-home")
DEFAULT_QUERIES = (
    Path(__file__).resolve().parents[1]
    / "tests"
    / "fixtures"
    / "linux_kernel_relevance_queries.json"
)
TMP_ROOT = Path("/tmp").resolve()
DEFAULT_SPAM_PATTERNS = [
    "Documentation/**",
    "samples/**",
    "tools/**",
    "tools/testing/**",
    "**/selftests/**",
]


@dataclass(frozen=True)
class Judgment:
    pattern: str
    grade: int


@dataclass(frozen=True)
class QueryCase:
    id: str
    query: str
    intent: str
    judgments: list[Judgment]
    spam_patterns: list[str]


def run(cmd: list[str], *, cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=True, check=True)


def timed(cmd: list[str], *, cwd: Path, env: dict[str, str]) -> tuple[float, str]:
    start = time.perf_counter()
    result = run(cmd, cwd=cwd, env=env)
    return time.perf_counter() - start, result.stdout


def ensure_kernel_checkout(path: Path) -> None:
    if not (path / "kernel").is_dir() or not (path / "Makefile").is_file():
        raise SystemExit(f"Linux kernel checkout not found at {path}")


def ensure_bench_home_under_tmp(path: Path) -> Path:
    resolved = path.resolve()
    try:
        resolved.relative_to(TMP_ROOT)
    except ValueError as exc:
        raise SystemExit(f"--bench-home must resolve under {TMP_ROOT}, got {resolved}") from exc
    if resolved == TMP_ROOT:
        raise SystemExit(f"--bench-home must be a child of {TMP_ROOT}, got {resolved}")
    return resolved


def existing_index(path: Path) -> bool:
    indexes_dir = path / "indexes"
    return indexes_dir.is_dir() and any(indexes_dir.glob("*/metadata.sqlite3"))


def load_cases(path: Path) -> list[QueryCase]:
    raw = json.loads(path.read_text())
    cases = []
    for item in raw.get("queries", []):
        judgments = [
            Judgment(pattern=entry["pattern"], grade=int(entry["grade"]))
            for entry in item.get("judgments", [])
        ]
        if not judgments:
            raise SystemExit(f"query {item.get('id', '<unknown>')} has no judgments")
        cases.append(
            QueryCase(
                id=item["id"],
                query=item["query"],
                intent=item.get("intent", ""),
                judgments=judgments,
                spam_patterns=list(item.get("spam_patterns", [])),
            )
        )
    if not cases:
        raise SystemExit(f"no query cases found in {path}")
    return cases


def path_matches(path: str, pattern: str) -> bool:
    return fnmatch.fnmatchcase(path, pattern)


def graded_ranked_paths(paths: list[str], judgments: list[Judgment]) -> list[int]:
    matched_judgments = set()
    grades = []
    for path in paths:
        best_idx = None
        best_grade = 0
        for idx, judgment in enumerate(judgments):
            if idx in matched_judgments:
                continue
            if path_matches(path, judgment.pattern) and judgment.grade > best_grade:
                best_idx = idx
                best_grade = judgment.grade
        if best_idx is not None:
            matched_judgments.add(best_idx)
        grades.append(best_grade)
    return grades


def matches_any(path: str, patterns: list[str]) -> bool:
    return any(path_matches(path, pattern) for pattern in patterns)


def ranked_paths(output: str) -> list[str]:
    parsed: Any = json.loads(output)
    if not isinstance(parsed, list):
        raise ValueError("search output is not a JSON list")
    paths = []
    seen = set()
    for item in parsed:
        if not isinstance(item, dict):
            continue
        path = item.get("file_path")
        if isinstance(path, str) and path not in seen:
            seen.add(path)
            paths.append(path)
    return paths


def dcg(grades: list[int]) -> float:
    total = 0.0
    for idx, grade in enumerate(grades):
        gain = (2**grade) - 1
        total += gain / math.log2(idx + 2)
    return total


def ndcg_at(grades: list[int], ideal_grades: list[int], k: int) -> float:
    actual = dcg(grades[:k])
    ideal = dcg(sorted(ideal_grades, reverse=True)[:k])
    if ideal <= sys.float_info.epsilon:
        return 0.0
    return actual / ideal


def precision_at(grades: list[int], k: int) -> float:
    return sum(1 for grade in grades[:k] if grade > 0) / k


def mrr_at(grades: list[int], k: int) -> float:
    for idx, grade in enumerate(grades[:k]):
        if grade >= 2:
            return 1.0 / (idx + 1)
    return 0.0


def recall_at(paths: list[str], judgments: list[Judgment], k: int) -> float:
    primary = [judgment for judgment in judgments if judgment.grade >= 2]
    if not primary:
        return 0.0
    matched = 0
    for judgment in primary:
        if any(path_matches(path, judgment.pattern) for path in paths[:k]):
            matched += 1
    return matched / len(primary)


def score_case(case: QueryCase, paths: list[str], *, limit: int) -> dict[str, Any]:
    grades = graded_ranked_paths(paths, case.judgments)
    ideal_grades = [judgment.grade for judgment in case.judgments]
    spam_patterns = [*DEFAULT_SPAM_PATTERNS, *case.spam_patterns]
    spam_top10_paths = [path for path in paths[:10] if matches_any(path, spam_patterns)]
    forbidden_top3_paths = [path for path in paths[:3] if matches_any(path, spam_patterns)]
    return {
        "id": case.id,
        "query": case.query,
        "intent": case.intent,
        "hit_count": len(paths),
        "top_grade": grades[0] if grades else 0,
        "ndcg10": ndcg_at(grades, ideal_grades, 10),
        "mrr10": mrr_at(grades, 10),
        "precision5": precision_at(grades, 5),
        "recall20": recall_at(paths, case.judgments, 20),
        "spam_top10_count": len(spam_top10_paths),
        "forbidden_top3_count": len(forbidden_top3_paths),
        "top5": paths[:5],
        "spam_top10": spam_top10_paths,
        "forbidden_top3": forbidden_top3_paths,
        "limit": limit,
    }


def mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0


def aggregate(case_scores: list[dict[str, Any]], *, elapsed_ms: float, limit: int) -> dict[str, Any]:
    query_count = len(case_scores)
    no_hit_queries = sum(1 for score in case_scores if score["hit_count"] == 0)
    spam_top10_count = sum(int(score["spam_top10_count"]) for score in case_scores)
    forbidden_top3_count = sum(int(score["forbidden_top3_count"]) for score in case_scores)
    spam_top10_rate = spam_top10_count / max(query_count * 10, 1)
    forbidden_top3_rate = forbidden_top3_count / max(query_count * 3, 1)
    no_hit_rate = no_hit_queries / max(query_count, 1)

    mean_ndcg10 = mean([float(score["ndcg10"]) for score in case_scores])
    mean_mrr10 = mean([float(score["mrr10"]) for score in case_scores])
    mean_precision5 = mean([float(score["precision5"]) for score in case_scores])
    mean_recall20 = mean([float(score["recall20"]) for score in case_scores])
    top_relevant_rate = mean(
        [1.0 if int(score["top_grade"]) >= 2 else 0.0 for score in case_scores]
    )

    weighted_quality = (
        0.55 * mean_ndcg10
        + 0.20 * mean_mrr10
        + 0.15 * mean_precision5
        + 0.10 * mean_recall20
    )
    quality_points = 100.0 * weighted_quality
    penalty_factor = max(
        0.0,
        1.0
        - 0.35 * spam_top10_rate
        - 0.50 * forbidden_top3_rate
        - 0.50 * no_hit_rate,
    )
    linux_relevance_score = quality_points * penalty_factor

    return {
        "linux_relevance_score": linux_relevance_score,
        "quality_points": quality_points,
        "penalty_factor": penalty_factor,
        "mean_ndcg10": mean_ndcg10,
        "mean_mrr10": mean_mrr10,
        "mean_precision5": mean_precision5,
        "mean_recall20": mean_recall20,
        "top_relevant_rate": top_relevant_rate,
        "spam_top10_rate": spam_top10_rate,
        "forbidden_top3_rate": forbidden_top3_rate,
        "no_hit_rate": no_hit_rate,
        "spam_top10_count": spam_top10_count,
        "forbidden_top3_count": forbidden_top3_count,
        "no_hit_queries": no_hit_queries,
        "query_count": query_count,
        "limit": limit,
        "elapsed_ms": elapsed_ms,
    }


def kernel_commit(kernel: Path) -> str:
    try:
        return run(["git", "-C", str(kernel), "rev-parse", "--short", "HEAD"], cwd=kernel, env=os.environ.copy()).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, default=DEFAULT_KERNEL)
    parser.add_argument("--bench-home", type=Path, default=DEFAULT_HOME)
    parser.add_argument("--queries", type=Path, default=DEFAULT_QUERIES)
    parser.add_argument("--limit", type=int, default=50)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--skip-index", action="store_true")
    parser.add_argument("--reindex", action="store_true")
    parser.add_argument("--binary", type=Path, default=None)
    parser.add_argument("--details", action="store_true")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parent.parent
    kernel = args.kernel.resolve()
    bench_home = ensure_bench_home_under_tmp(args.bench_home)
    binary = (
        args.binary.resolve()
        if args.binary is not None
        else repo / "target" / "release" / "ig"
    )
    ensure_kernel_checkout(kernel)
    cases = load_cases(args.queries)

    env = os.environ.copy()
    env["IVYGREP_HOME"] = str(bench_home)
    env["IVYGREP_NO_AUTOSPAWN"] = "1"
    env["RUST_BACKTRACE"] = env.get("RUST_BACKTRACE", "1")

    if not args.skip_build and args.binary is None:
        timed(["cargo", "build", "--release", "--locked", "--bin", "ig"], cwd=repo, env=env)
    if not binary.exists():
        raise SystemExit(f"missing release binary at {binary}")

    if args.reindex:
        shutil.rmtree(bench_home, ignore_errors=True)
    if args.skip_index:
        if not existing_index(bench_home):
            raise SystemExit(f"--skip-index needs existing index under {bench_home}")
    elif args.reindex or not existing_index(bench_home):
        bench_home.mkdir(parents=True, exist_ok=True)
        timed(
            [
                str(binary),
                "--add",
                str(kernel),
                "--force",
                "--json",
                "--no-watch",
                "--hash",
            ],
            cwd=repo,
            env=env,
        )

    start = time.perf_counter()
    case_scores = []
    for case in cases:
        seconds, stdout = timed(
            [
                str(binary),
                "--hash",
                "--json",
                "--no-watch",
                "-n",
                str(args.limit),
                case.query,
                str(kernel),
            ],
            cwd=repo,
            env=env,
        )
        paths = ranked_paths(stdout)
        score = score_case(case, paths, limit=args.limit)
        score["query_ms"] = seconds * 1000.0
        case_scores.append(score)
        if args.details:
            print(
                f"{case.id}: ndcg10={score['ndcg10']:.3f} "
                f"mrr10={score['mrr10']:.3f} p5={score['precision5']:.3f} "
                f"recall20={score['recall20']:.3f} top5={score['top5']}"
            )

    metrics = aggregate(case_scores, elapsed_ms=(time.perf_counter() - start) * 1000.0, limit=args.limit)
    metrics["kernel_commit"] = kernel_commit(kernel)
    metrics["bench_home"] = str(bench_home)
    metrics["query_file"] = str(args.queries.resolve())
    print(json.dumps(metrics, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
