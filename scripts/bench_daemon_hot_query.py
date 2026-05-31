#!/usr/bin/env python3
"""Measure process-cold, warm distinct-query, and cache-replay search latency."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import signal
import statistics
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


DEFAULT_KERNEL = Path("/home/bruno/githubworkspace/linux")
DEFAULT_HOME = Path("/tmp/ivygrep-daemon-hot-bench-home")
DEFAULT_QUERY = "kernel memory allocation"
DEFAULT_DISTINCT_QUERIES = [
    "scheduler task wakeup",
    "virtual memory page fault",
    "network socket receive buffer",
    "filesystem inode lookup",
    "device driver interrupt handler",
    "process signal delivery",
    "mutex lock contention",
]
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


def has_index(bench_home: Path) -> bool:
    indexes_dir = bench_home / "indexes"
    return indexes_dir.is_dir() and any(indexes_dir.glob("*/metadata.sqlite3"))


class DaemonProcess:
    def __init__(self, proc: subprocess.Popen[str], log_file: Any) -> None:
        self.proc = proc
        self.log_file = log_file

    def stop(self) -> None:
        if self.proc.poll() is None:
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(self.proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.proc.wait(timeout=5)
        self.log_file.close()


def start_daemon(binary: Path, *, cwd: Path, env: dict[str, str], bench_home: Path) -> DaemonProcess:
    socket = bench_home / "daemon.sock"
    socket.unlink(missing_ok=True)
    log_path = bench_home / "bench_daemon.log"
    log_file = log_path.open("ab")
    proc = subprocess.Popen(
        [str(binary), "--daemon"],
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=False,
        start_new_session=True,
    )
    daemon = DaemonProcess(proc=proc, log_file=log_file)
    try:
        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                raise SystemExit(f"daemon exited early; see {log_path}")
            if socket.exists():
                try:
                    run([str(binary), "--status", "--json"], cwd=cwd, env=env)
                    return daemon
                except subprocess.CalledProcessError:
                    pass
            time.sleep(0.05)
        raise SystemExit(f"daemon socket not ready at {socket}; see {log_path}")
    except BaseException:
        daemon.stop()
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, default=DEFAULT_KERNEL)
    parser.add_argument("--bench-home", type=Path, default=DEFAULT_HOME)
    parser.add_argument("--query", default=DEFAULT_QUERY)
    parser.add_argument(
        "--distinct-query",
        action="append",
        dest="distinct_queries",
        help="warm daemon cache-miss query; repeat for multiple samples",
    )
    parser.add_argument("--limit", type=int, default=20)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--reindex", action="store_true")
    parser.add_argument("--binary", type=Path, default=None)
    args = parser.parse_args()

    if args.samples < 1:
        raise SystemExit("--samples must be at least 1")
    if args.warmups < 0:
        raise SystemExit("--warmups must be non-negative")

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

    index_created = False
    index_seconds = 0.0
    if args.reindex or not has_index(bench_home):
        shutil.rmtree(bench_home, ignore_errors=True)
        bench_home.mkdir(parents=True, exist_ok=True)
        index_created = True
        index_seconds, _ = timed(
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

    def query_cmd(query: str, *, no_watch: bool = False) -> list[str]:
        query_args = ["--hash", "--json"]
        if no_watch:
            query_args.append("--no-watch")
        query_args.extend(["-n", str(args.limit), query, str(kernel)])
        return [str(binary), *query_args]

    local_cmd = query_cmd(args.query, no_watch=True)
    daemon_cmd = query_cmd(args.query)
    distinct_queries = args.distinct_queries or DEFAULT_DISTINCT_QUERIES

    local_ms: list[float] = []
    local_hits = 0
    for _ in range(args.samples):
        seconds, stdout = timed(local_cmd, cwd=repo, env=env)
        local_ms.append(seconds * 1000.0)
        local_hits = max(local_hits, hit_count(stdout))

    daemon = start_daemon(binary, cwd=repo, env=env, bench_home=bench_home)
    try:
        first_seconds, first_stdout = timed(daemon_cmd, cwd=repo, env=env)
        daemon_hits = hit_count(first_stdout)
        for _ in range(args.warmups):
            _, stdout = timed(daemon_cmd, cwd=repo, env=env)
            daemon_hits = max(daemon_hits, hit_count(stdout))

        daemon_cache_replay_ms: list[float] = []
        for _ in range(args.samples):
            seconds, stdout = timed(daemon_cmd, cwd=repo, env=env)
            daemon_cache_replay_ms.append(seconds * 1000.0)
            daemon_hits = max(daemon_hits, hit_count(stdout))

        daemon_warm_distinct_ms: list[float] = []
        distinct_hits = 0
        for i in range(args.samples):
            seconds, stdout = timed(
                query_cmd(distinct_queries[i % len(distinct_queries)]),
                cwd=repo,
                env=env,
            )
            daemon_warm_distinct_ms.append(seconds * 1000.0)
            distinct_hits += hit_count(stdout)
    finally:
        daemon.stop()

    local_p95_ms = percentile(local_ms, 0.95)
    daemon_cache_replay_p95_ms = percentile(daemon_cache_replay_ms, 0.95)
    daemon_warm_distinct_p95_ms = percentile(daemon_warm_distinct_ms, 0.95)
    local_median_ms = statistics.median(local_ms)
    daemon_cache_replay_median_ms = statistics.median(daemon_cache_replay_ms)
    daemon_warm_distinct_median_ms = statistics.median(daemon_warm_distinct_ms)
    metrics = {
        "primary_score_ms": daemon_warm_distinct_p95_ms,
        "daemon_warm_distinct_p95_ms": daemon_warm_distinct_p95_ms,
        "daemon_warm_distinct_median_ms": daemon_warm_distinct_median_ms,
        "daemon_cache_replay_p95_ms": daemon_cache_replay_p95_ms,
        "daemon_cache_replay_median_ms": daemon_cache_replay_median_ms,
        # Backward-compatible aliases for historical benchmark consumers.
        "daemon_hot_p95_ms": daemon_cache_replay_p95_ms,
        "daemon_hot_median_ms": daemon_cache_replay_median_ms,
        "daemon_first_query_ms": first_seconds * 1000.0,
        "local_process_cold_p95_ms": local_p95_ms,
        "local_process_cold_median_ms": local_median_ms,
        "daemon_speedup_vs_local_median": (
            local_median_ms / daemon_warm_distinct_median_ms
            if daemon_warm_distinct_median_ms > 0
            else 0.0
        ),
        "daemon_speedup_vs_local_p95": local_p95_ms / daemon_warm_distinct_p95_ms
        if daemon_warm_distinct_p95_ms > 0
        else 0.0,
        "local_hits": local_hits,
        "daemon_hits": daemon_hits,
        "distinct_hits": distinct_hits,
        "samples": args.samples,
        "warmups": args.warmups,
        "build_seconds": build_seconds,
        "index_created": int(index_created),
        "index_seconds": index_seconds,
    }

    print(json.dumps(metrics, sort_keys=True))
    if local_hits == 0 or daemon_hits == 0:
        raise SystemExit("benchmark query returned no hits")
    return 0


if __name__ == "__main__":
    sys.exit(main())
