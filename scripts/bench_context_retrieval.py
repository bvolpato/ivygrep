#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Benchmark retrieval on frozen tasks against clean pre-change repository trees."""

from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time
from typing import Any


TEXT_SUFFIXES = {
    ".c",
    ".cc",
    ".cpp",
    ".cs",
    ".css",
    ".go",
    ".h",
    ".hpp",
    ".html",
    ".java",
    ".js",
    ".json",
    ".kt",
    ".md",
    ".py",
    ".rb",
    ".rs",
    ".sh",
    ".swift",
    ".toml",
    ".ts",
    ".tsx",
    ".yaml",
    ".yml",
}


def run(command: list[str], cwd: Path, env: dict[str, str]) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return completed.stdout


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args],
        cwd=repo,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def git_path_exists(repo: Path, revision: str, path: str) -> bool:
    return subprocess.run(
        ["git", "cat-file", "-e", f"{revision}:{path}"],
        cwd=repo,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def historical_tasks(repo: Path, limit: int) -> list[dict[str, Any]]:
    rows = git(repo, "log", "--no-merges", "-n", "160", "--format=%H%x09%s").splitlines()
    tasks: list[dict[str, Any]] = []
    seen_subjects: set[str] = set()
    for row in rows:
        commit, subject = row.split("\t", 1)
        if subject in seen_subjects or len(subject.split()) < 3:
            continue
        parents = git(repo, "rev-list", "--parents", "-n", "1", commit).split()
        if len(parents) != 2:
            continue
        base_commit = parents[1]
        changed = git(
            repo,
            "diff",
            "--name-only",
            base_commit,
            commit,
        ).splitlines()
        expected = sorted(
            path
            for path in changed
            if git_path_exists(repo, base_commit, path)
            and Path(path).suffix.lower() in TEXT_SUFFIXES
            and not path.startswith("docs/benchmarks/")
        )
        if not 1 <= len(expected) <= 8:
            continue
        seen_subjects.add(subject)
        tasks.append(
            {
                "id": commit[:12],
                "task": subject,
                "base_commit": base_commit,
                "source_commit": commit,
                "expected_paths": expected,
                "label_source": "changed_paths",
            }
        )
        if len(tasks) == limit:
            break
    if len(tasks) < limit:
        raise RuntimeError(f"only found {len(tasks)} usable historical tasks; requested {limit}")
    return tasks


def load_tasks(repo: Path, path: Path) -> list[dict[str, Any]]:
    tasks = json.loads(path.read_text())["tasks"]
    if not tasks:
        raise ValueError("task fixture contains no tasks")
    for task in tasks:
        missing = sorted(
            {"id", "task", "base_commit", "expected_paths"} - task.keys()
        )
        if missing:
            raise ValueError(f"task is missing required fields {missing}: {task}")
        if not task["expected_paths"]:
            raise ValueError(f"task {task['id']} has no expected paths")
        base_commit = git(
            repo, "rev-parse", "--verify", f"{task['base_commit']}^{{commit}}"
        ).strip()
        label_source = task.get("label_source", "changed_paths")
        if label_source not in {"changed_paths", "curated"}:
            raise ValueError(
                f"task {task['id']} has unsupported label_source {label_source!r}"
            )
        unavailable = [
            expected
            for expected in task["expected_paths"]
            if not git_path_exists(repo, task["base_commit"], expected)
        ]
        if unavailable:
            raise ValueError(
                f"task {task['id']} paths do not exist at {task['base_commit']}: {unavailable}"
            )
        if label_source == "changed_paths":
            if "source_commit" not in task:
                raise ValueError(
                    f"task {task['id']} uses changed_paths labels without source_commit"
                )
            source_commit = git(
                repo, "rev-parse", "--verify", f"{task['source_commit']}^{{commit}}"
            ).strip()
            source_parent = git(repo, "rev-parse", f"{source_commit}^").strip()
            if source_parent != base_commit:
                raise ValueError(
                    f"task {task['id']} base {task['base_commit']} is not source parent {source_parent}"
                )
            changed_paths = set(
                git(repo, "diff", "--name-only", task["base_commit"], source_commit).splitlines()
            )
            unchanged = [path for path in task["expected_paths"] if path not in changed_paths]
            if unchanged:
                raise ValueError(
                    f"task {task['id']} expected paths were not changed by {source_commit}: {unchanged}"
                )
        task["label_source"] = label_source
    return tasks


@contextmanager
def clean_task_worktree(repo: Path, base_commit: str, destination: Path):
    git(repo, "worktree", "add", "--detach", "--force", str(destination), base_commit)
    try:
        if git(destination, "status", "--porcelain").strip():
            raise RuntimeError(f"benchmark worktree is dirty: {destination}")
        yield destination
    finally:
        subprocess.run(
            ["git", "worktree", "remove", "--force", str(destination)],
            cwd=repo,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )


def percentile(values: list[float], percentile_value: float) -> float:
    ordered = sorted(values)
    index = math.ceil((percentile_value / 100) * len(ordered)) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def normalize_path(path: str, repo: Path) -> str:
    candidate = Path(path)
    if candidate.is_absolute():
        try:
            return candidate.relative_to(repo).as_posix()
        except ValueError:
            return candidate.as_posix()
    return candidate.as_posix().removeprefix("./")


def path_role(path: str) -> str:
    candidate = Path(path)
    name = candidate.name.lower()
    parts = {part.lower() for part in candidate.parts}
    if parts.intersection({"test", "tests", "testing", "spec", "specs", "__tests__"}) or any(
        marker in name for marker in ("_test.", ".test.", ".spec.")
    ):
        return "test"
    if parts.intersection({"doc", "docs", "example", "examples"}) or name in {
        "readme.md",
        "changelog.md",
    }:
        return "documentation"
    if parts.intersection({".github", "config"}) or name in {
        "cargo.toml",
        "package.json",
        "pyproject.toml",
    }:
        return "config"
    return "primary"


def retrieval_metrics(paths: list[str], expected_paths: list[str]) -> dict[str, Any]:
    unique_paths = list(dict.fromkeys(paths))
    expected = set(expected_paths)
    matched = expected.intersection(unique_paths)
    first_rank = next(
        (rank for rank, path in enumerate(unique_paths, start=1) if path in expected),
        None,
    )
    role_recall = {}
    for role in sorted({path_role(path) for path in expected_paths}):
        role_paths = {path for path in expected if path_role(path) == role}
        role_recall[role] = len(role_paths.intersection(unique_paths)) / len(role_paths)
    return {
        "selected_files": len(unique_paths),
        "matched_files": len(matched),
        "recall": len(matched) / len(expected),
        "precision": len(matched) / len(unique_paths) if unique_paths else 0.0,
        "reciprocal_rank": 1.0 / first_rank if first_rank else 0.0,
        "role_recall": role_recall,
    }


def evaluate_query(
    binary: Path,
    repo: Path,
    env: dict[str, str],
    mode: str,
    budget: int,
    limit: int,
    task: dict[str, Any],
) -> dict[str, Any]:
    if mode == "context":
        command = [
            str(binary),
            "context",
            "--json",
            "--hash",
            "--no-watch",
            "--budget",
            str(budget),
            task["task"],
            str(repo),
        ]
    else:
        command = [
            str(binary),
            "--json",
            "--hash",
            "--no-watch",
            "--limit",
            str(limit),
            task["task"],
            str(repo),
        ]
    started = time.perf_counter()
    payload = json.loads(run(command, repo, env))
    latency_ms = (time.perf_counter() - started) * 1000
    covered_roles: list[str] = []
    if mode == "context":
        paths = [normalize_path(item["file_path"], repo) for item in payload["items"]]
        used_tokens = int(payload["used_tokens"])
        candidate_count = int(payload["candidate_count"])
        covered_roles = sorted(
            role for role, count in payload["coverage"].items() if role != "files" and count
        )
    else:
        hits = payload["hits"] if isinstance(payload, dict) else payload
        paths = [normalize_path(hit["file_path"], repo) for hit in hits]
        used_tokens = 0
        candidate_count = len(hits)
    metrics = retrieval_metrics(paths, task["expected_paths"])
    return {
        **task,
        **metrics,
        "recall_per_1k_tokens": (
            float(metrics["recall"]) * 1_000 / used_tokens if used_tokens else 0.0
        ),
        "latency_ms": latency_ms,
        "used_tokens": used_tokens,
        "candidate_count": candidate_count,
        "covered_roles": covered_roles,
        "paths": paths,
    }


def summarize_mode(mode: str, rows: list[dict[str, Any]]) -> dict[str, Any]:
    latencies = [row["latency_ms"] for row in rows]
    roles = sorted({role for row in rows for role in row["role_recall"]})
    return {
        "mode": mode,
        "queries": len(rows),
        "mean_recall": statistics.fmean(row["recall"] for row in rows),
        "mean_precision": statistics.fmean(row["precision"] for row in rows),
        "mean_reciprocal_rank": statistics.fmean(row["reciprocal_rank"] for row in rows),
        "mean_used_tokens": statistics.fmean(row["used_tokens"] for row in rows),
        "mean_recall_per_1k_tokens": statistics.fmean(
            row["recall_per_1k_tokens"] for row in rows
        ),
        "zero_recall_rate": sum(row["recall"] == 0 for row in rows) / len(rows),
        "mean_covered_roles": statistics.fmean(len(row["covered_roles"]) for row in rows),
        "mean_role_recall": {
            role: statistics.fmean(
                row["role_recall"][role] for row in rows if role in row["role_recall"]
            )
            for role in roles
        },
        "latency_p50_ms": statistics.median(latencies),
        "latency_p95_ms": percentile(latencies, 95),
        "rows": rows,
    }


def constant_topk_baseline(tasks: list[dict[str, Any]], k: int) -> dict[str, Any]:
    """Score the K most frequent gold paths in the fixture as a constant answer.

    This is an oracle prior, not a retriever: it reads the labels. Any retriever
    that cannot beat it by a margin is not using the task text.
    """
    if k < 1:
        raise ValueError("constant baseline k must be >= 1")
    counts = Counter(path for task in tasks for path in task["expected_paths"])
    selected = [path for path, _ in sorted(counts.items(), key=lambda item: (-item[1], item[0]))][:k]
    rows = [
        {**task, **retrieval_metrics(selected, task["expected_paths"])} for task in tasks
    ]
    roles = sorted({role for row in rows for role in row["role_recall"]})
    return {
        "mode": "constant-topk",
        "k": k,
        "paths": selected,
        "queries": len(rows),
        "mean_recall": statistics.fmean(row["recall"] for row in rows),
        "mean_precision": statistics.fmean(row["precision"] for row in rows),
        "mean_reciprocal_rank": statistics.fmean(row["reciprocal_rank"] for row in rows),
        "zero_recall_rate": sum(row["recall"] == 0 for row in rows) / len(rows),
        "mean_role_recall": {
            role: statistics.fmean(
                row["role_recall"][role] for row in rows if role in row["role_recall"]
            )
            for role in roles
        },
    }


def evaluate_tasks(
    binary: Path,
    repo: Path,
    modes: list[str],
    budget: int,
    limit: int,
    tasks: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows = {mode: [] for mode in modes}
    with tempfile.TemporaryDirectory(prefix="ivygrep-context-bench-") as temporary:
        root = Path(temporary)
        for position, task in enumerate(tasks):
            worktree = root / "worktrees" / f"{position:03d}"
            home = root / "homes" / f"{position:03d}"
            home.mkdir(parents=True)
            with clean_task_worktree(repo, task["base_commit"], worktree):
                env = os.environ.copy()
                env["IVYGREP_HOME"] = str(home)
                env["IVYGREP_NO_AUTOSPAWN"] = "1"
                env["IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT"] = "1"
                env["IVYGREP_ENHANCE_MAX_LOAD_RATIO"] = "0"
                run(
                    [str(binary), "--add", str(worktree), "--force", "--hash", "--no-watch"],
                    worktree,
                    env,
                )
                run([str(binary), "--enhance-hash-internal", str(worktree)], worktree, env)
                for mode in modes:
                    rows[mode].append(
                        evaluate_query(binary, worktree, env, mode, budget, limit, task)
                    )
    return [summarize_mode(mode, rows[mode]) for mode in modes]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--tasks", type=int, default=30)
    parser.add_argument("--tasks-from", type=Path)
    parser.add_argument("--budget", type=int, default=8_000)
    parser.add_argument("--search-limit", type=int, default=20)
    parser.add_argument("--modes", default="search,context")
    parser.add_argument(
        "--baseline",
        choices=["none", "constant-topk"],
        default="constant-topk",
        help="Label-derived constant answer scored next to ivygrep (default: constant-topk).",
    )
    parser.add_argument("--baseline-k", type=int, default=5)
    parser.add_argument(
        "--min-margin-over-baseline",
        type=float,
        default=None,
        help=(
            "Gate: context mean_recall must exceed the baseline mean_recall by this "
            "margin. Off by default; the baseline is always scored and reported."
        ),
    )
    parser.add_argument("--min-context-recall", type=float)
    parser.add_argument("--min-context-primary-recall", type=float)
    parser.add_argument("--min-context-test-recall", type=float)
    parser.add_argument("--max-context-zero-recall-rate", type=float)
    parser.add_argument("--min-context-covered-roles", type=float)
    parser.add_argument("--min-context-recall-per-1k-tokens", type=float)
    return parser.parse_args()


def check_context_gates(
    results: list[dict[str, Any]],
    args: argparse.Namespace,
    baseline: dict[str, Any] | None = None,
) -> None:
    gates = {
        "mean_recall": (args.min_context_recall, lambda actual, expected: actual >= expected),
        "zero_recall_rate": (
            args.max_context_zero_recall_rate,
            lambda actual, expected: actual <= expected,
        ),
        "mean_recall_per_1k_tokens": (
            args.min_context_recall_per_1k_tokens,
            lambda actual, expected: actual >= expected,
        ),
        "mean_covered_roles": (
            args.min_context_covered_roles,
            lambda actual, expected: actual >= expected,
        ),
    }
    context = next((result for result in results if result["mode"] == "context"), None)
    role_gates = {
        "primary": args.min_context_primary_recall,
        "test": args.min_context_test_recall,
    }
    has_gate = any(expected is not None for expected in role_gates.values()) or any(
        expected is not None for expected, _ in gates.values()
    )
    if context is None and has_gate:
        raise ValueError("context gates require context mode")
    failures = []
    if context is not None:
        for role, expected in role_gates.items():
            if expected is not None:
                role_recall = context["mean_role_recall"].get(role, 0.0)
                if role_recall >= expected:
                    continue
                failures.append(
                    f"mean_role_recall.{role}="
                    f"{role_recall:.6f}, threshold={expected:.6f}"
                )
        for metric, (expected, predicate) in gates.items():
            if expected is None:
                continue
            actual: Any = context
            for component in metric.split("."):
                actual = actual[component]
            if not predicate(actual, expected):
                failures.append(f"{metric}={actual:.6f}, threshold={expected:.6f}")
        margin = getattr(args, "min_margin_over_baseline", None)
        if baseline is not None and margin is not None:
            actual_margin = context["mean_recall"] - baseline["mean_recall"]
            if actual_margin < margin:
                failures.append(
                    f"mean_recall margin over {baseline['mode']}(k={baseline['k']})="
                    f"{actual_margin:.6f}, threshold={margin:.6f} "
                    f"(context={context['mean_recall']:.6f}, baseline={baseline['mean_recall']:.6f})"
                )
    if failures:
        raise SystemExit("context retrieval gate failed: " + "; ".join(failures))


def main() -> None:
    args = parse_args()
    repo = args.repo.resolve()
    binary = args.binary.resolve()
    modes = [mode.strip() for mode in args.modes.split(",") if mode.strip()]
    invalid_modes = sorted(set(modes) - {"search", "context"})
    if invalid_modes:
        raise ValueError(f"unsupported modes: {', '.join(invalid_modes)}")
    binary_version = run([str(binary), "--version"], repo, os.environ.copy()).strip()
    if args.tasks_from:
        tasks = load_tasks(repo, args.tasks_from)
    else:
        tasks = historical_tasks(repo, args.tasks)
    results = evaluate_tasks(binary, repo, modes, args.budget, args.search_limit, tasks)
    baseline = (
        constant_topk_baseline(tasks, args.baseline_k)
        if args.baseline == "constant-topk"
        else None
    )
    output = {
        "schema_version": 2,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "binary": str(binary),
        "binary_sha256": sha256_file(binary),
        "binary_version": binary_version,
        "repo": str(repo),
        "repo_commit": git(repo, "rev-parse", "HEAD").strip(),
        "source_repo_dirty": bool(git(repo, "status", "--porcelain").strip()),
        "evaluation_tree": "clean_pre_change_worktree",
        "harness_sha256": sha256_file(Path(__file__)),
        "budget": args.budget,
        "search_limit": args.search_limit,
        "tasks": tasks,
        "results": results,
        "baseline": baseline,
        "min_margin_over_baseline": args.min_margin_over_baseline,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n")
    summary = {
        result["mode"]: {key: value for key, value in result.items() if key != "rows"}
        for result in results
    }
    if baseline is not None:
        summary[baseline["mode"]] = baseline
        context = next((result for result in results if result["mode"] == "context"), None)
        if context is not None:
            summary["context_margin_over_baseline"] = {
                "mean_recall": context["mean_recall"] - baseline["mean_recall"],
                "mean_reciprocal_rank": (
                    context["mean_reciprocal_rank"] - baseline["mean_reciprocal_rank"]
                ),
            }
    print(json.dumps(summary, indent=2))
    check_context_gates(results, args, baseline)


if __name__ == "__main__":
    main()
