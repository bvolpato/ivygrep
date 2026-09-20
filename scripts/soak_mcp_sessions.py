#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Soak many `ig --mcp` sessions against one shared daemon and bound resource growth.

Phases, each optional:

* stampede: start N sessions in the same instant with no daemon running.
* lifecycle: start sessions and end them by SIGKILL, closed stdin, or closed
  stdout, some of them mid-request.
* load: N concurrent sessions issue mixed `ig_search`/`ig_status` calls against
  several workspaces while the daemon and the sessions are sampled.
* churn: create a Git worktree, search it through MCP, edit, search again, and
  delete it, many times.
* idle: CPU and wakeups of a daemon watching many workspaces that nobody calls.
* storm: dirty many workspaces at once and count concurrent background work.
  Needs `--enable-enhancement` to include hash and neural enhancement.

`--mode short` takes about two minutes and suits CI. `--mode long` runs for hours.
The default phases are stampede, lifecycle, load, and churn.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from datetime import datetime, timezone
import json
import math
import os
import platform
from pathlib import Path
import random
import select
import shutil
import signal
import statistics
import subprocess
import sys
import threading
import time
from typing import Any, Callable

from soak_daemon import copy_repo, percentile, resource_budgets, resource_gate, run, sha256_file


MIB = 1024 * 1024
PROTOCOL_VERSION = "2025-06-18"
PROBE = "src/soak_probe.rs"
SHORT_QUERIES = (
    "error handling", "workspace index status", "daemon request", "watcher debounce",
    "embedding model", "query cache", "merkle snapshot", "context pack budget",
)
LONG_QUERIES = (
    "where does the daemon decide that a watcher registration failed and how long does it wait "
    "before it retries the registration for that workspace",
    "I changed a file in a git worktree and the search still returns the old content.\n\n"
    "Find the code that reconciles the worktree overlay with the base index generation and "
    "explain which tombstones hide base chunks.",
    "how are background hash and neural enhancement jobs paused under memory pressure, battery "
    "power, or high system load, and where is the load ratio configured",
)
LITERALS = ("spawn_blocking", "IVYGREP_HOME", "fn main", "LruCache", "TODO")
REGEXES = (r"fn \w+_watcher", r"const MAX_[A-Z_]+", r"impl\s+Drop\s+for", r"Arc<Mutex<\w+>>")
SYMBOLS = ("Workspace", "resolve", "serve_stdio", "DaemonState", "index_workspace")
CALL_WEIGHTS = (("hybrid_short", 30), ("hybrid_long", 15), ("literal", 15), ("regex", 10),
                ("symbol", 10), ("context_pack", 10), ("status", 10))
# `--calls` can also select this kind: hybrid searches scoped to a directory or a file of the workspace.
SCOPES = ("src", "src/indexer", "docs", "tests", "src/daemon.rs", "README.md")
CALL_KINDS = tuple(name for name, _ in CALL_WEIGHTS) + ("scoped",)
PHASES = ("stampede", "lifecycle", "load", "churn", "idle", "storm")
MODES = {
    "short": {"stampede": 8, "lifecycle_cycles": 12, "clients": 8, "workspaces": 3, "duration": 60.0,
              "churn": 4, "sample_interval": 1.0, "settle": 5.0, "corpus": "subset", "churn_settle": 15.0,
              "gc_grace_seconds": 5, "load_warmup": 30.0, "load_settle": 0.0},
    "long": {"stampede": 32, "lifecycle_cycles": 1000, "clients": 64, "workspaces": 8, "duration": 7200.0,
             "churn": 500, "sample_interval": 10.0, "settle": 100.0, "corpus": "full", "churn_settle": 100.0,
             "gc_grace_seconds": 20, "load_warmup": 600.0, "load_settle": 120.0},
}
# Long mode settles for 100 s before it samples a baseline or an end state: on Linux with glibc an idle
# daemon returns freed memory after 60 to 90 s, so what is compared is memory still in use, not memory
# that malloc arenas happen to hold. Short mode cannot wait that long and keeps the same budgets. The
# load phase of long mode also stops calling for `load_settle` seconds after its warmup and after its
# last call, and compares the idle daemon before and after the load.


# --------------------------------------------------------------------------------------
# /proc sampling


def parse_smaps_rollup(text: str) -> dict[str, int]:
    """Rss, Pss, and Anonymous bytes of one process from `/proc/<pid>/smaps_rollup`."""
    fields = {}
    for line in text.splitlines():
        name, _, value = line.partition(":")
        parts = value.split()
        if len(parts) == 2 and parts[1] == "kB" and parts[0].isdigit():
            fields[name] = int(parts[0]) * 1024
    return {"rss_bytes": fields["Rss"], "pss_bytes": fields["Pss"], "rss_anon_bytes": fields["Anonymous"]}


def inotify_watch_count(fdinfo: str) -> int:
    """Watches held by one inotify descriptor, from `/proc/<pid>/fdinfo/<fd>`."""
    return sum(1 for line in fdinfo.splitlines() if line.startswith("inotify wd:"))


def process_sample(pid: int) -> dict[str, int]:
    """One resource sample. A missing process raises; it never reads as zero."""
    proc = Path("/proc") / str(pid)
    sample = parse_smaps_rollup((proc / "smaps_rollup").read_text())
    status = dict(line.split(":", 1) for line in (proc / "status").read_text().splitlines() if ":" in line)
    sample["threads"] = int(status["Threads"])
    fds = instances = watches = 0
    for entry in (proc / "fd").iterdir():
        fds += 1
        try:
            if os.readlink(entry) == "anon_inode:inotify":
                instances += 1
                watches += inotify_watch_count((proc / "fdinfo" / entry.name).read_text())
        except OSError:
            continue  # the descriptor closed between listing and reading
    sample.update(fds=fds, inotify_instances=instances, inotify_watches=watches)
    if min(sample["rss_bytes"], sample["threads"], sample["fds"]) <= 0:
        raise RuntimeError(f"invalid process sample: {sample}")
    return sample


