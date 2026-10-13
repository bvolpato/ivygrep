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
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any

from check_daemon_equivalence import daemon_request, start_daemon

if sys.platform == "darwin":
    import macos_process


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


def cpu_affinity() -> list[int] | None:
    """CPUs this process may run on. macOS has no affinity sets."""
    return sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None


def process_sample(pid: int) -> dict[str, int]:
    # Missing/inaccessible processes are failures, never reassuring zero samples.
    if sys.platform == "darwin":
        # No file-backed RSS here: the footprint counts compressed pages, so it can exceed the resident size.
        sample = macos_process.memory_sample(pid)
        if min(sample.values()) <= 0:
            raise RuntimeError(f"invalid process sample: {sample}")
        return sample
    proc = Path("/proc") / str(pid)
    status = (proc / "status").read_text()
    fields = dict(line.split(":", 1) for line in status.splitlines() if ":" in line)

    def kib(name: str) -> int:
        return int(fields[name].split()[0]) * 1024

    sample = {"rss_bytes": kib("VmRSS"), "rss_anon_bytes": kib("RssAnon"), "rss_file_bytes": kib("RssFile"),
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


def resource_budgets(*, rss_growth_mib: float, total_rss_growth_mib: float, fd_growth: int,
                     thread_growth: int) -> dict[str, int]:
    """Growth budgets per daemon epoch.

    Anonymous RSS is the leak signal. File-backed RSS comes from mapped index
    segments that reindexing replaces and the kernel reclaims, so total RSS
    swings by tens of MiB within one epoch and gets a looser budget.
    """
    mib = 1024 * 1024
    return {"rss_anon_bytes": int(rss_growth_mib * mib), "rss_bytes": int(total_rss_growth_mib * mib),
            "fds": fd_growth, "threads": thread_growth}


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


def uncached_probe_hits(home: Path, repo: Path) -> list[dict[str, Any]]:
    # A new query key on every request avoids accepting a stale daemon result
    # from the query cache after a source mutation.
    return search(home, repo, f"soak revision value {time.monotonic_ns()}", probe_only=True)


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
            raise StaleProbeError(f"watcher returned stale probe: expected={expected!r}, hits={hits}")
        time.sleep(0.1)


class StaleProbeError(AssertionError):
    pass


def stored_chunk_text(value: str | bytes) -> str:
    """SQLite holds a chunk as text or as a zstd frame. Python has zstd from version 3.14."""
    if isinstance(value, str):
        return value
    try:
        from compression import zstd
        return zstd.decompress(value).decode(errors="replace")
    except Exception:
        return f"<{len(value)} compressed bytes>"


def stale_probe_evidence(home: Path, repo: Path, env: dict[str, str]) -> dict[str, Any]:
    """What the source tree, the index stores, and the daemon each hold for the probe.

    Call it while the daemon still runs. It only reads: a local `ig` search could
    repair a store and replace the state under inspection. Each fact is read
    independently, and a fact that cannot be read is reported as its error text.
    """
    def read(fact: Any) -> Any:
        try:
            return fact()
        except Exception as error:
            return f"unavailable: {error}"

    def probe_file_text() -> str | None:
        probe = repo / PROBE
        return probe.read_text() if probe.exists() else None

    def git_clean() -> bool:
        return not run(["git", "status", "--porcelain"], repo, env).strip()

    def stores() -> list[dict[str, Any]]:
        found = []
        for index in sorted((home / "indexes").iterdir()):
            if not (index / "metadata.sqlite3").is_file():
                continue
            connection = sqlite3.connect(f"file:{index / 'metadata.sqlite3'}?mode=ro", uri=True)
            try:
                rows = connection.execute("SELECT text FROM chunks WHERE file_path = ?", (PROBE,)).fetchall()
            finally:
                connection.close()
            texts = [stored_chunk_text(text) for (text,) in rows]
            found.append({"sqlite_probe_texts": texts,
                          "snapshot_lists_probe": f'"{PROBE}"' in (index / "merkle_snapshot.json").read_text(),
                          "clean_checkout_state_recorded": (index / "indexed_git_state").is_file()})
        return found

    def daemon_uncached_hits() -> list[dict[str, Any]]:
        # The daemon cannot have cached this query.
        hits = search(home, repo, f"soak revision value {time.monotonic_ns()}", probe_only=True)
        return [{"preview": hit.get("preview"), "reason": hit.get("reason")} for hit in hits]

    return {
        "probe_file_text": read(probe_file_text),
        "git_worktree_clean": read(git_clean),
        "index_stores": read(stores),
        "daemon_uncached_query_hits": read(daemon_uncached_hits),
    }


def workspace_index_dir(home: Path, repo: Path) -> Path:
    for metadata_path in (home / "indexes").glob("*/workspace.json"):
        metadata = json.loads(metadata_path.read_text())
        indexed_root = Path(metadata["root"])
        if indexed_root.resolve() == repo.resolve():
            return metadata_path.parent
    raise FileNotFoundError(f"workspace index is missing for {repo.name}")


def mutation_probe_store_state(home: Path, repo: Path, *, overlay: bool) -> dict[str, Any]:
    index_dir = workspace_index_dir(home, repo)
    sqlite_path = index_dir / ("overlay.sqlite3" if overlay else "metadata.sqlite3")
    connection = sqlite3.connect(f"file:{sqlite_path}?mode=ro", uri=True)
    try:
        texts = [stored_chunk_text(row[0]) for row in connection.execute(
            "SELECT text FROM chunks WHERE file_path = ?", (PROBE,))]
        has_tombstones = connection.execute(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'tombstones'"
        ).fetchone() is not None
        tombstones = ({row[0] for row in connection.execute("SELECT file_path FROM tombstones")}
                      if has_tombstones else set())
    finally:
        connection.close()
    snapshot = json.loads((index_dir / "merkle_snapshot.json").read_text())
    return {
        "sqlite_chunk_texts": texts,
        "snapshot_lists_probe": PROBE in snapshot.get("files", {}),
        "overlay_tombstones_probe": PROBE in tombstones,
        "clean_checkout_state_recorded": (index_dir / "indexed_git_state").is_file(),
    }


def assert_mutation_probe_state(
    state: dict[str, Any],
    expected: str | None,
    *,
    overlay: bool,
    overlay_tombstone_expected: bool | None = None,
) -> None:
    if expected is None:
        if state["sqlite_chunk_texts"] or state["snapshot_lists_probe"]:
            raise AssertionError(f"deleted probe remains in index state: {state}")
        if overlay and not state["overlay_tombstones_probe"]:
            raise AssertionError(f"deleted base probe has no overlay tombstone: {state}")
    else:
        if not state["snapshot_lists_probe"] or not state["sqlite_chunk_texts"]:
            raise AssertionError(f"live probe is missing from index state: {state}")
        if (overlay and overlay_tombstone_expected is not None
                and state["overlay_tombstones_probe"] != overlay_tombstone_expected):
            raise AssertionError(f"overlay tombstone state is incorrect: {state}")


def verify_mutation_state(
    home: Path,
    repo: Path,
    expected: str | None,
    *,
    overlay: bool,
    overlay_tombstone_expected: bool | None = None,
    require_clean_checkout: bool = False,
    stage: str,
    report: dict[str, Any],
) -> None:
    report["active_stage"] = stage
    deadline = time.monotonic() + 20
    uncached_hits: list[dict[str, Any]] = []
    cached_hits: list[dict[str, Any]] = []
    state: dict[str, Any] | None = None
    state_error: str | None = None
    clean = False
    while True:
        try:
            uncached_hits = uncached_probe_hits(home, repo)
            cached_hits = search(home, repo, "soak revision value", probe_only=True)
            state = mutation_probe_store_state(home, repo, overlay=overlay)
            state_error = None
            try:
                assert_mutation_probe_state(
                    state,
                    expected,
                    overlay=overlay,
                    overlay_tombstone_expected=overlay_tombstone_expected,
                )
            except AssertionError as error:
                state_error = str(error)
            if require_clean_checkout:
                clean = not run(["git", "status", "--porcelain"], repo, os.environ.copy()).strip()
            clean_state_ready = (
                not require_clean_checkout or (clean and state["clean_checkout_state_recorded"])
            )
            if (probe_matches(uncached_hits, expected)
                    and probe_matches(cached_hits, expected)
                    and state_error is None and clean_state_ready):
                break
        except Exception as error:
            state_error = f"unavailable: {error}"
        if time.monotonic() >= deadline:
            report["last_stage_observation"] = {
                "uncached_hits": uncached_hits,
                "cached_hits": cached_hits,
                "store_state": state,
                "store_error": state_error,
                "git_worktree_clean": clean,
            }
            raise StaleProbeError(
                f"mutation state did not settle at {stage}: expected={expected!r}, "
                f"uncached_hits={uncached_hits}, cached_hits={cached_hits}, "
                f"store_state={state}, store_error={state_error}, git_clean={clean}"
            )
        time.sleep(0.1)
    check = {
        "stage": stage,
        "expected_present": expected is not None,
        "uncached_hits": uncached_hits,
        "cached_hits": cached_hits,
        "store_state": state,
        "passed": True,
    }
    if require_clean_checkout:
        check["git_worktree_clean"] = clean
    report["mutation_checks"].append(check)


def mutation_revision(revision: int) -> str:
    return f"pub fn soak_revision() -> u64 {{ {revision} }}"


def clean_branch_mutation_storm(home: Path, repo: Path, git: Any, report: dict[str, Any]) -> None:
    stop = threading.Event()
    lock = threading.Lock()
    generation = [0]
    stable_deleted_observed = threading.Event()
    stats = {"queries": 0, "stable_deleted_queries": 0, "stable_deleted_hits": 0,
             "query_errors": []}
    report["clean_branch_mutation_storm"] = stats
    probe = repo / PROBE

    def query_worker() -> None:
        while not stop.is_set():
            with lock:
                generation_before = generation[0]
                present_before = probe.exists()
            try:
                hits = uncached_probe_hits(home, repo)
                with lock:
                    generation_after = generation[0]
                    present_after = probe.exists()
                    stats["queries"] += 1
                    stable_deleted_window = (
                        generation_before == generation_after
                        and generation_after % 2 == 0
                        and not present_before
                        and not present_after
                    )
                    if stable_deleted_window:
                        stats["stable_deleted_queries"] += 1
                        if not probe_matches(hits, None):
                            stats["stable_deleted_hits"] += 1
                        stable_deleted_observed.set()
            except Exception as error:
                with lock:
                    if len(stats["query_errors"]) < 5:
                        stats["query_errors"].append(str(error))
            stop.wait(0.01)

    def checkout(branch: str) -> None:
        with lock:
            generation[0] += 1
        try:
            git("checkout", "-q", branch)
        finally:
            with lock:
                generation[0] += 1

    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [executor.submit(query_worker) for _ in range(2)]
        try:
            for cycle in range(12):
                checkout("deleted-probe")
                if cycle == 0 and not stable_deleted_observed.wait(timeout=10):
                    raise AssertionError("no query observed a stable deleted-branch window")
                checkout("main")
            checkout("deleted-probe")
        finally:
            stop.set()
            for future in futures:
                future.result(timeout=35)

    if (not stats["queries"] or not stats["stable_deleted_queries"]
            or stats["query_errors"] or stats["stable_deleted_hits"]):
        raise AssertionError(f"clean branch mutation queries failed: {stats}")


def run_mutation_correctness(binary: Path, root: Path, home: Path, env: dict[str, str],
                             report: dict[str, Any]) -> None:
    repo = root / "repo"
    worktree = root / "deleted-branch"
    repo.mkdir()
    env.update(IVYGREP_HOME=str(home), IVYGREP_NO_AUTOSPAWN="1",
               IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT="1")
    env.setdefault("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0")

    def git(*args: str, cwd: Path = repo) -> None:
        run(["git", "-c", "user.name=Mutation Soak", "-c", "user.email=soak@example.invalid",
             "-c", "commit.gpgSign=false", "-c", "core.hooksPath=/dev/null", *args], cwd, env)

    def index(path: Path) -> None:
        run([str(binary), "--add", str(path), "--force", "--hash", "--json"], path, env)
        run([str(binary), "--enhance-hash-internal", str(path)], path, env)

    git("init", "-q", "-b", "main")
    probe = repo / PROBE
    probe.parent.mkdir(parents=True)
    initial = mutation_revision(0)
    probe.write_text(initial + "\n")
    git("add", ".")
    git("commit", "-qm", "initial probe")

    # Create a linked branch whose committed tree deletes a file inherited from
    # the indexed base. This exercises the overlay tombstone path before any
    # dirty-worktree mutations happen.
    git("checkout", "-q", "-b", "deleted-probe")
    probe.unlink()
    git("add", "-A")
    git("commit", "-qm", "delete probe in branch")
    git("checkout", "-q", "main")
    git("worktree", "add", "-q", "--detach", str(worktree), "deleted-probe")

    index(repo)
    index(worktree)
    daemon = start_daemon(binary, cwd=repo, env=env, bench_home=home)
    report["transport"] = "direct-daemon-rpc-with-cached-and-uncached-queries"

    def evidence(path: Path, *, overlay: bool) -> dict[str, Any]:
        def read(fact: Any) -> Any:
            try:
                return fact()
            except Exception as error:
                return f"unavailable: {error}"

        def file_text() -> str | None:
            current = path / PROBE
            return current.read_text() if current.exists() else None

        return {
            "probe_file_text": read(file_text),
            "git_worktree_status": read(lambda: run(["git", "status", "--porcelain"], path, env)),
            "index_store": read(lambda: mutation_probe_store_state(home, path, overlay=overlay)),
            "daemon_uncached_query_hits": read(lambda: uncached_probe_hits(home, path)),
        }

    try:
        verify_mutation_state(home, repo, initial, overlay=False, stage="base-initial", report=report)
        verify_mutation_state(home, worktree, None, overlay=True,
                              stage="branch-delete-tombstone", report=report)

        clean_branch_mutation_storm(home, repo, git, report)
        verify_mutation_state(home, repo, None, overlay=False, require_clean_checkout=True,
                              stage="base-clean-branch-delete", report=report)
        git("checkout", "-q", "main")
        verify_mutation_state(home, repo, initial, overlay=False,
                              stage="base-clean-branch-restore", report=report)

        revision = mutation_revision(1)
        probe.write_text(revision + "\n")
        verify_mutation_state(home, repo, revision, overlay=False, stage="base-update", report=report)

        probe.unlink()
        verify_mutation_state(home, repo, None, overlay=False, stage="base-delete", report=report)

        revision = mutation_revision(2)
        probe.write_text(revision + "\n")
        verify_mutation_state(home, repo, revision, overlay=False, stage="base-restore", report=report)

        worktree_probe = worktree / PROBE
        revision = mutation_revision(3)
        worktree_probe.parent.mkdir(parents=True, exist_ok=True)
        worktree_probe.write_text(revision + "\n")
        verify_mutation_state(home, worktree, revision, overlay=True,
                              overlay_tombstone_expected=True,
                              stage="overlay-restore", report=report)

        worktree_probe.unlink()
        verify_mutation_state(home, worktree, None, overlay=True,
                              stage="overlay-dirty-delete", report=report)

        revision = mutation_revision(4)
        worktree_probe.write_text(revision + "\n")
        verify_mutation_state(home, worktree, revision, overlay=True,
                              overlay_tombstone_expected=True,
                              stage="overlay-dirty-restore", report=report)
    except Exception:
        report["stale_probe_evidence"] = {
            "base": evidence(repo, overlay=False),
            "overlay": evidence(worktree, overlay=True),
        }
        raise
    finally:
        daemon.stop()


def main_correctness(args: argparse.Namespace, binary: Path) -> None:
    env = os.environ.copy()
    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "mode": "mutation-correctness",
        "platform": platform.platform(),
        "machine": platform.machine(),
        "binary_sha256": sha256_file(binary),
        "binary_version": run([str(binary), "--version"], Path.cwd(), env).strip(),
        "harness_sha256": sha256_file(Path(__file__)),
        "transport": "direct-daemon-rpc-no-cli-fallback",
        "mutation_checks": [],
        "passed": False,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    failure = None
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="ivygrep-mutation-") as temporary:
        root = Path(temporary)
        home = root / "home"
        try:
            run_mutation_correctness(binary, root, home, env, report)
            report["passed"] = True
        except Exception as error:
            failure = error
            report["failure"] = str(error)
        finally:
            log = home / "equivalence-daemon.log"
            if log.exists():
                shutil.copyfile(log, args.output.with_suffix(".daemon.log"))
            report["duration_seconds"] = time.monotonic() - started
            args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    if failure is not None:
        raise RuntimeError(f"mutation correctness failed; evidence: {args.output}") from failure


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--correctness-only", action="store_true",
                        help="run bounded create/update/delete/restore checks without resource sampling")
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--duration", type=float, default=300.0, help="total loaded seconds, excluding cooldowns")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--mutation-interval", type=float, default=0.25)
    parser.add_argument("--check-interval", type=float, default=10.0)
    parser.add_argument("--cooldown", type=float, default=10.0)
    parser.add_argument("--warmup", type=float, default=30.0,
                        help="run the same mutation/query workload before sampling each PID")
    parser.add_argument("--restarts", type=int, default=2)
    parser.add_argument("--rss-growth-mib", type=float, default=32.0,
                        help="anonymous RSS growth budget per daemon epoch")
    parser.add_argument("--total-rss-growth-mib", type=float, default=96.0,
                        help="total RSS growth budget per daemon epoch, including mapped index files")
    parser.add_argument("--fd-growth", type=int, default=8)
    parser.add_argument("--thread-growth", type=int, default=4)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not (sys.platform == "darwin" or Path("/proc/self/status").is_file()) or not hasattr(os, "killpg"):
        parser.error("daemon soak requires Linux /proc or macOS libproc, and process groups")
    if args.correctness_only:
        main_correctness(args, args.binary.resolve())
        return
    numeric = (args.duration, args.mutation_interval, args.check_interval, args.cooldown, args.warmup,
               args.rss_growth_mib, args.total_rss_growth_mib)
    if not all(math.isfinite(value) for value in numeric):
        parser.error("numeric arguments must be finite")
    if args.restarts < 0 or args.workers < 1 or args.duration / (args.restarts + 1) < 30:
        parser.error("positive workers and at least 30 loaded seconds per daemon epoch are required")
    if min(args.mutation_interval, args.check_interval) <= 0 or min(args.cooldown, args.warmup, args.rss_growth_mib, args.total_rss_growth_mib, args.fd_growth, args.thread_growth) < 0:
        parser.error("intervals must be positive and budgets/cooldown nonnegative")
    binary, source = args.binary.resolve(), args.repo.resolve()
    env = os.environ.copy()
    budgets = resource_budgets(rss_growth_mib=args.rss_growth_mib, total_rss_growth_mib=args.total_rss_growth_mib,
                               fd_growth=args.fd_growth, thread_growth=args.thread_growth)
    report: dict[str, Any] = {
        "schema_version": 3, "generated_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(), "machine": platform.machine(),
        "cpu_affinity": cpu_affinity(),
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
    if sys.platform == "darwin":
        report["resource_sampling"] = macos_process.METRICS
    args.output.parent.mkdir(parents=True, exist_ok=True)
    failure = None
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="ivygrep-soak-") as temp:
        root = Path(temp)
        repo, home = root / "repo", root / "home"
        env.update(IVYGREP_HOME=str(home), IVYGREP_NO_AUTOSPAWN="1",
                   IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT="1")
        # The explicit hash enhancement below must not wait for a quiet host.
        env.setdefault("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0")
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
            if isinstance(error, StaleProbeError):
                report["stale_probe_evidence"] = stale_probe_evidence(home, repo, env)
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
