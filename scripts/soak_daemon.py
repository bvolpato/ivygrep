#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Check daemon correctness under mutation/restart load and bound resource growth."""

from __future__ import annotations

import argparse
from collections import deque
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import math
import os
import platform
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import threading
import time
from typing import Any

from check_daemon_equivalence import daemon_request, start_daemon


QUERIES = (
    "error handling and recovery", "background enhancement worker",
    "workspace index status", "filtered semantic search",
    "context graph dependency", "daemon request concurrency",
)
PROBE = "src/soak_probe.rs"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(command: list[str], cwd: Path, env: dict[str, str]) -> str:
    return subprocess.run(command, cwd=cwd, env=env, check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300).stdout


def percentile(values: list[float], percentile_value: int) -> float:
    ordered = sorted(values)
    index = math.ceil(len(ordered) * percentile_value / 100) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def process_sample(pid: int) -> dict[str, int]:
    # Missing/inaccessible processes are failures, never reassuring zero samples.
    proc = Path("/proc") / str(pid)
    status = (proc / "status").read_text()
    fields = dict(line.split(":", 1) for line in status.splitlines() if ":" in line)
    sample = {"rss_bytes": int(fields["VmRSS"].split()[0]) * 1024,
              "fds": len(list((proc / "fd").iterdir())), "threads": int(fields["Threads"])}
    if min(sample.values()) <= 0:
        raise RuntimeError(f"invalid process sample: {sample}")
    return sample