def cpu_ticks(pid: int) -> int:
    """User plus system CPU ticks a process has used, from `/proc/<pid>/stat`."""
    fields = (Path("/proc") / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
    return int(fields[11]) + int(fields[12])


def context_switches(pid: int) -> dict[int, int]:
    """Voluntary context switches of every thread of a process, by thread id: how often a thread
    woke up and went back to sleep. Involuntary switches are preemptions by a busy host, not wakeups."""
    switches = {}
    for task in (Path("/proc") / str(pid) / "task").iterdir():
        try:
            switches[int(task.name)] = sum(
                int(line.split(":")[1]) for line in (task / "status").read_text().splitlines()
                if line.startswith("voluntary_ctxt_switches"))
        except OSError:
            continue  # the thread exited between listing and reading
    return switches


def wakeups(before: dict[int, int], after: dict[int, int]) -> int:
    """Context switches between two samples. Threads that exited in between drop out instead of
    subtracting their whole history, and threads that started count from zero."""
    return sum(count - before.get(thread, 0) for thread, count in after.items())


def owned_processes(home: Path) -> list[dict[str, Any]]:
    """Processes whose environment names this soak's `IVYGREP_HOME`. Nothing else is ever signalled."""
    needle = f"IVYGREP_HOME={home}".encode()
    found = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            if needle not in (entry / "environ").read_bytes().split(b"\0"):
                continue
            argv = [part.decode(errors="replace") for part in (entry / "cmdline").read_bytes().split(b"\0") if part]
            state, parent = (entry / "stat").read_text().rsplit(")", 1)[1].split()[:2]
        except OSError:
            continue
        if state != "Z":
            found.append({"pid": int(entry.name), "ppid": int(parent), "argv": argv})
    return found


def classify_processes(processes: list[dict[str, Any]]) -> dict[str, list[int]]:
    """Group processes by role. A child that a daemon or a session forked still shows its parent's
    command line until it execs `git` or an enhancement worker, so it is not a second daemon or session."""
    kinds: dict[str, list[int]] = {"daemon": [], "mcp": [], "enhancement": [], "other": []}
    roles = {process["pid"]: next((flag for flag in ("--daemon", "--mcp") if flag in process["argv"]), None)
             for process in processes}
    for process in processes:
        argv = process["argv"]
        if roles[process["pid"]] is not None and roles.get(process.get("ppid")) == roles[process["pid"]]:
            continue
        if "--daemon" in argv:
            kinds["daemon"].append(process["pid"])
        elif "--mcp" in argv:
            kinds["mcp"].append(process["pid"])
        elif any(arg.startswith("--enhance") for arg in argv):
            kinds["enhancement"].append(process["pid"])
        elif argv and Path(argv[0]).name == "ig":
            kinds["other"].append(process["pid"])
    return kinds


def index_store_stats(home: Path, *, sizes: bool) -> dict[str, int]:
    """Index directories under the home, how many name a root that no longer exists, and bytes."""
    indexes = home / "indexes"
    stats = {"index_dirs": 0, "orphan_index_dirs": 0, "index_bytes": 0}
    if not indexes.is_dir():
        return stats
    for entry in indexes.iterdir():
        if not entry.is_dir():
            continue
        stats["index_dirs"] += 1
        try:
            root = Path(json.loads((entry / "workspace.json").read_text())["root"])
            stats["orphan_index_dirs"] += 0 if root.is_dir() else 1
        except (OSError, ValueError, KeyError):
            pass
        if sizes:
            for directory, _, files in os.walk(entry):
                for name in files:
                    try:
                        stats["index_bytes"] += os.lstat(os.path.join(directory, name)).st_blocks * 512
                    except OSError:
                        continue
    return stats


def enhancement_progress(home: Path) -> dict[str, dict[str, Any]]:
    """Per workspace root: whether hash and neural vectors cover the current index generation, and
    whether its worker waits for one of the `IVYGREP_ENHANCE_MAX_WORKERS` places."""
    progress: dict[str, dict[str, Any]] = {}
    for entry in (home / "indexes").glob("*/workspace.json"):
        try:
            metadata = json.loads(entry.read_text())
        except (OSError, ValueError):
            continue

        def generation(name: str) -> int | None:
            try:
                return int((entry.parent / name).read_text().strip())
            except (OSError, ValueError):
                return None

        def text(name: str) -> str:
            try:
                return (entry.parent / name).read_text().strip()
            except OSError:
                return ""

        current = metadata.get("index_generation", 0)
        progress[str(metadata.get("root"))] = {
            "hash": generation(".hash_enhanced_generation") == current,
            "neural": generation(".neural_enhanced_generation") == current,
            "queued": text(".enhancing.phase") == "queued",
        }
    return progress


# --------------------------------------------------------------------------------------
# Statistics and gates


def linear_slope(points: list[tuple[float, float]]) -> dict[str, float]:
    """Least-squares slope of `(x, y)` with a 95% interval from the slope's standard error.

    Samples of one process are autocorrelated, so the interval is a lower bound
    on the real uncertainty; it still separates a steady climb from noise.
    """
    count = len(points)
    if count < 3:
        raise ValueError("at least 3 points are required for a slope")
    mean_x = sum(x for x, _ in points) / count
    mean_y = sum(y for _, y in points) / count
    spread = sum((x - mean_x) ** 2 for x, _ in points)
    if spread == 0:
        raise ValueError("slope needs samples at different times")
    slope = sum((x - mean_x) * (y - mean_y) for x, y in points) / spread
    intercept = mean_y - slope * mean_x
    residual = sum((y - intercept - slope * x) ** 2 for x, y in points)
    error = math.sqrt(residual / (count - 2) / spread)
    return {"slope": slope, "ci95": 1.96 * error, "samples": count}


def growth_per_hour(samples: list[dict[str, Any]], resource: str, *, warmup_fraction: float = 0.2) -> dict[str, float]:
    """Slope of one resource per hour after discarding the warmup share of the samples."""
    steady = samples[math.ceil(len(samples) * warmup_fraction):]
    fit = linear_slope([(sample["elapsed_seconds"] / 3600, float(sample[resource])) for sample in steady])
    return {"per_hour": fit["slope"], "ci95_per_hour": fit["ci95"], "samples": fit["samples"],
            "hours": (steady[-1]["elapsed_seconds"] - steady[0]["elapsed_seconds"]) / 3600}


def session_budgets(*, rss_growth_mib: float, fd_growth: int, thread_growth: int) -> dict[str, int]:
    """Growth budgets for the largest single MCP session."""
    return {"rss_anon_bytes": int(rss_growth_mib * MIB), "fds": fd_growth, "threads": thread_growth}


def summarize_sessions(samples: list[dict[str, int]]) -> dict[str, int]:
    """Collapse per-session samples: totals for the machine, maxima for the per-session gate."""
    if not samples:
        raise RuntimeError("no MCP session could be sampled")
    summary = {"sessions": len(samples)}
    for resource in ("rss_bytes", "pss_bytes", "rss_anon_bytes", "fds", "threads"):
        values = [sample[resource] for sample in samples]
        summary[f"total_{resource}"] = sum(values)
        summary[resource] = max(values)
    return summary


def latency_drift(latencies: list[tuple[float, float]]) -> dict[str, float] | None:
    """p50/p95 of the first and last quarter of `(elapsed_seconds, milliseconds)` observations."""
    if len(latencies) < 40:
        return None
    ordered = sorted(latencies)
    width = len(ordered) // 4
    first = [value for _, value in ordered[:width]]
    last = [value for _, value in ordered[-width:]]
    return {"count": len(ordered), "first_p50_ms": statistics.median(first), "last_p50_ms": statistics.median(last),
            "first_p95_ms": percentile(first, 95), "last_p95_ms": percentile(last, 95),
            "p50_ratio": statistics.median(last) / max(statistics.median(first), 1e-9)}


def churn_gate(baseline: dict[str, int], settled: dict[str, int], budgets: dict[str, int]) -> dict[str, Any]:
    """After every churned worktree is gone, nothing it needed may remain."""
    metrics = {}
    for resource, budget in budgets.items():
        growth = settled[resource] - baseline[resource]
        metrics[resource] = {"baseline": baseline[resource], "settled": settled[resource], "growth": growth,
                             "budget": budget, "passed": growth <= budget}
    return {"passed": all(metric["passed"] for metric in metrics.values()), "metrics": metrics}


def without_memory_gates(gate: dict[str, Any]) -> dict[str, Any]:
    """Report the memory metrics of a gate without failing on them.

    The memory gate is anonymous RSS under load with two malloc arenas. With glibc's default arenas RSS
    under load includes freed memory that about a hundred per-thread arenas keep, and it creeps for
    hours without a leak; with `--settle-every` it is a sawtooth. The idle daemon's memory after the
    idle trim (`settled_gate`) still grew 46 MiB in an hour of 64 sessions with default arenas, where
    the same number of calls with two arenas moved RSS under load by 24 MiB at most: pages that
    `malloc_trim` cannot return because something on them is in use. Both are reported, neither gates.
    """
    metrics = {name: {**metric, "gating": name not in ("rss_anon_bytes", "rss_bytes")}
               for name, metric in gate["metrics"].items()}
    return {**gate, "metrics": metrics,
            "passed": all(metric["passed"] for metric in metrics.values() if metric["gating"])}


def lifecycle_gate(processes: dict[str, list[int]], *, expect_daemon: bool) -> dict[str, Any]:
    """Exactly one daemon (or none), and no MCP session left behind."""
    daemons = len(processes["daemon"])
    passed = not processes["mcp"] and daemons == (1 if expect_daemon else 0)
    return {"passed": passed, "daemons": processes["daemon"], "orphan_mcp": processes["mcp"]}


# --------------------------------------------------------------------------------------
# MCP client


class McpError(RuntimeError):
    pass


class McpClient:
    """One `ig --mcp` child speaking newline-delimited JSON-RPC, one request at a time."""

    def __init__(self, binary: Path, env: dict[str, str], cwd: Path, *, stderr: Any = subprocess.DEVNULL) -> None:
        self.proc = subprocess.Popen([str(binary), "--mcp"], cwd=cwd, env=env, stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=stderr)
        self.pid = self.proc.pid
        self._buffer = b""
        self._next_id = 0

    def send(self, method: str, params: dict[str, Any] | None = None, *, notification: bool = False) -> int:
        self._next_id += 1
        message: dict[str, Any] = {"jsonrpc": "2.0", "method": method, "params": params or {}}
        if not notification:
            message["id"] = self._next_id
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(message).encode() + b"\n")
        self.proc.stdin.flush()
        return self._next_id

    def receive(self, request_id: int, timeout: float) -> dict[str, Any]:
        assert self.proc.stdout is not None
        deadline = time.monotonic() + timeout
        descriptor = self.proc.stdout.fileno()
        while True:
            while b"\n" in self._buffer:
                line, self._buffer = self._buffer.split(b"\n", 1)
                if line.strip():
                    message = json.loads(line)
                    if message.get("id") == request_id:
                        return message
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([descriptor], [], [], remaining)[0]:
                raise McpError(f"no MCP response within {timeout}s (pid {self.pid})")
            chunk = os.read(descriptor, 1 << 16)
            if not chunk:
                raise McpError(f"MCP session {self.pid} closed stdout")
            self._buffer += chunk

    def request(self, method: str, params: dict[str, Any] | None = None, *, timeout: float = 180.0) -> dict[str, Any]:
        return self.receive(self.send(method, params), timeout)

    def initialize(self) -> dict[str, Any]:
        response = self.request("initialize", {"protocolVersion": PROTOCOL_VERSION, "capabilities": {},
                                               "clientInfo": {"name": "soak_mcp_sessions", "version": "1"}})
        if "result" not in response:
            raise McpError(f"initialize failed: {response}")
        self.send("notifications/initialized", notification=True)
        return response

    def call(self, tool: str, arguments: dict[str, Any], *, timeout: float = 180.0) -> dict[str, Any]:
        return tool_payload(self.request("tools/call", {"name": tool, "arguments": arguments}, timeout=timeout))

    def close(self, *, how: str = "stdin", wait: float = 15.0) -> int | None:
        """End the session the way a client could: closed stdin, closed stdout, or SIGKILL.

        With `stdout`, stdin stays open until the session has exited: the
        session must end because a reply cannot be written, not because of EOF.
        """
        exit_code = self.proc.poll()
        if exit_code is None:
            if how == "sigkill":
                self.proc.kill()
            elif how == "stdout":
                assert self.proc.stdout is not None
                self.proc.stdout.close()
                try:
                    self.send("ping")  # guarantees a reply the session cannot write
                except OSError:
                    pass  # it already noticed
                try:
                    exit_code = self.proc.wait(timeout=wait)
                except subprocess.TimeoutExpired:
                    return None
        for stream in (self.proc.stdin, self.proc.stdout):
            try:
                if stream is not None and not stream.closed:
                    stream.close()
            except OSError:
                pass
        try:
            return self.proc.wait(timeout=wait) if exit_code is None else exit_code
        except subprocess.TimeoutExpired:
            return None


