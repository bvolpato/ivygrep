#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Run concurrent daemon search and watcher churn while sampling resources."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import signal
import statistics
import subprocess
import tempfile
import threading
import time
from typing import Any


QUERIES = (
    "error handling and recovery",
    "background enhancement worker",
    "workspace index status",
    "filtered semantic search",
    "context graph dependency",
    "daemon request concurrency",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(command: list[str], cwd: Path, env: dict[str, str]) -> str:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def percentile(values: list[float], percentile_value: int) -> float:
    ordered = sorted(values)
    index = math.ceil(len(ordered) * percentile_value / 100) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def process_sample(pid: int) -> dict[str, int]:
    proc = Path("/proc") / str(pid)
    if not proc.exists():
        return {"rss_bytes": 0, "fds": 0, "threads": 0}
    try:
        status = (proc / "status").read_text()
        fds = len(list((proc / "fd").iterdir()))
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return {"rss_bytes": 0, "fds": 0, "threads": 0}
    fields = {
        line.split(":", 1)[0]: line.split(":", 1)[1].strip()
        for line in status.splitlines()
        if ":" in line
    }
    rss_kib = int(fields.get("VmRSS", "0 kB").split()[0])
    return {
        "rss_bytes": rss_kib * 1024,
        "fds": fds,
        "threads": int(fields.get("Threads", "0")),
    }


def wait_for_daemon(home: Path, daemon: subprocess.Popen[bytes]) -> None:
    endpoint = home / ("daemon.port" if os.name == "nt" else "daemon.sock")
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if daemon.poll() is not None:
            raise RuntimeError(f"daemon exited with {daemon.returncode}")
        if endpoint.exists():
            return
        time.sleep(0.05)
    raise TimeoutError("daemon endpoint did not appear")


def copy_repo(source: Path, destination: Path) -> None:
    ignored = shutil.ignore_patterns(".git", "target", "node_modules", ".venv", "__pycache__")
    shutil.copytree(source, destination, ignore=ignored)
    subprocess.run(["git", "init", "-q"], cwd=destination, check=True)
    subprocess.run(["git", "add", "."], cwd=destination, check=True)
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=Soak Benchmark",
            "-c",
            "user.email=soak@example.com",
            "-c",
            "commit.gpgSign=false",
            "commit",
            "-qm",
            "benchmark corpus",
        ],
        cwd=destination,
        check=True,
    )


