#!/usr/bin/env python3
"""Measure ivygrep cold index and base-search queries on a Linux checkout."""

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
DEFAULT_COMPLEX_QUERY = "kernel memory allocation"
DEFAULT_SIMPLE_QUERY = "kmalloc"
DEFAULT_LITERAL_QUERY = "kmalloc"
DEFAULT_REGEX_QUERY = r"kmalloc\s*\("
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


def ensure_existing_index(path: Path) -> None:
    indexes_dir = path / "indexes"
    if not indexes_dir.is_dir() or not any(indexes_dir.glob("*/metadata.sqlite3")):
        raise SystemExit(f"--skip-index needs existing benchmark index under {path}")


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
    parser.add_argument("--query", default=None, help="Alias for --complex-query")
    parser.add_argument("--complex-query", default=DEFAULT_COMPLEX_QUERY)
    parser.add_argument("--simple-query", default=DEFAULT_SIMPLE_QUERY)
    parser.add_argument("--literal-query", default=DEFAULT_LITERAL_QUERY)
    parser.add_argument("--regex-query", default=DEFAULT_REGEX_QUERY)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--skip-index", action="store_true")
    parser.add_argument("--binary", type=Path, default=None)
    args = parser.parse_args()
    if args.query is not None:
        args.complex_query = args.query

    repo = Path(__file__).resolve().parent.parent
    kernel = args.kernel.resolve()
    bench_home = ensure_bench_home_under_tmp(args.bench_home)
    binary = (
        args.binary.resolve()
        if args.binary is not None
        else repo / "target" / "release" / "ig"
    )
    ensure_kernel_checkout(kernel)

    env = os.environ.copy()
    env["IVYGREP_HOME"] = str(bench_home)
    env["IVYGREP_NO_AUTOSPAWN"] = "1"
    env["RUST_BACKTRACE"] = env.get("RUST_BACKTRACE", "1")

    build_seconds = 0.0
    if not args.skip_build and args.binary is None:
        build_seconds, _ = timed(
            ["cargo", "build", "--release", "--locked", "--bin", "ig"],
            cwd=repo,
            env=env,
        )
    if not binary.exists():
        raise SystemExit(f"missing release binary at {binary}")

    index_seconds = 0.0
    index_summary: dict[str, Any] = {}
    if args.skip_index:
        ensure_existing_index(bench_home)
    else:
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

    complex_cmd = [
        str(binary),
        "--hash",
        "--json",
        "--no-watch",
        "-n",
        "20",
        args.complex_query,
        str(kernel),
    ]
    simple_cmd = [
        str(binary),
        "--hash",
        "--json",
        "--no-watch",
        "-n",
        "20",
        args.simple_query,
        str(kernel),
    ]
    literal_cmd = [
        str(binary),
        "--literal",
        "--json",
        "--no-watch",
        "-n",
        "20",
        args.literal_query,
        str(kernel),
    ]
    regex_cmd = [
        str(binary),
        "--regex",
        "--json",
        "--no-watch",
        "-n",
        "20",
        args.regex_query,
        str(kernel),
    ]

    def hot_samples(cmd: list[str]) -> tuple[list[float], int]:
        ms: list[float] = []
        hits = 0
        for _ in range(args.samples):
            seconds, stdout = timed(cmd, cwd=repo, env=env)
            ms.append(seconds * 1000.0)
            hits = max(hits, hit_count(stdout))
        return ms, hits

    complex_cold_seconds, complex_cold_stdout = timed(complex_cmd, cwd=repo, env=env)
    complex_cold_hits = hit_count(complex_cold_stdout)

    complex_ms, complex_hits = hot_samples(complex_cmd)
    simple_ms, simple_hits = hot_samples(simple_cmd)
    literal_ms, literal_hits = hot_samples(literal_cmd)
    regex_ms, regex_hits = hot_samples(regex_cmd)

    index_ms = index_seconds * 1000.0
    complex_cold_query_ms = complex_cold_seconds * 1000.0
    complex_p95_ms = percentile(complex_ms, 0.95)
    simple_p95_ms = percentile(simple_ms, 0.95)
    literal_p95_ms = percentile(literal_ms, 0.95)
    regex_p95_ms = percentile(regex_ms, 0.95)
    primary_score_ms = (
        index_ms
        + complex_cold_query_ms
        + complex_p95_ms
        + simple_p95_ms
        + literal_p95_ms
        + regex_p95_ms
    )

    metrics = {
        "primary_score_ms": primary_score_ms,
        "cold_index_ms": index_ms,
        "cold_query_ms": complex_cold_query_ms,
        "hot_query_median_ms": statistics.median(complex_ms) if complex_ms else 0.0,
        "hot_query_p95_ms": complex_p95_ms,
        "complex_cold_query_ms": complex_cold_query_ms,
        "complex_hot_median_ms": statistics.median(complex_ms) if complex_ms else 0.0,
        "complex_hot_p95_ms": complex_p95_ms,
        "simple_hot_median_ms": statistics.median(simple_ms) if simple_ms else 0.0,
        "simple_hot_p95_ms": simple_p95_ms,
        "literal_hot_median_ms": statistics.median(literal_ms) if literal_ms else 0.0,
        "literal_hot_p95_ms": literal_p95_ms,
        "regex_hot_median_ms": statistics.median(regex_ms) if regex_ms else 0.0,
        "regex_hot_p95_ms": regex_p95_ms,
        "cold_query_hits": complex_cold_hits,
        "hot_query_hits": complex_hits,
        "complex_hits": complex_hits,
        "simple_hits": simple_hits,
        "literal_hits": literal_hits,
        "regex_hits": regex_hits,
        "indexed_files": int(index_summary.get("indexed_files", 0) or 0),
        "total_chunks": int(index_summary.get("total_chunks", 0) or 0),
        "deleted_files": int(index_summary.get("deleted_files", 0) or 0),
        "index_size_mb": dir_size_bytes(bench_home) / 1024.0 / 1024.0,
        "build_seconds": build_seconds,
        "samples": args.samples,
    }

    if (
        complex_cold_hits == 0
        or complex_hits == 0
        or simple_hits == 0
        or literal_hits == 0
        or regex_hits == 0
    ):
        print(json.dumps(metrics, sort_keys=True))
        raise SystemExit("benchmark query returned no hits")

    print(json.dumps(metrics, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
