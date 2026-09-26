#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Measure model cost and concurrent MCP latency on a pinned source corpus."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import statistics
import subprocess
import tarfile
import threading
import time

from bench_million_chunks import DaemonClient, binary_identity, directory_size, percentile, sha256_file, start_daemon, stop_daemon
from soak_mcp_sessions import McpClient

QUERIES = (
    "how are workspace index updates published",
    "cancel a search that is waiting for CPU capacity",
    "load the neural embedding model",
    "build a context pack within its token budget",
    "filter ignored files from semantic search results",
    "share a base index with a git worktree",
    "handle a failed filesystem watcher",
    "find cached query results for a workspace",
)
PROFILES = ("static-retrieval-v1", "potion-code-16m-v2")


def latency_summary(values: list[float]) -> dict:
    return {"samples": len(values), "p50_ms": percentile(values, 0.50),
            "p95_ms": percentile(values, 0.95), "p99_ms": percentile(values, 0.99),
            "maximum_ms": max(values), "raw_ms": values}


def proc_sample(pid: int) -> dict:
    proc = Path("/proc") / str(pid)
    status = dict(line.split(":", 1) for line in (proc / "status").read_text().splitlines() if ":" in line)
    stat = (proc / "stat").read_text().rsplit(")", 1)[1].split()
    io = dict(line.split(":", 1) for line in (proc / "io").read_text().splitlines())
    return {"rss_bytes": int(status["VmRSS"].split()[0]) * 1024,
            "cpu_ms": (int(stat[11]) + int(stat[12])) * 1000 / os.sysconf("SC_CLK_TCK"),
            "write_bytes": int(io["write_bytes"]), "threads": int(status["Threads"])}


def measured_command(command: list[str], cwd: Path, env: dict, log: Path) -> dict:
    started = time.perf_counter()
    with log.open("wb") as output:
        process = subprocess.Popen(command, cwd=cwd, env=env, stdout=output, stderr=output, start_new_session=True)
        try:
            # wait4 reports this child only. RUSAGE_CHILDREN accumulates earlier runs.
            deadline = time.monotonic() + 600
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    break
                if time.monotonic() >= deadline:
                    raise TimeoutError("benchmark command exceeded 600 seconds")
                time.sleep(0.01)
            if process.returncode != 0:
                raise RuntimeError(f"benchmark command exited with {process.returncode}. See {log.name}.")
            return {"elapsed_ms": (time.perf_counter() - started) * 1000,
                    "peak_rss_bytes": usage.ru_maxrss * 1024,
                    "cpu_ms": (usage.ru_utime + usage.ru_stime) * 1000,
                    "filesystem_write_bytes": usage.ru_oublock * 512}
        finally:
            if process.returncode is None:
                os.killpg(process.pid, signal.SIGKILL)
                _, status, _ = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)


def export_corpus(source: Path, work: Path, revision: str) -> tuple[Path, dict]:
    corpus = work / "corpus"
    corpus.mkdir()
    archive = work / "corpus.tar"
    subprocess.run(["git", "archive", "--output", str(archive), revision,
                    "src", "README.md", "Cargo.toml", "docs/architecture.md"], cwd=source, check=True)
    with tarfile.open(archive) as bundle:
        bundle.extractall(corpus, filter="data")
    archive.unlink()
    subprocess.run(["git", "init", "-q", str(corpus)], check=True)
    digest = hashlib.sha256()
    files = sorted(path for path in corpus.rglob("*") if path.is_file() and ".git" not in path.parts)
    for path in files:
        digest.update(path.relative_to(corpus).as_posix().encode() + b"\0" + path.read_bytes())
    return corpus, {"revision": revision, "files": len(files),
                    "source_bytes": sum(path.stat().st_size for path in files),
                    "sha256": digest.hexdigest(), "selection": ["src", "README.md", "Cargo.toml", "docs/architecture.md"]}


