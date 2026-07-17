#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Benchmark context-pack retrieval against repository change history."""

from __future__ import annotations

import argparse
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
    ).stdout


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
        changed = git(
            repo,
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            commit,
        ).splitlines()
        expected = sorted(
            path
            for path in changed
            if (repo / path).is_file()
            and (Path(path).suffix.lower() in TEXT_SUFFIXES or Path(path).name == "Cargo.lock")
            and not path.startswith("docs/benchmarks/")
        )
        if not 1 <= len(expected) <= 8:
            continue
        seen_subjects.add(subject)
        tasks.append({"id": commit[:12], "task": subject, "expected_paths": expected})
        if len(tasks) == limit:
            break
    if len(tasks) < limit:
        raise RuntimeError(f"only found {len(tasks)} usable historical tasks; requested {limit}")
    return tasks


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


def retrieval_metrics(paths: list[str], expected_paths: list[str]) -> dict[str, float | int]:
    unique_paths = list(dict.fromkeys(paths))
    expected = set(expected_paths)
    matched = expected.intersection(unique_paths)
    first_rank = next(
        (rank for rank, path in enumerate(unique_paths, start=1) if path in expected),
        None,
    )
    return {
        "selected_files": len(unique_paths),
        "matched_files": len(matched),
        "recall": len(matched) / len(expected),
        "precision": len(matched) / len(unique_paths) if unique_paths else 0.0,
        "reciprocal_rank": 1.0 / first_rank if first_rank else 0.0,
    }


def evaluate_mode(
    binary: Path,
    repo: Path,
    env: dict[str, str],
    mode: str,
    budget: int,
    limit: int,
    tasks: list[dict[str, Any]],
) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    for task in tasks:
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
        if mode == "context":
            paths = [normalize_path(item["file_path"], repo) for item in payload["items"]]
            used_tokens = int(payload["used_tokens"])
            candidate_count = int(payload["candidate_count"])
        else:
            hits = payload["hits"] if isinstance(payload, dict) else payload
            paths = [normalize_path(hit["file_path"], repo) for hit in hits]
            used_tokens = 0
            candidate_count = len(hits)
        row = {
            **task,
            **retrieval_metrics(paths, task["expected_paths"]),
            "latency_ms": latency_ms,
            "used_tokens": used_tokens,
            "candidate_count": candidate_count,
            "paths": paths,
        }
        rows.append(row)

    latencies = [row["latency_ms"] for row in rows]
    return {
        "mode": mode,
        "queries": len(rows),
        "mean_recall": statistics.fmean(row["recall"] for row in rows),
        "mean_precision": statistics.fmean(row["precision"] for row in rows),
        "mean_reciprocal_rank": statistics.fmean(row["reciprocal_rank"] for row in rows),
        "mean_used_tokens": statistics.fmean(row["used_tokens"] for row in rows),
        "latency_p50_ms": statistics.median(latencies),
        "latency_p95_ms": percentile(latencies, 95),
        "rows": rows,
    }


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
    return parser.parse_args()


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
        tasks = json.loads(args.tasks_from.read_text())["tasks"]
    else:
        tasks = historical_tasks(repo, args.tasks)
    with tempfile.TemporaryDirectory(prefix="ivygrep-context-bench-") as home:
        env = os.environ.copy()
        env["IVYGREP_HOME"] = home
        env["IVYGREP_NO_AUTOSPAWN"] = "1"
        env["IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT"] = "1"
        env["IVYGREP_ENHANCE_MAX_LOAD_RATIO"] = "0"
        run(
            [str(binary), "--add", str(repo), "--force", "--hash", "--no-watch"],
            repo,
            env,
        )
        run([str(binary), "--enhance-hash-internal", str(repo)], repo, env)
        results = [
            evaluate_mode(
                binary,
                repo,
                env,
                mode,
                args.budget,
                args.search_limit,
                tasks,
            )
            for mode in modes
        ]
    output = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "binary": str(binary),
        "binary_sha256": sha256_file(binary),
        "binary_version": binary_version,
        "repo": str(repo),
        "repo_commit": git(repo, "rev-parse", "HEAD").strip(),
        "repo_dirty": bool(git(repo, "status", "--porcelain").strip()),
        "harness_sha256": sha256_file(Path(__file__)),
        "budget": args.budget,
        "search_limit": args.search_limit,
        "tasks": tasks,
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps({result["mode"]: {key: value for key, value in result.items() if key != "rows"} for result in results}, indent=2))


if __name__ == "__main__":
    main()