def tool_payload(response: dict[str, Any]) -> dict[str, Any]:
    """Structured payload of a `tools/call` response; protocol and tool errors raise."""
    if "error" in response:
        raise McpError(f"JSON-RPC error: {response['error']}")
    result = response["result"]
    if result.get("isError"):
        raise McpError(f"tool error: {result['content'][0]['text'][:400]}")
    return result["structuredContent"]


def wait_until_searchable(client: McpClient, workspace: Path, *, timeout: float = 600.0) -> None:
    """First searches answer `status: indexing` until the daemon has built the index."""
    deadline = time.monotonic() + timeout
    while True:
        payload = client.call("ig_search", {"query": "fn main", "path": str(workspace), "literal": True})
        if payload.get("status") != "indexing":
            return
        if time.monotonic() >= deadline:
            raise McpError(f"{workspace} still indexing after {timeout}s")
        time.sleep(min(2.0, float(payload.get("retry_after_secs", 2))))


# --------------------------------------------------------------------------------------
# Workload


def pick_call(rng: random.Random, workspaces: list[Path], home_workspace: Path, sequence: int,
              weights: tuple[tuple[str, int], ...] = CALL_WEIGHTS) -> tuple[str, str, dict[str, Any]]:
    """One realistic call: usually the session's own workspace, sometimes another one.

    Every third query carries a counter so the daemon's 128-entry query cache
    keeps evicting instead of serving one warm set forever.
    """
    kind = rng.choices([name for name, _ in weights], [weight for _, weight in weights])[0]
    workspace = home_workspace if rng.random() < 0.9 else rng.choice(workspaces)
    suffix = f" {sequence}" if sequence % 3 == 0 else ""
    arguments: dict[str, Any] = {"path": str(workspace)}
    if kind == "status":
        return kind, "ig_status", {}
    if kind == "scoped":
        scope = workspace / rng.choice(SCOPES)
        arguments["path"] = str(scope if scope.exists() else workspace)
        arguments["query"] = rng.choice(SHORT_QUERIES) + suffix
    elif kind == "hybrid_short":
        arguments["query"] = rng.choice(SHORT_QUERIES) + suffix
    elif kind == "hybrid_long":
        arguments["query"] = rng.choice(LONG_QUERIES) + suffix
    elif kind == "literal":
        arguments.update(query=rng.choice(LITERALS), literal=True)
    elif kind == "regex":
        arguments.update(query=rng.choice(REGEXES), regex=True)
    elif kind == "symbol":
        arguments.update(query=rng.choice(SYMBOLS), symbol=True)
    else:
        arguments.update(query=rng.choice(SHORT_QUERIES + LONG_QUERIES) + suffix, output="context_pack",
                         budget_tokens=rng.choice((2000, 8000)))
    return kind, "ig_search", arguments


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-c", "user.name=Soak Benchmark", "-c", "user.email=soak@example.invalid",
                           "-c", "commit.gpgSign=false", "-c", "core.hooksPath=/dev/null", *args],
                          cwd=repo, check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=300).stdout


def prepare_workspaces(source: Path, root: Path, count: int, corpus: str, *, full_limit: int | None = None) -> list[Path]:
    """Independent Git repositories. `subset` keeps indexing short; `full` copies the corpus.

    Past `full_limit` every workspace is a subset, so phases that only need
    many watched roots stay cheap.
    """
    workspaces = []
    for index in range(count):
        workspace = root / f"workspace-{index:02d}"
        if full_limit is not None and index >= full_limit:
            corpus = "subset"
        if corpus == "full" and index % 2 == 0:
            copy_repo(source, workspace)
        else:
            workspace.mkdir(parents=True)
            for name in ("src", "docs") if corpus == "full" else ("src",):
                if (source / name).is_dir():
                    shutil.copytree(source / name, workspace / name, ignore=shutil.ignore_patterns("__pycache__"))
            (workspace / "README.md").write_text(f"# soak workspace {index}\n")
            for args in (["init", "-q"], ["add", "."], ["commit", "-qm", "benchmark corpus"]):
                git(workspace, *args)
        workspaces.append(workspace)
    return workspaces


