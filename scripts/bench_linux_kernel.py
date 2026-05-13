#!/usr/bin/env python3
"""Measure ivygrep cold index, cold query, and hot query on a Linux checkout."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


DEFAULT_KERNEL = Path("/home/bruno/githubworkspace/linux")
DEFAULT_HOME = Path("/tmp/ivygrep-linux-bench-home")
DEFAULT_QUERY = "kernel memory allocation"
DEFAULT_LITERAL_QUERY = "kmalloc"
TMP_ROOT = Path("/tmp").resolve()


def run(cmd: list[str], *, cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=True, check=True)


def timed(cmd: list[str], *, cwd: Path, env: dict[str, str]) -> tuple[float, str]:
    start = time.perf_counter()
    result = run(cmd, cwd=cwd, env=env)
    return time.perf_counter() - start, result.stdout


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    rank = (len(ordered) - 1) * pct
    low = int(rank)
    high = min(low + 1, len(ordered) - 1)
    if low == high:
        return ordered[low]
    frac = rank - low
    return ordered[low] + (ordered[high] - ordered[low]) * frac


def hit_count(output: str) -> int:
    try:
        parsed: Any = json.loads(output)
    except json.JSONDecodeError:
        return 0
    if not isinstance(parsed, list):
        return 0
    return sum(len(item.get("hits", [])) for item in parsed if isinstance(item, dict))


def dir_size_bytes(path: Path) -> int:
    total = 0
    if not path.exists():
        return total
    for entry in path.rglob("*"):
        if entry.is_file():
            try:
                total += entry.stat().st_size
            except OSError:
                pass
    return total


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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, default=DEFAULT_KERNEL)
    parser.add_argument("--bench-home", type=Path, default=DEFAULT_HOME)
    parser.add_argument("--query", default=DEFAULT_QUERY)
    parser.add_argument("--literal-query", default=DEFAULT_LITERAL_QUERY)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parent.parent
    kernel = args.kernel.resolve()
    bench_home = ensure_bench_home_under_tmp(args.bench_home)
    binary = repo / "target" / "release" / "ig"
    ensure_kernel_checkout(kernel)

    env = os.environ.copy()
    env["IVYGREP_HOME"] = str(bench_home)
    env["IVYGREP_NO_AUTOSPAWN"] = "1"
    env["RUST_BACKTRACE"] = env.get("RUST_BACKTRACE", "1")

    build_seconds = 0.0
    if not args.skip_build:
        build_seconds, _ = timed(
            ["cargo", "build", "--release", "--locked", "--bin", "ig"],
            cwd=repo,
            env=env,
        )
    if not binary.exists():
        raise SystemExit(f"missing release binary at {binary}")

    shutil.rmtree(bench_home, ignore_errors=True)
    bench_home.mkdir(parents=True, exist_ok=True)

    index_seconds, index_stdout = timed(
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
    try:
        index_summary = json.loads(index_stdout)
    except json.JSONDecodeError:
        index_summary = {}

    query_cmd = [
        str(binary),
        "--hash",
        "--json",
        "--no-watch",
        "-n",
        "20",
        args.query,
        str(kernel),
    ]
    literal_cmd = [
        str(binary),
        "--hash",
        "--literal",
        "--json",
        "--no-watch",
        "-n",
        "20",
        args.literal_query,
        str(kernel),
    ]

    cold_query_seconds, cold_stdout = timed(query_cmd, cwd=repo, env=env)
    cold_hits = hit_count(cold_stdout)

    hot_ms: list[float] = []
    hot_hits = 0
    for _ in range(args.samples):
        seconds, stdout = timed(query_cmd, cwd=repo, env=env)
        hot_ms.append(seconds * 1000.0)
        hot_hits = max(hot_hits, hit_count(stdout))

    literal_ms: list[float] = []
    literal_hits = 0
    for _ in range(max(3, args.samples // 2)):
        seconds, stdout = timed(literal_cmd, cwd=repo, env=env)
        literal_ms.append(seconds * 1000.0)
        literal_hits = max(literal_hits, hit_count(stdout))

    index_ms = index_seconds * 1000.0
    cold_query_ms = cold_query_seconds * 1000.0
    hot_p95_ms = percentile(hot_ms, 0.95)
    literal_p95_ms = percentile(literal_ms, 0.95)
    primary_score_ms = index_ms + cold_query_ms + hot_p95_ms

    metrics = {
        "primary_score_ms": primary_score_ms,
        "cold_index_ms": index_ms,
        "cold_query_ms": cold_query_ms,
        "hot_query_median_ms": statistics.median(hot_ms) if hot_ms else 0.0,
        "hot_query_p95_ms": hot_p95_ms,
        "literal_hot_median_ms": statistics.median(literal_ms) if literal_ms else 0.0,
        "literal_hot_p95_ms": literal_p95_ms,
        "cold_query_hits": cold_hits,
        "hot_query_hits": hot_hits,
        "literal_hits": literal_hits,
        "indexed_files": int(index_summary.get("indexed_files", 0) or 0),
        "total_chunks": int(index_summary.get("total_chunks", 0) or 0),
        "deleted_files": int(index_summary.get("deleted_files", 0) or 0),
        "index_size_mb": dir_size_bytes(bench_home) / 1024.0 / 1024.0,
        "build_seconds": build_seconds,
        "samples": args.samples,
    }

    if cold_hits == 0 or hot_hits == 0 or literal_hits == 0:
        print(json.dumps(metrics, sort_keys=True))
        raise SystemExit("benchmark query returned no hits")

    print(json.dumps(metrics, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