def diagnostics(log: Path) -> dict:
    stages: dict[str, list[float]] = {}
    scans = scanned = recoveries = 0
    for line in log.read_text(errors="replace").splitlines():
        line = re.sub(r"\x1b\[[0-9;]*m", "", line)
        stage = re.search(r'stage="([^"]+)"', line)
        elapsed = re.search(r"elapsed_ms=([0-9.eE+-]+)", line)
        if stage and elapsed:
            stages.setdefault(stage[1], []).append(float(elapsed[1]))
        if stage and stage[1] == "semantic_refill":
            recoveries += 1
            scans += "exact_scan=true" in line
            keys = re.search(r"scanned_keys=(\d+)", line)
            scanned += int(keys[1]) if keys else 0
    return {"stages": {stage: latency_summary(values) for stage, values in stages.items()},
            "semantic_recoveries": recoveries, "exact_scans": scans, "scanned_keys": scanned}


def benchmark_run(binary: Path, profile: str, corpus: Path, work: Path, args: argparse.Namespace) -> dict:
    home = work / "home"
    home.mkdir(parents=True)
    env = {key: value for key, value in os.environ.items() if not key.startswith("IVYGREP_")}
    env.update(IVYGREP_HOME=str(home), IVYGREP_MODEL_PROFILE=profile,
               IVYGREP_NO_AUTOSPAWN="1", IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT="1",
               IVYGREP_ENHANCE_MAX_LOAD_RATIO="0", IVYGREP_DISABLE_QUERY_CACHE="1",
               HF_HUB_OFFLINE="1", TOKENIZERS_PARALLELISM="false")
    if args.diagnostics:
        env["RUST_LOG"] = "ivygrep::performance=debug"
    phases = {}
    for name, command in (
        ("lexical_index", ["--add", str(corpus), "--no-watch", "--hash"]),
        ("hash_enhancement", ["--enhance-hash-internal", str(corpus)]),
        ("neural_enhancement", ["--enhance-internal", str(corpus)]),
    ):
        phases[name] = measured_command([str(binary), *command], corpus, env, work / f"{name}.log")
    background = work / "background"
    shutil.copytree(corpus, background, ignore=shutil.ignore_patterns(".git"))
    subprocess.run(["git", "init", "-q", str(background)], check=True)
    daemon, log, log_path = start_daemon(binary, corpus, env, home, "load.log")
    clients = []
    stop = threading.Event()
    samples = []
    index_errors = []
    indexing = None
    sampler = None
    completed_indexes = []
    try:
        with DaemonClient(home, corpus, True) as client:
            for query in QUERIES:
                client.query(query)  # Load the model and verify forced neural execution.
        forced = []
        with DaemonClient(home, corpus, True) as client:
            for index in range(args.samples):
                forced.append(client.query(QUERIES[index % len(QUERIES)])["elapsed_ms"])
        for _ in range(args.clients):
            client = McpClient(binary, env, corpus)
            clients.append(client)
            client.initialize()

        def sample() -> None:
            while not stop.is_set():
                try:
                    samples.append(proc_sample(daemon.pid))
                except OSError:
                    return
                stop.wait(0.02)

        def index_load() -> None:
            try:
                with DaemonClient(home, background) as client:
                    client.protocol_version = 9
                    iteration = 0
                    while not stop.is_set():
                        (background / "load_probe.rs").write_text(f"pub fn load_probe() -> usize {{ {iteration} }}\n")
                        response, elapsed = client._send({"protocol_version": 9, "type": "index",
                                                         "path": str(background), "watch": False, "skip_gitignore": False})
                        if response.get("type") == "error":
                            raise RuntimeError(response.get("message"))
                        completed_indexes.append(elapsed)
                        iteration += 1
            except Exception as error:
                index_errors.append(str(error))

        sampler = threading.Thread(target=sample)
        sampler.start()
        indexing = threading.Thread(target=index_load)
        indexing.start()
        before = proc_sample(daemon.pid)

        def calls(slot: int) -> dict:
            values = {"hybrid_search": [], "context_pack": []}
            for index in range(args.samples // args.clients):
                for kind in values:
                    arguments = {"path": str(corpus), "query": QUERIES[(slot + index) % len(QUERIES)], "limit": 20}
                    if kind == "context_pack":
                        arguments.update(output="context_pack", budget_tokens=2000)
                    started = time.perf_counter()
                    clients[slot].call("ig_search", arguments)
                    values[kind].append((time.perf_counter() - started) * 1000)
            return values

        with ThreadPoolExecutor(max_workers=args.clients) as pool:
            results = list(pool.map(calls, range(args.clients)))
        after = proc_sample(daemon.pid)
        stop.set()
        indexing.join(timeout=180)
        if indexing.is_alive():
            raise TimeoutError("background index did not finish")
        if index_errors:
            raise RuntimeError(f"background index failed: {index_errors}")
        sampler.join(timeout=5)
        for client in clients:
            if client.close() != 0:
                raise RuntimeError("MCP client did not exit cleanly")
        clients.clear()
        idle = []
        for _ in range(max(1, int(args.idle_seconds * 10))):
            time.sleep(0.1)
            idle.append(proc_sample(daemon.pid)["rss_bytes"])
        return {"profile": profile, "phases": phases,
                "forced_neural": latency_summary(forced),
                "concurrent_mcp": {kind: latency_summary([value for result in results for value in result[kind]]) for kind in results[0]},
                "load_cpu_ms": after["cpu_ms"] - before["cpu_ms"],
                "load_filesystem_write_bytes": after["write_bytes"] - before["write_bytes"],
                "load_peak_rss_bytes": max(sample["rss_bytes"] for sample in samples),
                "idle_rss_bytes": statistics.median(idle), "idle_rss_samples": idle,
                "index_size_bytes": directory_size(home / "indexes"),
                "background_indexes": latency_summary(completed_indexes),
                "diagnostics": diagnostics(log_path) if args.diagnostics else None}
    finally:
        stop.set()
        stop_daemon(daemon, log)
        for thread in (indexing, sampler):
            if thread:
                thread.join(timeout=5)
        for client in clients:
            client.close(how="sigkill")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--profiles", default=",".join(PROFILES))
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--samples", type=int, default=128)
    parser.add_argument("--clients", type=int, default=8)
    parser.add_argument("--idle-seconds", type=float, default=3)
    parser.add_argument("--diagnostics", action="store_true")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Linux":
        parser.error("resource measurements require Linux /proc and wait4")
    if args.runs < 1 or args.clients < 1 or args.samples < args.clients or args.idle_seconds < 0:
        parser.error("use positive runs and clients, samples >= clients, and a nonnegative idle interval")
    args.binary = args.binary.resolve()
    args.repo = args.repo.resolve()
    args.work_dir = args.work_dir.resolve()
    args.work_dir.mkdir(parents=True, exist_ok=False)
    revision = subprocess.check_output(["git", "rev-parse", args.revision], cwd=args.repo, text=True).strip()
    corpus, manifest = export_corpus(args.repo, args.work_dir, revision)
    report = {"schema_version": 1, "status": "running", "measured_at": datetime.now(timezone.utc).isoformat(),
              "binary": binary_identity(args.binary),
              "harness_sha256": {name: sha256_file(Path(__file__).with_name(name)) for name in
                                  ("bench_resource_load.py", "bench_million_chunks.py", "soak_mcp_sessions.py", "soak_daemon.py")},
              "corpus": manifest, "runtime": {"system": platform.system(), "machine": platform.machine(),
                                               "logical_cpus": os.cpu_count()},
              "method": {"runs_per_profile": args.runs, "clients": args.clients, "samples": args.samples,
                         "idle_seconds": args.idle_seconds, "diagnostics_enabled": args.diagnostics,
                         "query_cache_enabled": False, "context_budget_tokens": 2000,
                         "rss_sample_interval_ms": 20, "p99_method": "nearest rank of observed samples",
                         "limitations": ["Shared host. Timing is not a CI latency guarantee.",
                                         "Idle RSS follows a short pause. It can include allocator retention.",
                                         "Concurrent MCP calls include one background indexing loop.",
                                         "Load RSS is sampled. Phase peak RSS uses per-child wait4.",
                                         "This corpus has no relevance labels."]}, "runs": []}
    profiles = args.profiles.split(",")
    if any(profile not in PROFILES for profile in profiles):
        parser.error("profiles must be static-retrieval-v1 or potion-code-16m-v2")
    for repetition in range(args.runs):
        for profile in profiles if repetition % 2 == 0 else reversed(profiles):
            print(f"Run {repetition + 1}: {profile}", flush=True)
            work = args.work_dir / f"run-{repetition + 1}-{profile}"
            work.mkdir()
            result = benchmark_run(args.binary, profile, corpus, work, args)
            result["repetition"] = repetition + 1
            report["runs"].append(result)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    report["status"] = "complete"
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