def resource_gate(samples: list[dict[str, Any]], budgets: dict[str, int]) -> dict[str, Any]:
    """Compare steady-state windows within ONE PID; restarts cannot mask growth."""
    if len(samples) < 20:
        raise ValueError("at least 20 load samples per daemon epoch are required")
    warm = samples[math.ceil(len(samples) * 0.2):]
    width = max(3, len(warm) // 4)
    metrics = {}
    for resource, budget in budgets.items():
        first = statistics.median(sample[resource] for sample in warm[:width])
        last = statistics.median(sample[resource] for sample in warm[-width:])
        metrics[resource] = {"baseline": first, "tail": last, "growth": last - first,
                             "budget": budget, "passed": last - first <= budget,
                             "peak": max(sample[resource] for sample in samples)}
    return {"passed": all(metric["passed"] for metric in metrics.values()),
            "sample_count": len(samples), "metrics": metrics}


def copy_repo(source: Path, destination: Path) -> None:
    ignored = shutil.ignore_patterns(".git", "target", "node_modules", ".venv", "__pycache__")
    shutil.copytree(source, destination, ignore=ignored)
    for args in (["init", "-q"], ["add", "."], ["commit", "-qm", "benchmark corpus"]):
        subprocess.run(["git", "-c", "user.name=Soak Benchmark", "-c", "user.email=soak@example.invalid",
                        "-c", "commit.gpgSign=false", "-c", "core.hooksPath=/dev/null", *args],
                       cwd=destination, check=True, timeout=120)


def search(home: Path, repo: Path, query: str, *, probe_only: bool = False) -> list[dict[str, Any]]:
    request = {"type": "search", "path": str(repo), "query": query, "limit": 10,
               "context": 0, "type_filter": None, "scope_path": None}
    if probe_only:
        request["include_globs"] = [PROBE]
    response = daemon_request(home, request)
    if response.get("type") != "search_results":
        raise RuntimeError(f"unexpected daemon search response: {response}")
    return response["hits"]


def probe_matches(hits: list[dict[str, Any]], expected: str | None) -> bool:
    if expected is None:
        return not hits
    return len(hits) == 1 and hits[0]["file_path"].replace("\\", "/") == PROBE and hits[0]["preview"].strip() == expected


def watcher_observed_probe(home: Path, repo: Path, expected: str | None) -> None:
    deadline = time.monotonic() + 20
    while True:
        # A stable query deliberately reuses the daemon cache. Verify exact
        # indexed text, not a filename or a literal preview loaded from disk.
        hits = search(home, repo, "soak revision value", probe_only=True)
        if probe_matches(hits, expected):
            return
        if time.monotonic() >= deadline:
            raise AssertionError(f"watcher returned stale probe: expected={expected!r}, hits={hits}")
        time.sleep(0.1)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--duration", type=float, default=300.0, help="total loaded seconds, excluding cooldowns")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--mutation-interval", type=float, default=0.25)
    parser.add_argument("--check-interval", type=float, default=10.0)
    parser.add_argument("--cooldown", type=float, default=10.0)
    parser.add_argument("--warmup", type=float, default=30.0,
                        help="run the same mutation/query workload before sampling each PID")
    parser.add_argument("--restarts", type=int, default=2)
    parser.add_argument("--rss-growth-mib", type=float, default=32.0)
    parser.add_argument("--fd-growth", type=int, default=8)
    parser.add_argument("--thread-growth", type=int, default=4)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not Path("/proc/self/status").is_file() or not hasattr(os, "killpg"):
        parser.error("daemon soak requires Linux /proc and process groups")
    numeric = (args.duration, args.mutation_interval, args.check_interval, args.cooldown, args.warmup,
               args.rss_growth_mib)
    if not all(math.isfinite(value) for value in numeric):
        parser.error("numeric arguments must be finite")
    if args.restarts < 0 or args.workers < 1 or args.duration / (args.restarts + 1) < 30:
        parser.error("positive workers and at least 30 loaded seconds per daemon epoch are required")
    if min(args.mutation_interval, args.check_interval) <= 0 or min(args.cooldown, args.warmup, args.rss_growth_mib, args.fd_growth, args.thread_growth) < 0:
        parser.error("intervals must be positive and budgets/cooldown nonnegative")
    binary, source = args.binary.resolve(), args.repo.resolve()
    env = os.environ.copy()
    budgets = {"rss_bytes": int(args.rss_growth_mib * 1024 * 1024),
               "fds": args.fd_growth, "threads": args.thread_growth}
    report: dict[str, Any] = {
        "schema_version": 2, "generated_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(), "machine": platform.machine(),
        "cpu_affinity": sorted(os.sched_getaffinity(0)),
        "binary_sha256": sha256_file(binary), "binary_version": run([str(binary), "--version"], source, env).strip(),
        "source_commit": run(["git", "rev-parse", "HEAD"], source, env).strip(),
        "source_dirty": bool(run(["git", "status", "--porcelain"], source, env).strip()),
        "harness_sha256": sha256_file(Path(__file__)), "load_duration_seconds": args.duration,
        "workers": args.workers, "mutation_interval_seconds": args.mutation_interval,
        "cooldown_seconds": args.cooldown, "requested_restarts": args.restarts,
        "warmup_seconds_per_epoch": args.warmup,
        "transport": "direct-daemon-rpc-no-cli-fallback", "resource_budgets": budgets,
        "epochs": [], "passed": False,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    failure = None
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="ivygrep-soak-") as temp:
        root = Path(temp)
        repo, home = root / "repo", root / "home"
        env.update(IVYGREP_HOME=str(home), IVYGREP_NO_AUTOSPAWN="1",
                   IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT="1")
        daemon = None
        try:
            copy_repo(source, repo)
            run([str(binary), "--add", str(repo), "--force", "--hash"], repo, env)
            run([str(binary), "--enhance-hash-internal", str(repo)], repo, env)
            probe = repo / PROBE
            probe.parent.mkdir(exist_ok=True)
            revision = 0
            for epoch in range(args.restarts + 1):
                # Alternate offline changes and deletes before starting the next PID.
                revision += 1
                expected = f"pub fn soak_revision() -> u64 {{ {revision} }}" if epoch % 2 == 0 else None
                if expected is None:
                    probe.unlink(missing_ok=True)
                else:
                    probe.write_text(expected + "\n")
                daemon = start_daemon(binary, cwd=repo, env=env, bench_home=home)
                watcher_observed_probe(home, repo, expected)
                record: dict[str, Any] = {"epoch": epoch, "pid": daemon.proc.pid, "queries": 0,
                                         "query_errors": 0, "errors": [], "samples": [],
                                         "mutations": 0, "correctness_checks": 1}
                report["epochs"].append(record)
                stop = threading.Event()
                latencies: deque[float] = deque(maxlen=50000)
                lock = threading.Lock()

                def query_worker(worker: int) -> None:
                    iteration = 0
                    while not stop.is_set():
                        before = time.perf_counter()
                        try:
                            search(home, repo, QUERIES[(worker + iteration) % len(QUERIES)])
                            with lock:
                                record["queries"] += 1
                                latencies.append((time.perf_counter() - before) * 1000)
                        except Exception as error:
                            with lock:
                                record["query_errors"] += 1
                                if len(record["errors"]) < 5:
                                    record["errors"].append(str(error))
                            stop.set()
                        iteration += 1
                        stop.wait(0.01)

                epoch_started = time.monotonic()
                next_mutation = next_sample = epoch_started
                next_check = epoch_started + args.check_interval
                with ThreadPoolExecutor(max_workers=args.workers) as executor:
                    futures = [executor.submit(query_worker, worker) for worker in range(args.workers)]
                    try:
                        while time.monotonic() - epoch_started < args.warmup + args.duration / (args.restarts + 1):
                            if stop.is_set():
                                raise RuntimeError(f"daemon query failed: {record['errors']}")
                            now = time.monotonic()
                            if now >= next_mutation:
                                revision += 1
                                expected = f"pub fn soak_revision() -> u64 {{ {revision} }}"
                                probe.write_text(expected + "\n")
                                record["mutations"] += 1
                                next_mutation = now + args.mutation_interval
                            if now >= next_sample and now - epoch_started >= args.warmup:
                                record["samples"].append({"elapsed_seconds": now - epoch_started - args.warmup,
                                                          **process_sample(daemon.proc.pid)})
                                next_sample = now + 1
                            if now >= next_check:
                                watcher_observed_probe(home, repo, expected)
                                probe.unlink()
                                watcher_observed_probe(home, repo, None)
                                probe.write_text(expected + "\n")
                                watcher_observed_probe(home, repo, expected)
                                record["correctness_checks"] += 3
                                next_check = time.monotonic() + args.check_interval
                            stop.wait(0.02)
                    finally:
                        stop.set()
                        for future in futures:
                            future.result(timeout=35)
                watcher_observed_probe(home, repo, expected)
                record["correctness_checks"] += 1
                record["resource_gate"] = resource_gate(record["samples"], budgets)
                record["latency_sample_count"] = len(latencies)
                record["latency_window"] = "last 50000 successful requests of this epoch"
                record["latency_p50_ms"] = statistics.median(latencies) if latencies else None
                record["latency_p95_ms"] = percentile(list(latencies), 95) if latencies else None
                if not record["queries"] or record["query_errors"] or not record["resource_gate"]["passed"]:
                    raise AssertionError(f"epoch {epoch} failed query/resource gates")
                cooldown_deadline = time.monotonic() + args.cooldown
                record["cooldown_samples"] = []
                while time.monotonic() < cooldown_deadline:
                    record["cooldown_samples"].append(process_sample(daemon.proc.pid))
                    time.sleep(min(1, max(0, cooldown_deadline - time.monotonic())))
                daemon.stop()
                daemon = None
                print(f"epoch {epoch} passed: {record['queries']} queries, {record['correctness_checks']} correctness checks", flush=True)
            report["passed"] = True
        except Exception as error:
            failure = error
            report["failure"] = str(error)
        finally:
            if daemon is not None:
                daemon.stop()
            log = home / "equivalence-daemon.log"
            if log.exists():
                shutil.copyfile(log, args.output.with_suffix(".daemon.log"))
            report["duration_seconds"] = time.monotonic() - started
            args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "epochs"}, indent=2))
    if failure is not None:
        raise RuntimeError(f"soak validation failed; evidence: {args.output}") from failure


if __name__ == "__main__":
    main()