def watcher_observed_probe(binary: Path, repo: Path, env: dict[str, str]) -> bool:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        payload = json.loads(
            run(
                [
                    str(binary),
                    "--json",
                    "--hash",
                    "--no-watch",
                    "--limit",
                    "10",
                    "soak_revision",
                    str(repo),
                ],
                repo,
                env,
            )
        )
        hits = payload.get("hits", payload) if isinstance(payload, dict) else payload
        if any(str(hit["file_path"]).endswith("src/soak_probe.rs") for hit in hits):
            return True
        time.sleep(0.1)
    return False


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--duration", type=float, default=300.0)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--mutation-interval", type=float, default=0.25)
    parser.add_argument("--cooldown", type=float, default=10.0)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not Path("/proc/self/status").is_file() or not hasattr(os, "killpg"):
        parser.error("daemon soak requires Linux /proc and process groups")
    binary = args.binary.resolve()
    source = args.repo.resolve()
    source_commit = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=source,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    source_dirty = bool(
        subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=source,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.strip()
    )

    with tempfile.TemporaryDirectory(prefix="ivygrep-soak-") as temp:
        root = Path(temp)
        repo = root / "repo"
        home = root / "home"
        copy_repo(source, repo)
        env = os.environ.copy()
        env["IVYGREP_HOME"] = str(home)
        env["IVYGREP_NO_AUTOSPAWN"] = "1"
        env["IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT"] = "1"
        run([str(binary), "--add", str(repo), "--force", "--hash"], repo, env)
        run([str(binary), "--enhance-hash-internal", str(repo)], repo, env)
        daemon_env = env.copy()
        daemon_env.pop("IVYGREP_NO_AUTOSPAWN")
        daemon_log = (root / "daemon.log").open("wb")
        daemon = subprocess.Popen(
            [str(binary), "--daemon"],
            cwd=repo,
            env=daemon_env,
            stdout=daemon_log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        wait_for_daemon(home, daemon)

        stop = threading.Event()
        latencies: list[float] = []
        errors: list[str] = []
        error_count = 0
        lock = threading.Lock()

        def query_worker(worker: int) -> None:
            nonlocal error_count
            iteration = 0
            while not stop.is_set():
                query = QUERIES[(worker + iteration) % len(QUERIES)]
                started = time.perf_counter()
                try:
                    run(
                        [
                            str(binary),
                            "--json",
                            "--hash",
                            "--no-watch",
                            "--limit",
                            "10",
                            query,
                            str(repo),
                        ],
                        repo,
                        daemon_env,
                    )
                    elapsed = (time.perf_counter() - started) * 1000
                    with lock:
                        latencies.append(elapsed)
                except subprocess.CalledProcessError as error:
                    with lock:
                        error_count += 1
                        if len(errors) < 5:
                            errors.append((error.stderr or str(error)).strip())
                iteration += 1

        samples: list[dict[str, Any]] = []
        probe = repo / "src" / "soak_probe.rs"
        probe.parent.mkdir(exist_ok=True)
        started = time.monotonic()
        next_mutation = started
        next_sample = started
        mutation_observed = False
        load_end_sample: dict[str, Any] = {}
        try:
            with ThreadPoolExecutor(max_workers=args.workers) as executor:
                futures = [executor.submit(query_worker, worker) for worker in range(args.workers)]
                revision = 0
                while time.monotonic() - started < args.duration:
                    now = time.monotonic()
                    if now >= next_mutation:
                        revision += 1
                        probe.write_text(
                            f"pub fn soak_revision() -> u64 {{ {revision} }}\n",
                            encoding="utf-8",
                        )
                        next_mutation += args.mutation_interval
                    if now >= next_sample:
                        samples.append(
                            {
                                "elapsed_seconds": now - started,
                                **process_sample(daemon.pid),
                            }
                        )
                        next_sample += 1.0
                    time.sleep(0.02)
                stop.set()
                for future in futures:
                    future.result(timeout=30)
                mutation_observed = watcher_observed_probe(binary, repo, daemon_env)
                load_end_sample = {
                    "elapsed_seconds": time.monotonic() - started,
                    **process_sample(daemon.pid),
                }
                samples.append(load_end_sample)
                cooldown_deadline = time.monotonic() + args.cooldown
                while time.monotonic() < cooldown_deadline:
                    samples.append(
                        {
                            "elapsed_seconds": time.monotonic() - started,
                            **process_sample(daemon.pid),
                        }
                    )
                    time.sleep(1.0)
        finally:
            stop.set()
            if daemon.poll() is None:
                os.killpg(daemon.pid, signal.SIGTERM)
                try:
                    daemon.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(daemon.pid, signal.SIGKILL)
            daemon_log.close()

        rss_values = [sample["rss_bytes"] for sample in samples]
        fd_values = [sample["fds"] for sample in samples]
        thread_values = [sample["threads"] for sample in samples]
        warm_index = min(len(samples) - 1, math.ceil(len(samples) * 0.2)) if samples else 0
        report = {
            "schema_version": 1,
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "binary_sha256": sha256_file(binary),
            "binary_version": run([str(binary), "--version"], repo, env).strip(),
            "source_commit": source_commit,
            "source_dirty": source_dirty,
            "harness_sha256": sha256_file(Path(__file__)),
            "duration_seconds": time.monotonic() - started,
            "load_duration_seconds": args.duration,
            "cooldown_seconds": args.cooldown,
            "workers": args.workers,
            "mutation_interval_seconds": args.mutation_interval,
            "queries": len(latencies),
            "query_errors": error_count,
            "query_error_samples": errors,
            "watcher_observed_mutation": mutation_observed,
            "latency_p50_ms": statistics.median(latencies) if latencies else None,
            "latency_p95_ms": percentile(latencies, 95) if latencies else None,
            "resource": {
                "rss_start_bytes": rss_values[0] if rss_values else 0,
                "rss_end_bytes": rss_values[-1] if rss_values else 0,
                "rss_peak_bytes": max(rss_values, default=0),
                "rss_at_load_end_bytes": load_end_sample.get("rss_bytes", 0),
                "rss_reclaimed_during_cooldown_bytes": max(
                    0,
                    load_end_sample.get("rss_bytes", 0)
                    - (rss_values[-1] if rss_values else 0),
                ),
                "rss_growth_after_warmup_bytes": (
                    rss_values[-1] - rss_values[warm_index] if rss_values else 0
                ),
                "fds_start": fd_values[0] if fd_values else 0,
                "fds_end": fd_values[-1] if fd_values else 0,
                "fds_peak": max(fd_values, default=0),
                "fds_growth_after_warmup": (
                    fd_values[-1] - fd_values[warm_index] if fd_values else 0
                ),
                "threads_start": thread_values[0] if thread_values else 0,
                "threads_end": thread_values[-1] if thread_values else 0,
                "threads_peak": max(thread_values, default=0),
                "threads_growth_after_warmup": (
                    thread_values[-1] - thread_values[warm_index]
                    if thread_values
                    else 0
                ),
            },
            "samples": samples,
        }
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({key: value for key, value in report.items() if key != "samples"}, indent=2))
        if error_count or not mutation_observed:
            raise RuntimeError(
                f"soak validation failed: query_errors={error_count}, "
                f"watcher_observed_mutation={mutation_observed}"
            )


if __name__ == "__main__":
    main()
