#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Measure warm MCP context packs at one and eight clients on a preindexed corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path

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

def run(command: list[str], *, cwd: Path, env: dict[str, str], check: bool = True):
    return subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True, check=check)

def sha256_file(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()

def corpus_hash(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(p for p in root.rglob("*") if p.is_file() and ".git" not in p.parts):
        digest.update(path.relative_to(root).as_posix().encode() + b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()

def isolated_env(home: Path, profile: str) -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if not key.startswith("IVYGREP_")}
    env.update(IVYGREP_HOME=str(home), IVYGREP_MODEL_PROFILE=profile, IVYGREP_NO_AUTOSPAWN="1",
               IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT="1", IVYGREP_ENHANCE_MAX_LOAD_RATIO="0",
               IVYGREP_DISABLE_QUERY_CACHE="1", HF_HUB_OFFLINE="1", TOKENIZERS_PARALLELISM="false")
    return env

def normalize(value, root: str):
    if isinstance(value, dict):
        return {key: normalize(item, root) for key, item in sorted(value.items())}
    if isinstance(value, list):
        return [normalize(item, root) for item in value]
    return value.replace(root, "<corpus>") if isinstance(value, str) else value

def daemon_resources(pid: int) -> dict[str, int]:
    proc = Path("/proc") / str(pid)
    fields = (proc / "stat").read_text().rsplit(")", 1)[1].split()
    rss_kb = int(re.search(r"^VmRSS:\s+(\d+)", (proc / "status").read_text(), re.M)[1])
    cpu_ticks = int(fields[11]) + int(fields[12])
    return {"cpu_ms": round(cpu_ticks * 1000 / os.sysconf("SC_CLK_TCK"), 3), "rss_bytes": rss_kb * 1024}

def close_owned(client) -> None:
    client.close(wait=3)
    if client.proc.poll() is None:
        client.proc.terminate()
        try:
            client.proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            client.proc.kill()
            client.proc.wait(timeout=3)

def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    low = int(position)
    high = min(low + 1, len(ordered) - 1)
    return ordered[low] + (ordered[high] - ordered[low]) * (position - low)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--home", type=Path, required=True, help="existing preindexed IVYGREP_HOME")
    parser.add_argument("--model-profile", choices=("static", "potion-code-16m-v2"), required=True)
    parser.add_argument("--rounds", type=int, default=16, help="samples per query at each concurrency")
    parser.add_argument("--budget", type=int, default=2000)
    parser.add_argument("--output", type=Path, required=True, help="new JSON path; existing files are refused")
    args = parser.parse_args()

    binary, repo, corpus, home = (p.resolve() for p in (args.binary, args.repo, args.corpus, args.home))
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error(f"--binary is not executable: {binary}")
    if not (repo / "scripts" / "soak_mcp_sessions.py").is_file() or not (repo / "scripts" / "check_daemon_equivalence.py").is_file():
        parser.error(f"--repo lacks the daemon/MCP helpers: {repo}")
    if not corpus.is_dir():
        parser.error(f"--corpus does not exist: {corpus}")
    if not Path("/proc").is_dir():
        parser.error("daemon CPU/RSS sampling requires Linux /proc")
    if not home.is_dir() or not any((home / "indexes").glob("*/metadata.sqlite3")):
        parser.error(f"--home must contain an existing workspace index: {home}")
    if (home / "daemon.sock").exists() or (home / "daemon.port").exists():
        parser.error(f"--home has an existing daemon endpoint; refusing to disturb it: {home}")
    if args.rounds < 1 or not 256 <= args.budget <= 131072:
        parser.error("--rounds must be positive and --budget must be between 256 and 131072")
    output = args.output.resolve()
    if output.exists() or not output.parent.is_dir():
        parser.error(f"--output must be a new path in an existing directory: {output}")
    corpus_digest = corpus_hash(corpus)

    sys.path.insert(0, str(repo / "scripts"))
    import check_daemon_equivalence as daemon_helpers
    import soak_mcp_sessions as mcp_helpers

    protocol_text = (repo / "src" / "protocol.rs").read_text(encoding="utf-8")
    match = re.search(r"DAEMON_PROTOCOL_VERSION:\s*u32\s*=\s*(\d+)", protocol_text)
    if not match:
        parser.error("could not read daemon protocol version from --repo/src/protocol.rs")
    protocol_version = int(match[1])

    env = isolated_env(home, args.model_profile)
    daemon = daemon_helpers.start_daemon(binary, cwd=repo, env=env, bench_home=home)
    clients = []
    try:
        warmer = mcp_helpers.McpClient(binary, env, corpus)
        clients.append(warmer)
        warmer.initialize()
        status = warmer.call("ig_status", {})
        workspaces = [item for item in status.get("workspaces", [])
                      if Path(item["workspace_root"]).resolve() == corpus]
        if not workspaces or workspaces[0].get("ready_to_query") is not True:
            raise RuntimeError(f"corpus is not ready to query: {corpus}")
        if any(workspaces[0].get(field) for field in (
            "indexing_in_progress", "enhancing_in_progress", "indexing_stalled", "enhancing_stalled"
        )):
            raise RuntimeError(f"corpus must be idle before measuring: {corpus}")

        fingerprints: dict[str, str] = {}

        def context_pack(client, query: str) -> tuple[float, str]:
            started = time.perf_counter()
            payload = client.call(
                "ig_search",
                {"query": query, "path": str(corpus), "output": "context_pack",
                 "budget_tokens": args.budget},
                timeout=180,
            )
            elapsed_ms = (time.perf_counter() - started) * 1000
            if payload.get("status") == "indexing":
                raise RuntimeError(f"status=indexing for {query!r}")
            pack = payload.get("context_pack")
            if not isinstance(pack, dict):
                raise RuntimeError(f"missing context_pack for {query!r}")
            if pack.get("budget_tokens") != args.budget or int(pack.get("used_tokens", args.budget + 1)) > args.budget:
                raise RuntimeError(f"context pack exceeded or mismatched the {args.budget}-token budget")
            stable = normalize(pack, str(corpus))
            fingerprint = hashlib.sha256(json.dumps(stable, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            return elapsed_ms, fingerprint

        for query in QUERIES:
            _, fingerprints[query] = context_pack(warmer, query)

        measurements = {}
        total_requests = args.rounds * len(QUERIES)
        for concurrency in (1, 8):
            sessions = [warmer] if concurrency == 1 else []
            for _ in range(concurrency - len(sessions)):
                client = mcp_helpers.McpClient(binary, env, corpus)
                sessions.append(client)
                clients.append(client)
                client.initialize()

            def worker(slot: int) -> list[tuple[int, float]]:
                samples = []
                for turn in range(total_requests // concurrency):
                    query_index = (turn * concurrency + slot) % len(QUERIES)
                    query = QUERIES[query_index]
                    elapsed, fingerprint = context_pack(sessions[slot], query)
                    if fingerprint != fingerprints[query]:
                        raise RuntimeError(f"normalized pack fingerprint changed for {query!r}")
                    samples.append((query_index, elapsed))
                return samples

            before = daemon_resources(daemon.proc.pid)
            started = time.perf_counter()
            with ThreadPoolExecutor(max_workers=concurrency) as pool:
                batches = list(pool.map(worker, range(concurrency)))
            wall_ms = (time.perf_counter() - started) * 1000
            after = daemon_resources(daemon.proc.pid)
            samples = [sample for batch in batches for sample in batch]
            elapsed = [sample[1] for sample in samples]
            by_query = {
                query: [sample[1] for sample in samples if QUERIES[sample[0]] == query]
                for query in QUERIES
            }
            if len(samples) != total_requests or any(len(by_query[query]) != args.rounds for query in QUERIES):
                raise RuntimeError("incomplete context-measure request distribution")
            measurements[str(concurrency)] = {
                "clients": concurrency,
                "requests": len(samples),
                "wall_ms": wall_ms,
                "requests_per_second": len(samples) / (wall_ms / 1000),
                "total_latency_p50_ms": statistics.median(elapsed),
                "total_latency_p95_ms": percentile(elapsed, 0.95),
                "elapsed_ms": elapsed,
                "per_query": {
                    query: {"elapsed_ms": by_query[query], "normalized_pack_sha256": fingerprints[query]}
                    for query in QUERIES
                },
                "daemon_before": before,
                "daemon_after": after,
                "daemon_cpu_delta_ms": round(after["cpu_ms"] - before["cpu_ms"], 3),
                "daemon_rss_delta_bytes": after["rss_bytes"] - before["rss_bytes"],
                "mcp_pids": [client.pid for client in sessions],
            }
            if concurrency == 1:
                close_owned(warmer)
                clients.remove(warmer)

        source_commit = run(["git", "-C", str(repo), "rev-parse", "HEAD"], cwd=repo, env=env, check=False).stdout.strip() or None
        corpus_commit = run(["git", "-C", str(corpus), "rev-parse", "HEAD"], cwd=repo, env=env, check=False).stdout.strip() or None
        if corpus_hash(corpus) != corpus_digest:
            raise RuntimeError("corpus content changed during measurement")
        result = {
            "schema_version": 1,
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "identity": {
                "source_repo": str(repo), "source_commit": source_commit,
                "source_dirty": bool(run(["git", "-C", str(repo), "status", "--porcelain"], cwd=repo, env=env, check=False).stdout),
                "binary": str(binary), "binary_sha256": sha256_file(binary),
                "binary_version": run([str(binary), "--version"], cwd=repo, env=env).stdout.strip(),
                "host": {"platform": platform.platform(), "machine": platform.machine(), "processor": platform.processor(),
                         "cpu_count": os.cpu_count()},
                "corpus": str(corpus), "corpus_commit": corpus_commit,
                "corpus_sha256": corpus_digest,
                "corpus_hash_method": "sha256 of sorted relative paths, NUL separators, and file bytes; .git excluded",
                "home": str(home), "daemon_pid": daemon.proc.pid, "daemon_protocol_version": protocol_version,
                "model_profile": args.model_profile, "query_result_cache_disabled": True,
                "budget_tokens": args.budget, "rounds_per_query": args.rounds,
                "queries": list(QUERIES),
            },
            "warm_fingerprints": fingerprints,
            "measurements": measurements,
        }
        with output.open("x", encoding="utf-8") as stream:
            json.dump(result, stream, indent=2, sort_keys=True)
            stream.write("\n")
        print(f"wrote {output}")
        return 0
    finally:
        for client in clients:
            close_owned(client)
        daemon.stop()

if __name__ == "__main__":
    raise SystemExit(main())