class LoadStats:
    """Call counts, failures, and bounded latency samples shared by the session threads."""

    def __init__(self) -> None:
        self.calls: dict[str, int] = defaultdict(int)
        self.errors: list[str] = []
        self.error_count = 0
        self.latencies: dict[str, list[tuple[float, float]]] = defaultdict(list)
        self.lock = threading.Lock()

    def record(self, kind: str, elapsed: float, milliseconds: float) -> None:
        with self.lock:
            self.calls[kind] += 1
            bucket = self.latencies[kind]
            if len(bucket) < 200_000:
                bucket.append((elapsed, milliseconds))

    def fail(self, message: str) -> None:
        with self.lock:
            self.error_count += 1
            if len(self.errors) < 10:
                self.errors.append(message[:500])


class Soak:
    def __init__(self, args: argparse.Namespace, env: dict[str, str], home: Path, work: Path) -> None:
        self.args, self.env, self.home, self.work = args, env, home, work
        self.binary: Path = args.binary
        self.report: dict[str, Any] = {}

    def client(self, cwd: Path) -> McpClient:
        return McpClient(self.binary, self.env, cwd)

    def daemon_pid(self) -> int:
        daemons = classify_processes(owned_processes(self.home))["daemon"]
        if len(daemons) != 1:
            raise RuntimeError(f"expected exactly one daemon for {self.home}, found {daemons}")
        return daemons[0]

    def daemon_sample(self, *, sizes: bool = False) -> dict[str, int]:
        return {**process_sample(self.daemon_pid()), **index_store_stats(self.home, sizes=sizes)}

    # -- stampede ----------------------------------------------------------------------

    def stampede(self, workspace: Path, sessions: int) -> dict[str, Any]:
        """Start every session at once with no daemon; all must answer through one daemon."""
        if classify_processes(owned_processes(self.home))["daemon"]:
            raise RuntimeError("stampede needs a home without a running daemon")
        barrier = threading.Barrier(sessions)
        clients: list[McpClient | None] = [None] * sessions
        outcomes: list[str | None] = [None] * sessions
        peak_daemons = [0]
        done = threading.Event()

        def watch_daemons() -> None:
            while not done.is_set():
                peak_daemons[0] = max(peak_daemons[0], len(classify_processes(owned_processes(self.home))["daemon"]))
                time.sleep(0.05)

        def session(slot: int) -> None:
            try:
                barrier.wait(timeout=60)
                client = clients[slot] = self.client(workspace)
                client.initialize()
                wait_until_searchable(client, workspace)
                payload = client.call("ig_search", {"query": "workspace index status", "path": str(workspace)})
                outcomes[slot] = "ok" if payload.get("result_count", 0) > 0 else f"no hits: {payload}"
            except Exception as error:  # noqa: BLE001 - every failure is reported
                outcomes[slot] = f"{type(error).__name__}: {error}"

        watcher = threading.Thread(target=watch_daemons, daemon=True)
        watcher.start()
        started = time.monotonic()
        threads = [threading.Thread(target=session, args=(slot,)) for slot in range(sessions)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        elapsed = time.monotonic() - started
        time.sleep(3)  # daemons that lost the single-instance lock exit within ~2 s
        done.set()
        watcher.join()
        processes = classify_processes(owned_processes(self.home))
        recorded = (self.home / "daemon.pid").read_text().strip() if (self.home / "daemon.pid").exists() else None
        # A session that found no daemon in time searched or indexed in-process and keeps that memory.
        anon = [process_sample(client.pid)["rss_anon_bytes"] for client in clients
                if client is not None and client.proc.poll() is None]
        for client in clients:
            if client is not None:
                client.close()
        failures = [outcome for outcome in outcomes if outcome != "ok"]
        result = {"sessions": sessions, "answered": sessions - len(failures), "failures": failures[:5],
                  "seconds": elapsed, "peak_daemon_processes": peak_daemons[0], "daemons_after": processes["daemon"],
                  "recorded_daemon_pid": recorded, "largest_session_rss_anon_bytes": max(anon, default=0),
                  "sessions_that_worked_in_process": sum(1 for value in anon if value > 16 * MIB),
                  "passed": not failures and len(processes["daemon"]) == 1 and recorded == str(processes["daemon"][0])}
        after = lifecycle_gate(classify_processes(owned_processes(self.home)), expect_daemon=True)
        result["orphan_mcp_after_close"] = after["orphan_mcp"]
        result["passed"] = result["passed"] and after["passed"]
        return result

    # -- lifecycle ---------------------------------------------------------------------

    def lifecycle(self, workspace: Path, cycles: int) -> dict[str, Any]:
        """Start and end sessions every way a client can, some of them mid-request."""
        endings: dict[str, int] = defaultdict(int)
        slow_exits: list[str] = []
        rng = random.Random(self.args.seed)
        # A context pack takes long enough that the session usually ends while the daemon builds it.
        long_query = {"query": LONG_QUERIES[1], "path": str(workspace), "output": "context_pack"}
        # The same cycles first warm the daemon's lazy pools, so the baseline is steady state.
        warmup = max(6, cycles // 10)
        baseline: dict[str, int] = {}
        for cycle in range(-warmup, cycles):
            if cycle == 0:
                time.sleep(self.args.settle)
                baseline = self.daemon_sample()
            how = ("sigkill", "stdin", "stdout")[cycle % 3]
            mid_request = cycle % 2 == 0
            client = self.client(workspace)
            client.initialize()
            if mid_request:
                client.send("tools/call", {"name": "ig_search", "arguments": {
                    **long_query, "query": f"{long_query['query']} {rng.random()}"}})
                time.sleep(rng.uniform(0.0, 0.05))
            else:
                client.call("ig_search", {"query": rng.choice(SHORT_QUERIES), "path": str(workspace)})
            if client.close(how=how, wait=30.0) is None:
                slow_exits.append(f"cycle {cycle}: {how} mid_request={mid_request} pid={client.pid}")
                client.proc.kill()
                client.proc.wait(timeout=10)
            endings[f"{how}{'_mid_request' if mid_request else ''}"] += 1
        time.sleep(self.args.settle)
        settled = self.daemon_sample()
        gate = churn_gate(baseline, settled, {"rss_anon_bytes": int(self.args.rss_growth_mib * MIB),
                                              "fds": self.args.fd_growth, "threads": self.args.thread_growth})
        processes = lifecycle_gate(classify_processes(owned_processes(self.home)), expect_daemon=True)
        return {"cycles": cycles, "endings": dict(endings), "sessions_that_outlived_their_client": slow_exits[:10],
                "daemon_baseline": baseline, "daemon_settled": settled, "daemon_gate": gate, "processes": processes,
                "passed": not slow_exits and gate["passed"] and processes["passed"]}

    # -- load --------------------------------------------------------------------------

    def load(self, workspaces: list[Path], clients: int, duration: float) -> dict[str, Any]:
        stats = LoadStats()
        stop = threading.Event()
        sessions = [self.client(workspaces[slot % len(workspaces)]) for slot in range(clients)]
        for session in sessions:
            session.initialize()
        # The same load first warms the daemon unsampled: thread pools, search contexts, and the preview
        # cache fill for a while, and that growth is not what the budgets are about.
        started = time.monotonic() + self.args.load_warmup + self.args.load_settle
        pause = threading.Event()

        def worker(slot: int) -> None:
            rng = random.Random(self.args.seed * 1000 + slot)
            session, home_workspace, sequence = sessions[slot], workspaces[slot % len(workspaces)], 0
            while not stop.is_set():
                if pause.is_set():
                    stop.wait(0.2)
                    continue
                sequence += 1
                kind, tool, arguments = pick_call(rng, workspaces, home_workspace, sequence * clients + slot,
                                                  self.args.call_weights)
                before = time.perf_counter()
                try:
                    session.call(tool, arguments)
                    if time.monotonic() >= started:
                        stats.record(kind, time.monotonic() - started, (time.perf_counter() - before) * 1000)
                except Exception as error:  # noqa: BLE001 - counted and reported
                    stats.fail(f"{kind}: {type(error).__name__}: {error}")
                    if session.proc.poll() is not None:
                        return
                stop.wait(self.args.think_time)

        threads = [threading.Thread(target=worker, args=(slot,), daemon=True) for slot in range(clients)]
        for thread in threads:
            thread.start()
        samples, session_samples = [], []
        settled: list[dict[str, Any]] = []
        stop.wait(self.args.load_warmup)
        if self.args.load_settle:
            # Nobody calls for a while: an idle daemon returns freed memory (Linux with glibc), and what
            # it still holds is memory in use. The same is sampled after the load, and with
            # `--settle-every` at intervals during it.
            pause.set()
            stop.wait(max(0.0, started - time.monotonic()))
            settled.append({"elapsed_seconds": 0.0, "unix_time": time.time(), **self.daemon_sample()})
            pause.clear()
        daemon_pid = self.daemon_pid()
        next_settle = started + self.args.settle_every if self.args.load_settle and self.args.settle_every else None
        try:
            while (elapsed := time.monotonic() - started) < duration:
                if self.daemon_pid() != daemon_pid:
                    raise RuntimeError("the daemon was replaced during the load phase")
                if next_settle is not None and time.monotonic() >= next_settle:
                    pause.set()
                    time.sleep(self.args.load_settle)
                    settled.append({"elapsed_seconds": time.monotonic() - started, "unix_time": time.time(),
                                    **self.daemon_sample()})
                    print(f"settled sample {len(settled)}: {settled[-1]}", flush=True)
                    pause.clear()
                    next_settle = time.monotonic() + self.args.settle_every
                    continue
                samples.append({"elapsed_seconds": elapsed, **self.daemon_sample()})
                session_samples.append({"elapsed_seconds": elapsed, **summarize_sessions(
                    [process_sample(session.pid) for session in sessions if session.proc.poll() is None])})
                if stats.error_count > self.args.max_errors:
                    raise RuntimeError(f"too many failed calls: {stats.errors}")
                time.sleep(self.args.sample_interval)
        finally:
            stop.set()
            for thread in threads:
                thread.join(timeout=240)
            if settled and sys.exc_info()[0] is None:
                time.sleep(self.args.load_settle)
                settled.append({"elapsed_seconds": time.monotonic() - started, "unix_time": time.time(),
                                **self.daemon_sample()})
            exits = [session.close() for session in sessions]
        settled_gate = None
        if len(settled) >= 2:
            settled_gate = without_memory_gates(churn_gate(settled[0], settled[-1], {
                "rss_anon_bytes": int(self.args.rss_growth_mib * MIB), "fds": self.args.fd_growth,
                "threads": self.args.thread_growth}))
            settled_gate["samples"] = settled
            if len(settled) >= 4:
                # The first interval still fills caches; the slope is over the samples after it.
                fit = linear_slope([(sample["elapsed_seconds"] / 3600, sample["rss_anon_bytes"] / MIB)
                                    for sample in settled[1:]])
                settled_gate["rss_anon_mib_per_hour"] = fit
        budgets = resource_budgets(rss_growth_mib=self.args.rss_growth_mib,
                                   total_rss_growth_mib=self.args.total_rss_growth_mib,
                                   fd_growth=self.args.fd_growth + clients,
                                   # The blocking pool grows and shrinks with the requests in flight.
                                   thread_growth=self.args.thread_growth + min(clients, 16))
        budgets["inotify_watches"] = self.args.inotify_watch_growth
        session_limits = session_budgets(rss_growth_mib=self.args.session_rss_growth_mib, fd_growth=4, thread_growth=2)
        daemon_gate, session_gate = resource_gate(samples, budgets), resource_gate(session_samples, session_limits)
        if self.args.malloc_arenas == "default" or (settled_gate and self.args.settle_every):
            daemon_gate = without_memory_gates(daemon_gate)
        slopes = {"daemon_rss_anon_mib_per_hour": scale(growth_per_hour(samples, "rss_anon_bytes"), MIB),
                  "daemon_fds_per_hour": growth_per_hour(samples, "fds"),
                  "daemon_threads_per_hour": growth_per_hour(samples, "threads"),
                  "largest_session_rss_anon_mib_per_hour": scale(growth_per_hour(session_samples, "rss_anon_bytes"), MIB)}
        total_calls = sum(stats.calls.values())
        if total_calls:
            hourly = slopes["daemon_rss_anon_mib_per_hour"]
            calls_per_hour = total_calls / (duration / 3600)
            slopes["daemon_rss_anon_mib_per_million_calls"] = {
                "per_million_calls": hourly["per_hour"] / calls_per_hour * 1e6,
                "ci95_per_million_calls": hourly["ci95_per_hour"] / calls_per_hour * 1e6}
        return {"clients": clients, "workspaces": len(workspaces), "duration_seconds": duration, "daemon_pid": daemon_pid,
                "calls": dict(stats.calls), "total_calls": total_calls, "failed_calls": stats.error_count,
                "errors": stats.errors, "latency": {kind: latency_drift(values) for kind, values in stats.latencies.items()},
                "daemon_gate": daemon_gate, "session_gate": session_gate, "settled_gate": settled_gate,
                "slopes": slopes,
                "session_exit_codes": sorted({-1 if code is None else code for code in exits}),
                "last_sessions": session_samples[-1], "last_daemon": samples[-1],
                "samples": samples, "session_samples": session_samples,
                "passed": bool(total_calls) and stats.error_count <= self.args.max_errors and daemon_gate["passed"]
                and session_gate["passed"] and (settled_gate is None or settled_gate["passed"])
                and None not in exits}

    # -- worktree churn ----------------------------------------------------------------

    def churn(self, base: Path, count: int, checkpoints: list[int]) -> dict[str, Any]:
        """Create, search, edit, search, and delete agent worktrees under `<repo>/.claude/worktrees`.

        The first worktrees are not counted: they warm the daemon's overlay
        indexing and fill its per-workspace LRU caches, the largest of which
        hold 256 entries, so a baseline taken earlier reads bounded cache fill
        as growth. While a worktree exists, the base must keep returning each
        file once: a nested worktree is a workspace of its own.
        """
        session = self.client(base)
        session.initialize()
        wait_until_searchable(session, base)
        warmup = max(2, min(256, count // 2))
        baseline: dict[str, int] = {}
        records: list[dict[str, Any]] = []
        stale: list[str] = []
        duplicates: list[str] = []
        started = time.monotonic()
        for number in range(1 - warmup, count + 1):
            if number == 1:
                time.sleep(self.args.churn_settle)
                baseline = self.daemon_sample(sizes=True)
                records.append({"worktrees": 0, **baseline})
                started = time.monotonic()
            name = f"agent-{number + warmup:05d}"
            worktree = base / ".claude" / "worktrees" / name
            git(base, "worktree", "add", "-q", "-b", f"soak-{name}", str(worktree))
            wait_until_searchable(session, worktree)
            marker = f"soak_worktree_marker_{number + warmup}"
            (worktree / PROBE).write_text(f"pub fn {marker}() -> u64 {{ 1 }}\n")
            if not self.edit_visible(session, worktree, marker):
                stale.append(f"worktree {name} never returned its edit")
            base_hits = session.call("ig_search", {"query": "fn main", "path": str(base), "literal": True, "limit": 50})
            nested = [hit["file_path"] for hit in base_hits.get("results", []) if ".claude/worktrees" in hit["file_path"]]
            if nested and len(duplicates) < 10:
                duplicates.append(f"base search returned {nested[0]}")
            git(base, "worktree", "remove", "--force", str(worktree))
            git(base, "branch", "-q", "-D", f"soak-{name}")
            if number in checkpoints:
                records.append({"worktrees": number, "elapsed_seconds": time.monotonic() - started,
                                **self.daemon_sample(sizes=True)})
                print(f"churn checkpoint {number}: {records[-1]}", flush=True)
        session.close()
        time.sleep(self.args.churn_settle)
        settled = self.daemon_sample(sizes=True)
        gate = churn_gate(baseline, settled, {
            "rss_anon_bytes": int(self.args.rss_growth_mib * MIB), "fds": self.args.fd_growth,
            "threads": self.args.thread_growth, "inotify_watches": self.args.inotify_watch_growth,
            "inotify_instances": 0, "index_dirs": self.args.index_dir_growth,
            "orphan_index_dirs": self.args.index_dir_growth})
        return {"worktrees": count, "warmup_worktrees": warmup, "seconds": time.monotonic() - started,
                "checkpoints": records, "settled": settled, "stale_edits": stale[:10],
                "base_results_from_nested_worktrees": duplicates, "gate": gate,
                "passed": gate["passed"] and not stale and not duplicates}

    # -- idle cost ---------------------------------------------------------------------

    def idle(self, workspaces: list[Path], seconds: float) -> dict[str, Any]:
        """CPU and wakeups of a daemon that watches every workspace while nobody calls it."""
        opener = self.client(workspaces[0])
        opener.initialize()
        for workspace in workspaces:
            wait_until_searchable(opener, workspace)
        opener.close()
        time.sleep(self.args.settle)
        pid = self.daemon_pid()
        ticks, switches, before = cpu_ticks(pid), context_switches(pid), time.monotonic()
        time.sleep(seconds)
        elapsed = time.monotonic() - before
        cpu_percent = (cpu_ticks(pid) - ticks) / os.sysconf("SC_CLK_TCK") / elapsed * 100
        switches_per_second = wakeups(switches, context_switches(pid)) / elapsed
        sample = self.daemon_sample(sizes=True)
        return {"watched_workspaces": len(workspaces), "seconds": elapsed, "cpu_percent_of_one_core": cpu_percent,
                "wakeups_per_second": switches_per_second, "daemon": sample,
                "cpu_budget_percent": self.args.idle_cpu_percent,
                "passed": cpu_percent <= self.args.idle_cpu_percent
                and sample["inotify_instances"] >= len(workspaces)}

    # -- background work storm ---------------------------------------------------------

    def storm(self, workspaces: list[Path], timeout: float) -> dict[str, Any]:
        """Dirty every workspace at once, as agents in many worktrees do, while one session keeps searching
        the last workspace. Once every edit is visible, another session creates, edits, and searches an agent
        worktree. Reports how much background work runs together, and when the searched workspace, the fresh
        worktree, and every workspace have current vectors."""
        session = self.client(workspaces[0])
        session.initialize()
        for workspace in workspaces:
            wait_until_searchable(session, workspace)
        pid = self.daemon_pid()
        last = len(workspaces) - 1
        roots = {str(workspace.resolve()): index for index, workspace in enumerate(workspaces)}
        peak = {"enhancement_processes": 0, "queued_enhancement_processes": 0, "running_enhancement_processes": 0,
                "paused_enhancement_processes": 0, "enhancement_rss_bytes": 0, "largest_worker_rss_bytes": 0,
                "daemon_rss_anon_bytes": 0, "daemon_threads": 0, "load1": 0.0}
        pause_reasons: set[str] = set()
        ticks_before, before = cpu_ticks(pid), time.monotonic()
        for index, workspace in enumerate(workspaces):
            for copy in range(20):
                (workspace / "src" / f"storm_{copy}.rs").write_text(
                    f"pub fn storm_marker_{index}_{copy}() -> u64 {{ {copy} }}\n" * 50)
        natural = "how does the storm marker get computed"
        pending = set(range(len(workspaces)))
        enhanced_after: dict[int, float] = {}
        times: dict[str, float | None] = {"visible": None, "searched_hash": None}
        # The base of the fresh worktree is a workspace from the middle of the queue.
        fresh = workspaces[len(workspaces) // 2] / ".claude" / "worktrees" / "agent-storm"
        fresh_times: dict[str, float | None] = {"created_at_seconds": None, "first_answer_after_seconds": None,
                                                "hash_vectors_after_seconds": None, "enhanced_after_seconds": None}
        stop, all_visible, failures = threading.Event(), threading.Event(), []

        def agents() -> None:
            # A natural-language query routes to neural retrieval, so it asks for neural vectors. Every
            # workspace asks until its edit is visible, and again every 15 s while vectors are missing.
            last_round = 0.0
            while not stop.is_set():
                ask_again = time.monotonic() - last_round >= 15
                if ask_again:
                    last_round = time.monotonic()
                for index in range(len(workspaces)):
                    if index in pending or (ask_again and index not in enhanced_after):
                        session.call("ig_search", {"query": natural, "path": str(workspaces[index])})
                    if index in pending:
                        hits = session.call("ig_search", {"query": f"storm_marker_{index}_19", "literal": True,
                                                          "path": str(workspaces[index])})
                        if hits.get("result_count", 0) > 0:
                            pending.discard(index)
                if not pending and times["visible"] is None:
                    times["visible"] = time.monotonic() - before
                    all_visible.set()
                # The session of the workspace in use searches all the time, and last in every round.
                session.call("ig_search", {"query": natural, "path": str(workspaces[last])})
                stop.wait(0.5)

        def fresh_worktree() -> None:
            while not all_visible.wait(0.5):
                if stop.is_set():
                    return
            newcomer = self.client(fresh.parents[2])
            try:
                newcomer.initialize()
                created = time.monotonic()
                fresh_times["created_at_seconds"] = created - before
                git(fresh.parents[2], "worktree", "add", "-q", "-b", "soak-agent-storm", str(fresh))
                wait_until_searchable(newcomer, fresh)
                fresh_times["first_answer_after_seconds"] = time.monotonic() - created
                for copy in range(5):
                    (fresh / "src" / f"fresh_{copy}.rs").write_text(
                        f"pub fn fresh_worktree_marker_{copy}() -> u64 {{ {copy} }}\n" * 50)
                while not stop.is_set():
                    newcomer.call("ig_search", {"query": natural, "path": str(fresh)})
                    stop.wait(0.5)
            finally:
                newcomer.close()

        def guarded(work: Any) -> None:
            try:
                work()
            except Exception as error:  # noqa: BLE001 - reported as the phase failure
                failures.append(f"{work.__name__}: {error}")
                stop.set()

        threads = [threading.Thread(target=guarded, args=(work,), daemon=True) for work in (agents, fresh_worktree)]
        for thread in threads:
            thread.start()
        while time.monotonic() - before < timeout and not stop.is_set():
            now = time.monotonic() - before
            progress = enhancement_progress(self.home)
            fresh_state = progress.get(str(fresh.resolve())) if fresh.exists() else None
            if fresh_state and fresh_times["created_at_seconds"] is not None:
                since_created = now - fresh_times["created_at_seconds"]
                if fresh_state["hash"] and fresh_times["hash_vectors_after_seconds"] is None:
                    fresh_times["hash_vectors_after_seconds"] = since_created
                if fresh_state["hash"] and fresh_state["neural"] and fresh_times["enhanced_after_seconds"] is None:
                    fresh_times["enhanced_after_seconds"] = since_created
            for root, state in progress.items():
                index = roots.get(root)
                if index is None or index in pending:
                    continue
                if index == last and state["hash"] and times["searched_hash"] is None:
                    times["searched_hash"] = now
                if state["hash"] and state["neural"]:
                    enhanced_after.setdefault(index, now)
            enhancers = classify_processes(owned_processes(self.home))["enhancement"]
            sizes = []
            for enhancer in enhancers:
                try:
                    sizes.append(process_sample(enhancer)["rss_bytes"])
                except (OSError, KeyError, RuntimeError):
                    continue  # the worker exited between listing and sampling
            daemon = process_sample(pid)
            # A worker that the memory, battery, or load guard paused says why in its index directory.
            paused = [path.read_text().strip() for path in (self.home / "indexes").glob("*/.enhancing.paused")]
            pause_reasons.update(reason.split("(")[0].strip() for reason in paused if reason)
            queued = min(len(enhancers), sum(1 for state in progress.values() if state["queued"]))
            peak["paused_enhancement_processes"] = max(peak["paused_enhancement_processes"], len(paused))
            peak["enhancement_processes"] = max(peak["enhancement_processes"], len(enhancers))
            peak["queued_enhancement_processes"] = max(peak["queued_enhancement_processes"], queued)
            peak["running_enhancement_processes"] = max(peak["running_enhancement_processes"],
                                                        len(enhancers) - queued)
            peak["enhancement_rss_bytes"] = max(peak["enhancement_rss_bytes"], sum(sizes))
            peak["largest_worker_rss_bytes"] = max(peak["largest_worker_rss_bytes"], max(sizes, default=0))
            peak["daemon_rss_anon_bytes"] = max(peak["daemon_rss_anon_bytes"], daemon["rss_anon_bytes"])
            peak["daemon_threads"] = max(peak["daemon_threads"], daemon["threads"])
            peak["load1"] = max(peak["load1"], os.getloadavg()[0])
            finished = not self.args.enable_enhancement or (
                len(enhanced_after) == len(workspaces) and fresh_times["hash_vectors_after_seconds"] is not None)
            if not pending and finished and not enhancers:
                break
            time.sleep(0.2)
        stop.set()
        for thread in threads:
            thread.join(timeout=240)
        session.close()
        leftover = classify_processes(owned_processes(self.home))["enhancement"]
        return {"dirty_workspaces": len(workspaces), "seconds": time.monotonic() - before,
                "edits_visible_after_seconds": times["visible"],
                "searched_workspace_hash_vectors_after_seconds": times["searched_hash"],
                "searched_workspace_enhanced_after_seconds": enhanced_after.get(last),
                "all_enhanced_after_seconds": max(enhanced_after.values())
                if len(enhanced_after) == len(workspaces) else None,
                "enhanced_workspaces": len(enhanced_after),
                "enhanced_after_seconds": [enhanced_after.get(index) for index in range(len(workspaces))],
                "fresh_worktree": fresh_times,
                "peak": peak,
                "daemon_cpu_seconds": (cpu_ticks(pid) - ticks_before) / os.sysconf("SC_CLK_TCK"),
                "cpu_count": os.cpu_count(), "background_enhancement": self.args.enable_enhancement,
                "pause_reasons": sorted(pause_reasons), "workspaces_never_updated": sorted(pending),
                "failures": failures,
                # Paused workers wait for the host to calm down; they are reported, not failed.
                "enhancement_processes_left": leftover, "passed": not pending and not failures}

    def edit_visible(self, session: McpClient, worktree: Path, needle: str, *, timeout: float = 60.0) -> bool:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            payload = session.call("ig_search", {"query": needle, "path": str(worktree), "literal": True})
            if payload.get("result_count", 0) > 0:
                return True
            time.sleep(0.2)
        return False


def scale(fit: dict[str, float], unit: int) -> dict[str, float]:
    return {**fit, "per_hour": fit["per_hour"] / unit, "ci95_per_hour": fit["ci95_per_hour"] / unit}


def stop_owned_processes(home: Path) -> None:
    """End what this run started: the daemon through its pid, then anything naming this home."""
    for signum in (signal.SIGTERM, signal.SIGKILL):
        for process in owned_processes(home):
            try:
                os.kill(process["pid"], signum)
            except ProcessLookupError:
                pass
        deadline = time.monotonic() + 5
        while owned_processes(home) and time.monotonic() < deadline:
            time.sleep(0.1)


def reported_environment(items: list[str]) -> list[str]:
    """`--env` settings as the report records them.

    Reports are published, and `--env` may carry a credential such as a model
    hub token. Only the settings that decide what was measured keep their
    value: ivygrep's own variables and the allocator's. Every other variable is
    recorded by name.
    """
    reported = []
    for item in items:
        key = item.partition("=")[0]
        secret = any(word in key.upper() for word in ("TOKEN", "SECRET", "PASSWORD", "CREDENTIAL", "KEY"))
        keeps_value = key.startswith(("IVYGREP_", "MALLOC_")) and not secret
        reported.append(item if keeps_value else f"{key}=<redacted>")
    return sorted(reported)


def path_layout_error(source: Path, work: Path, output: Path) -> str | None:
    """Why these resolved paths cannot be used together, or `None`.

    A work directory inside the corpus is copied into itself with every full
    workspace, and an output inside the work directory is deleted with it after
    a successful run.
    """
    if work == source or source in work.parents:
        return f"--work-dir {work} must be outside --repo {source}"
    if output == work or work in output.parents:
        return f"--output {output} must be outside --work-dir {work}, which is removed after a successful run"
    return None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path.cwd(), help="corpus copied into the soak workspaces")
    parser.add_argument("--mode", choices=sorted(MODES), default="short")
    parser.add_argument("--work-dir", type=Path, required=True,
                        help="empty scratch directory for the isolated IVYGREP_HOME and workspaces")
    parser.add_argument("--phases", default="stampede,lifecycle,load,churn",
                        help=f"comma-separated, in order, from: {', '.join(PHASES)}")
    parser.add_argument("--stampede", type=int, help="sessions started at once with no daemon")
    parser.add_argument("--lifecycle-cycles", type=int)
    parser.add_argument("--clients", type=int)
    parser.add_argument("--workspaces", type=int)
    parser.add_argument("--duration", type=float, help="seconds of sampled load")
    parser.add_argument("--load-warmup", type=float, help="seconds of the same load before sampling starts")
    parser.add_argument("--calls", default="",
                        help="comma-separated call kinds for the load phase instead of the default mix, to see "
                             f"which kind of request moves a resource: {', '.join(CALL_KINDS)}")
    parser.add_argument("--settle-every", type=float, default=0.0,
                        help="also stop calling for --load-settle seconds at this interval during the load and "
                             "sample the idle daemon, for a series of memory in use over time")
    parser.add_argument("--load-settle", type=float,
                        help="seconds without calls after the warmup and after the load, before the idle daemon "
                             "is sampled for the settled memory gate; 0 skips that gate")
    parser.add_argument("--churn", type=int, help="worktrees to create and delete")
    parser.add_argument("--churn-checkpoints", default="10,100,500")
    parser.add_argument("--churn-settle", type=float,
                        help="wait before the churn baseline and after the last deletion; must cover the "
                             "collection grace period plus one pass")
    parser.add_argument("--gc-grace-seconds", type=int,
                        help="IVYGREP_INDEX_GC_GRACE_SECS for the daemon, so the churn gate sees deleted "
                             "worktrees collected; 0 disables collection")
    parser.add_argument("--idle-workspaces", type=int, default=40)
    parser.add_argument("--idle-seconds", type=float, default=120.0)
    parser.add_argument("--idle-cpu-percent", type=float, default=5.0,
                        help="idle daemon CPU budget, in percent of one core")
    parser.add_argument("--storm-workspaces", type=int, default=20)
    parser.add_argument("--storm-timeout", type=float, default=900.0)
    parser.add_argument("--corpus", choices=("subset", "full"))
    parser.add_argument("--sample-interval", type=float)
    parser.add_argument("--settle", type=float)
    parser.add_argument("--think-time", type=float, default=0.02)
    parser.add_argument("--seed", type=int, default=20260919)
    parser.add_argument("--max-errors", type=int, default=0)
    parser.add_argument("--rss-growth-mib", type=float, default=32.0, help="daemon anonymous RSS growth budget")
    parser.add_argument("--total-rss-growth-mib", type=float, default=96.0)
    parser.add_argument("--session-rss-growth-mib", type=float, default=16.0,
                        help="anonymous RSS growth budget of the largest MCP session")
    parser.add_argument("--fd-growth", type=int, default=8)
    parser.add_argument("--thread-growth", type=int, default=4)
    parser.add_argument("--inotify-watch-growth", type=int, default=16)
    parser.add_argument("--index-dir-growth", type=int, default=1)
    parser.add_argument("--enable-enhancement", action="store_true",
                        help="keep background hash and neural enhancement on (off by default for repeatability)")
    parser.add_argument("--malloc-arenas", default="2",
                        help="MALLOC_ARENA_MAX for the daemon and the sessions, or `default` for glibc's eight per "
                             "core. With the default a busy daemon strands freed memory in about a hundred "
                             "per-thread arenas: 64 sessions drifted 38 MiB in two hours with no leak, which no "
                             "growth budget can tell from one. With two arenas anonymous RSS follows the memory "
                             "in use, so the same budgets catch a real leak. It costs throughput, so production "
                             "numbers need `default`.")
    parser.add_argument("--env", action="append", default=[], metavar="KEY=VALUE",
                        help="variable for every session and, through the session that spawns it, the daemon; "
                             "the caller's own IVYGREP_* variables never reach them")
    parser.add_argument("--keep-work-dir", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    selected = [kind for kind in args.calls.split(",") if kind]
    if unknown := sorted(set(selected) - set(CALL_KINDS)):
        parser.error(f"--calls: unknown kinds {unknown}; known: {', '.join(CALL_KINDS)}")
    args.call_weights = tuple((kind, 1) for kind in selected) or CALL_WEIGHTS
    for name, value in MODES[args.mode].items():
        if getattr(args, name) is None:
            setattr(args, name, value)
    if not Path("/proc/self/smaps_rollup").is_file():
        parser.error("the MCP session soak requires Linux /proc")
    phases = [phase for phase in args.phases.split(",") if phase]
    if unknown := set(phases) - set(PHASES):
        parser.error(f"unknown phases: {sorted(unknown)}")
    if "load" in phases and args.duration / args.sample_interval < 25:
        parser.error("the load phase needs at least 25 samples; lower --sample-interval or raise --duration")
    if min(args.clients, args.workspaces) < 1:
        parser.error("clients and workspaces must be positive")
    args.binary, source = args.binary.resolve(), args.repo.resolve()
    work = args.work_dir.resolve()
    args.output = args.output.resolve()
    if (layout_error := path_layout_error(source, work, args.output)) is not None:
        parser.error(layout_error)
    if work.exists() and any(work.iterdir()):
        parser.error(f"--work-dir {work} must be empty")
    home = work / "home"
    home.mkdir(parents=True)
    env = {key: value for key, value in os.environ.items() if not key.startswith("IVYGREP_") and key != "CI"}
    env.update(IVYGREP_HOME=str(home), IVYGREP_INDEX_GC_GRACE_SECS=str(args.gc_grace_seconds))
    if not args.enable_enhancement:
        env["IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT"] = "1"
    if args.malloc_arenas != "default":
        if not args.malloc_arenas.isdigit() or int(args.malloc_arenas) < 1:
            parser.error("--malloc-arenas takes a positive number or `default`")
        env["MALLOC_ARENA_MAX"] = args.malloc_arenas
    else:
        env.pop("MALLOC_ARENA_MAX", None)
    for item in args.env:
        key, separator, value = item.partition("=")
        if not key or not separator:
            parser.error(f"--env takes KEY=VALUE, got {item!r}")
        env[key] = value
    report: dict[str, Any] = {
        "schema_version": 1, "generated_at": datetime.now(timezone.utc).isoformat(), "mode": args.mode,
        "platform": platform.platform(), "machine": platform.machine(), "cpu_affinity": sorted(os.sched_getaffinity(0)),
        "binary_sha256": sha256_file(args.binary),
        "binary_version": run([str(args.binary), "--version"], source, env).strip(),
        "source_commit": run(["git", "rev-parse", "HEAD"], source, env).strip(),
        "source_dirty": bool(run(["git", "status", "--porcelain"], source, env).strip()),
        "harness_sha256": sha256_file(Path(__file__)), "phases": phases,
        "background_enhancement": args.enable_enhancement, "extra_environment": reported_environment(args.env),
        "malloc_arenas": args.malloc_arenas,
        "settings": {key: getattr(args, key) for key in ("clients", "workspaces", "duration", "churn", "stampede",
                                                          "lifecycle_cycles", "corpus", "think_time", "seed", "load_warmup",
                                                          "load_settle", "settle_every", "calls")},
        "passed": False,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    failure: BaseException | None = None
    started = time.monotonic()
    soak = Soak(args, env, home, work)
    try:
        wanted = max(args.workspaces, args.idle_workspaces if "idle" in phases else 0,
                     args.storm_workspaces if "storm" in phases else 0)
        every = prepare_workspaces(source, work / "workspaces", wanted, args.corpus, full_limit=args.workspaces)
        workspaces = every[:args.workspaces]
        phase_runs: dict[str, Callable[[], dict[str, Any]]] = {
            "stampede": lambda: soak.stampede(workspaces[0], args.stampede),
            "lifecycle": lambda: soak.lifecycle(workspaces[0], args.lifecycle_cycles),
            "load": lambda: soak.load(workspaces, args.clients, args.duration),
            "churn": lambda: soak.churn(workspaces[0], args.churn,
                                        [int(value) for value in args.churn_checkpoints.split(",") if value]),
            "idle": lambda: soak.idle(every[:args.idle_workspaces], args.idle_seconds),
            "storm": lambda: soak.storm(every[:args.storm_workspaces], args.storm_timeout),
        }
        for phase in phases:
            if phase != "stampede" and not classify_processes(owned_processes(home))["daemon"]:
                # The first search auto-spawns the daemon every later phase samples.
                opener = soak.client(workspaces[0])
                opener.initialize()
                wait_until_searchable(opener, workspaces[0])
                opener.close()
            if phase == "load":
                opener = soak.client(workspaces[0])
                opener.initialize()
                for workspace in workspaces:
                    wait_until_searchable(opener, workspace)
                opener.close()
            print(f"phase {phase} started", flush=True)
            report[phase] = phase_runs[phase]()
            summary = {key: value for key, value in report[phase].items() if key not in ("samples", "session_samples")}
            print(f"phase {phase}: {json.dumps(summary, default=str)[:3000]}", flush=True)
            if not report[phase]["passed"]:
                raise AssertionError(f"phase {phase} failed its gates")
        report["passed"] = True
    except BaseException as error:  # noqa: BLE001 - the report records every failure
        failure = error
        report["failure"] = f"{type(error).__name__}: {error}"
    finally:
        log = home / "daemon.log"
        if log.exists():
            report["daemon_log_bytes"] = log.stat().st_size
            shutil.copyfile(log, args.output.with_suffix(".daemon.log"))
        stop_owned_processes(home)
        report["duration_seconds"] = time.monotonic() - started
        args.output.write_text(json.dumps(report, indent=2) + "\n")
        if not args.keep_work_dir and failure is None:
            shutil.rmtree(work, ignore_errors=True)
    print(json.dumps({key: value for key, value in report.items()
                      if key not in PHASES}, indent=2))
    if failure is not None:
        raise RuntimeError(f"MCP session soak failed; evidence: {args.output}") from failure


if __name__ == "__main__":
    main()
