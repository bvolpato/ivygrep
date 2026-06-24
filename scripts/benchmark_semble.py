#!/usr/bin/env python3
"""Benchmark ivygrep and Semble on Semble's pinned public retrieval tasks.

Run this script inside Semble's uv environment so its exact source checkout and
dependencies are used:

    uv run --project /tmp/semble python scripts/benchmark_semble.py \
      --semble-repo /tmp/semble --repo axum --repo fastapi --repo trpc
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import platform
import shutil
import signal
import socket
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any


DAEMON_PROTOCOL_VERSION = 3
DEFAULT_REPOS = ("axum", "fastapi", "trpc")


@dataclass(frozen=True)
class RankedHit:
    file_path: str
    start_line: int
    end_line: int
    content: str
    score: float


@dataclass(frozen=True)
class QueryRecord:
    repo: str
    language: str
    category: str
    query: str
    engine: str
    ndcg_at_10: float
    latency_ms: float
    returned_tokens: int
    relevant_ranks: tuple[int, ...]
    hits: tuple[RankedHit, ...]


@dataclass(frozen=True)
class BenchmarkSpec:
    name: str
    language: str
    benchmark_dir: Path


def persisted_hit(hit: RankedHit) -> dict[str, str | int | float]:
    return {
        "file_path": hit.file_path,
        "start_line": hit.start_line,
        "end_line": hit.end_line,
        "score": hit.score,
    }


def run(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    timeout: int = 600,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=True,
    )


def percentile(values: list[float], quantile: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = math.ceil(quantile * len(ordered)) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def dcg(relevances: list[int]) -> float:
    return sum(rel / math.log2(index + 2) for index, rel in enumerate(relevances))


def ndcg_at_10(relevant_ranks: list[int], relevant_count: int) -> float:
    if relevant_count == 0:
        return 0.0
    relevances = [0] * 10
    for rank in relevant_ranks:
        if 1 <= rank <= 10:
            relevances[rank - 1] = 1
    ideal = dcg([1] * min(10, relevant_count))
    return dcg(relevances) / ideal if ideal else 0.0


def path_matches(file_path: str, target_path: str) -> bool:
    file_path = file_path.replace("\\", "/")
    target_path = target_path.replace("\\", "/")
    return (
        file_path == target_path
        or file_path.endswith(f"/{target_path}")
        or target_path.endswith(f"/{file_path}")
    )


def target_rank(hits: list[RankedHit], target: Any) -> int | None:
    for rank, hit in enumerate(hits, 1):
        if not path_matches(hit.file_path, target.path):
            continue
        if not target.has_span or not (
            hit.end_line < target.start_line or hit.start_line > target.end_line
        ):
            return rank
    return None


def score_hits(hits: list[RankedHit], targets: tuple[Any, ...]) -> tuple[float, tuple[int, ...]]:
    ranks = tuple(
        rank for target in targets if (rank := target_rank(hits, target)) is not None
    )
    return ndcg_at_10(list(ranks), len(targets)), ranks


def flatten_ivygrep_hits(response: dict[str, Any]) -> list[RankedHit]:
    raw_hits = response.get("hits", [])
    hits = [
        RankedHit(
            file_path=str(hit["file_path"]),
            start_line=int(hit["start_line"]),
            end_line=int(hit["end_line"]),
            content=str(hit.get("preview", "")),
            score=float(hit.get("score", 0.0)),
        )
        for hit in raw_hits
    ]
    hits.sort(key=lambda hit: (-hit.score, hit.file_path, hit.start_line))
    return hits


def summarize(records: list[QueryRecord]) -> dict[str, Any]:
    by_engine: dict[str, list[QueryRecord]] = defaultdict(list)
    for record in records:
        by_engine[record.engine].append(record)

    summary: dict[str, Any] = {}
    for engine, engine_records in sorted(by_engine.items()):
        by_category: dict[str, list[float]] = defaultdict(list)
        for record in engine_records:
            by_category[record.category].append(record.ndcg_at_10)
        latencies = [record.latency_ms for record in engine_records]
        summary[engine] = {
            "queries": len(engine_records),
            "ndcg_at_10": statistics.mean(
                record.ndcg_at_10 for record in engine_records
            ),
            "latency_p50_ms": percentile(latencies, 0.50),
            "latency_p95_ms": percentile(latencies, 0.95),
            "mean_returned_tokens": statistics.mean(
                record.returned_tokens for record in engine_records
            ),
            "by_category": {
                category: statistics.mean(scores)
                for category, scores in sorted(by_category.items())
            },
        }
    return summary


class DaemonProcess:
    def __init__(self, process: subprocess.Popen[bytes], log: Any, log_path: Path):
        self.process = process
        self.log = log
        self.log_path = log_path

    def stop(self) -> None:
        if self.process.poll() is None:
            if os.name == "nt":
                self.process.terminate()
            else:
                os.killpg(self.process.pid, signal.SIGTERM)
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                if os.name == "nt":
                    self.process.kill()
                else:
                    os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait(timeout=5)
        self.log.close()


def start_daemon(
    binary: Path, *, cwd: Path, env: dict[str, str], home: Path
) -> DaemonProcess:
    endpoint = home / ("daemon.port" if os.name == "nt" else "daemon.sock")
    endpoint.unlink(missing_ok=True)
    log_path = home / "semble-benchmark-daemon.log"
    log = log_path.open("wb")
    process = subprocess.Popen(
        [str(binary), "--daemon"],
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=os.name != "nt",
    )
    daemon = DaemonProcess(process, log, log_path)
    try:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError("ivygrep daemon exited before becoming ready")
            if endpoint.exists():
                return daemon
            time.sleep(0.05)
        raise TimeoutError("ivygrep daemon did not become ready")
    except BaseException:
        daemon.stop()
        raise


class IvygrepDaemonClient:
    def __init__(self, home: Path):
        self.home = home
        self.connection: socket.socket | None = None
        self.reader: Any = None

    def __enter__(self) -> IvygrepDaemonClient:
        self._connect()
        return self

    def __exit__(self, _exc_type: Any, _exc: Any, _traceback: Any) -> None:
        self.close()

    def _connect(self) -> None:
        if os.name == "nt":
            port = int((self.home / "daemon.port").read_text().strip())
            self.connection = socket.create_connection(("127.0.0.1", port), timeout=120)
        else:
            self.connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.connection.settimeout(120)
            self.connection.connect(str(self.home / "daemon.sock"))
        self.reader = self.connection.makefile("rb")

    def close(self) -> None:
        if self.reader is not None:
            self.reader.close()
            self.reader = None
        if self.connection is not None:
            self.connection.close()
            self.connection = None

    def query(
        self,
        repo: Path,
        query: str,
        limit: int,
        type_filter: str | None = None,
    ) -> tuple[dict[str, Any], float]:
        request = {
            "protocol_version": DAEMON_PROTOCOL_VERSION,
            "type": "search",
            "path": str(repo),
            "query": query,
            "limit": limit,
            "context": 2,
            "type_filter": type_filter,
            "include_globs": [],
            "exclude_globs": [],
            "scope_path": None,
            "scope_is_file": False,
            "skip_gitignore": False,
            "force_neural": True,
        }
        payload = json.dumps(request).encode() + b"\n"
        assert self.connection is not None and self.reader is not None
        started = time.perf_counter()
        self.connection.sendall(payload)
        response_bytes = self.reader.readline()
        elapsed_ms = (time.perf_counter() - started) * 1000
        if not response_bytes:
            raise RuntimeError("ivygrep daemon closed connection without response")
        response = json.loads(response_bytes)
        if response.get("type") == "error":
            raise RuntimeError(response.get("message", "ivygrep search failed"))
        return response, elapsed_ms


def wait_for_neural_model(daemon: DaemonProcess) -> None:
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        if daemon.process.poll() is not None:
            raise RuntimeError("ivygrep daemon exited while loading neural model")
        if daemon.log_path.exists() and "embedding model ready" in daemon.log_path.read_text(
            encoding="utf-8", errors="replace"
        ):
            return
        time.sleep(0.1)
    raise TimeoutError("timed out waiting for ivygrep neural model")


def directory_size(path: Path) -> int:
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def ivygrep_benchmark_env() -> dict[str, str]:
    return os.environ.copy() | {
        "IVYGREP_NO_AUTOSPAWN": "1",
        "IVYGREP_ENHANCE_MAX_LOAD_RATIO": "0",
        "IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT": "1",
        "IVYGREP_DISABLE_QUERY_CACHE": "1",
    }


def public_binary_label(binary: Path, root: Path) -> str:
    try:
        return binary.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return binary.name


def materialize_indexed_files(source: Path, destination: Path, index: Any) -> None:
    for relative in sorted({chunk.file_path for chunk in index.chunks}):
        source_file = source / relative
        if not source_file.is_file():
            continue
        destination_file = destination / relative
        destination_file.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_file, destination_file)


def git_sha(path: Path) -> str:
    return subprocess.check_output(
        ["git", "-C", str(path), "rev-parse", "HEAD"], text=True
    ).strip()


def git_dirty(path: Path) -> bool:
    return bool(
        subprocess.check_output(
            ["git", "-C", str(path), "status", "--porcelain"], text=True
        ).strip()
    )


def build_semble_index(repo: Path, model: Any, modules: dict[str, Any]) -> Any:
    content = (modules["ContentType"].CODE,)
    bm25, semantic, chunks = modules["create_index_from_path"](
        repo,
        model=model,
        content=content,
        display_root=repo,
    )
    return modules["SembleIndex"](
        model,
        bm25,
        semantic,
        chunks,
        modules["DEFAULT_MODEL_NAME"],
        root=repo,
        content=content,
    )


def benchmark_semble_repo(
    spec: Any,
    tasks: list[Any],
    *,
    model: Any,
    modules: dict[str, Any],
    query_runs: int,
    top_k: int,
    persistence_root: Path,
) -> tuple[list[QueryRecord], dict[str, Any], Any]:
    started = time.perf_counter()
    index = build_semble_index(spec.benchmark_dir, model, modules)
    index_ms = (time.perf_counter() - started) * 1000
    index.search("benchmark warmup query", top_k=top_k)

    records = []
    for task in tasks:
        latencies = []
        results = []
        for run_index in range(query_runs):
            query = task.query + (" " * run_index)
            started = time.perf_counter()
            results = index.search(query, top_k=top_k)
            latencies.append((time.perf_counter() - started) * 1000)
        hits = [
            RankedHit(
                result.chunk.file_path,
                result.chunk.start_line,
                result.chunk.end_line,
                result.chunk.content,
                float(result.score),
            )
            for result in results
        ]
        ndcg, ranks = score_hits(hits, task.all_relevant)
        records.append(
            QueryRecord(
                repo=task.repo,
                language=task.language,
                category=task.category,
                query=task.query,
                engine="semble",
                ndcg_at_10=ndcg,
                latency_ms=statistics.median(latencies),
                returned_tokens=sum(len(hit.content) // 4 for hit in hits),
                relevant_ranks=ranks,
                hits=tuple(hits),
            )
        )

    persist_path = persistence_root / spec.name
    index.save(persist_path)
    return records, {
        "index_ms": index_ms,
        "index_bytes": directory_size(persist_path),
        "chunks": len(index.chunks),
    }, index


def index_ivygrep_repo(
    binary: Path, repo: Path, env: dict[str, str], *, force: bool
) -> dict[str, float | int]:
    phases: dict[str, float | int] = {}
    command = [str(binary), "--add", str(repo), "--no-watch", "--hash", "--json"]
    if force:
        command.append("--force")
    started = time.perf_counter()
    run(command, cwd=repo, env=env)
    phases["lexical_ms"] = (time.perf_counter() - started) * 1000

    started = time.perf_counter()
    run([str(binary), "--enhance-hash-internal", str(repo)], cwd=repo, env=env)
    phases["hash_ms"] = (time.perf_counter() - started) * 1000

    started = time.perf_counter()
    run([str(binary), "--enhance-internal", str(repo)], cwd=repo, env=env)
    phases["neural_ms"] = (time.perf_counter() - started) * 1000
    phases["ready_ms"] = (
        float(phases["lexical_ms"])
        + float(phases["hash_ms"])
        + float(phases["neural_ms"])
    )
    status = json.loads(
        run([str(binary), "--status", "--json"], cwd=repo, env=env).stdout
    )
    resolved_repo = repo.resolve()
    workspace = next(
        item for item in status if Path(item["root"]).resolve() == resolved_repo
    )
    phases["index_bytes"] = int(workspace["index_size_bytes"])
    phases["chunks"] = int(workspace["chunk_count"])
    return phases


def benchmark_ivygrep_repo(
    spec: Any,
    tasks: list[Any],
    *,
    client: IvygrepDaemonClient,
    query_runs: int,
    top_k: int,
) -> list[QueryRecord]:
    client.query(spec.benchmark_dir, "benchmark warmup query", top_k)
    records = []
    for task in tasks:
        latencies = []
        response = {}
        for run_index in range(query_runs):
            query = task.query + (" " * run_index)
            response, elapsed_ms = client.query(spec.benchmark_dir, query, top_k)
            latencies.append(elapsed_ms)
        hits = flatten_ivygrep_hits(response)[:top_k]
        if hits and not all(
            hit.get("neural_executed", False) for hit in response.get("hits", [])
        ):
            raise RuntimeError(f"ivygrep neural retrieval did not execute for {task.query!r}")
        ndcg, ranks = score_hits(hits, task.all_relevant)
        records.append(
            QueryRecord(
                repo=task.repo,
                language=task.language,
                category=task.category,
                query=task.query,
                engine="ivygrep",
                ndcg_at_10=ndcg,
                latency_ms=statistics.median(latencies),
                returned_tokens=sum(len(hit.content) // 4 for hit in hits),
                relevant_ranks=ranks,
                hits=tuple(hits),
            )
        )
    return records


def choose_mutation_file(repo: Path) -> Path:
    extensions = (".rs", ".py", ".ts", ".js", ".go", ".java", ".cpp", ".c")
    return next(
        path
        for path in sorted(repo.rglob("*"))
        if path.is_file() and path.suffix in extensions and ".git" not in path.parts
    )


def benchmark_refresh(
    source_repo: Path,
    *,
    binary: Path,
    base_env: dict[str, str],
    model: Any,
    modules: dict[str, Any],
    work_root: Path,
) -> dict[str, Any]:
    repo = work_root / "refresh-repo"
    shutil.copytree(source_repo, repo, ignore=shutil.ignore_patterns(".git"))
    ivy_home = work_root / "refresh-ivygrep-home"
    env = base_env | {"IVYGREP_HOME": str(ivy_home)}

    initial_ivygrep = index_ivygrep_repo(binary, repo, env, force=True)
    build_semble_index(repo, model, modules)

    mutation = choose_mutation_file(repo)
    comment = "# ivygrep semble refresh benchmark\n" if mutation.suffix == ".py" else "// ivygrep semble refresh benchmark\n"
    with mutation.open("a", encoding="utf-8") as handle:
        handle.write(comment)

    started = time.perf_counter()
    run(
        [str(binary), "--add", str(repo), "--no-watch", "--hash", "--json"],
        cwd=repo,
        env=env,
    )
    ivy_lexical_ms = (time.perf_counter() - started) * 1000

    started = time.perf_counter()
    run([str(binary), "--enhance-hash-internal", str(repo)], cwd=repo, env=env)
    ivy_hash_ms = (time.perf_counter() - started) * 1000

    started = time.perf_counter()
    run([str(binary), "--enhance-internal", str(repo)], cwd=repo, env=env)
    ivy_neural_ms = (time.perf_counter() - started) * 1000

    started = time.perf_counter()
    refreshed_semble = build_semble_index(repo, model, modules)
    semble_full_ms = (time.perf_counter() - started) * 1000

    return {
        "repo": source_repo.name,
        "mutated_file": mutation.relative_to(repo).as_posix(),
        "ivygrep_initial_ready_ms": float(initial_ivygrep["ready_ms"]),
        "ivygrep_lexical_refresh_ms": ivy_lexical_ms,
        "ivygrep_full_refresh_ms": ivy_lexical_ms + ivy_hash_ms + ivy_neural_ms,
        "semble_full_refresh_ms": semble_full_ms,
        "semble_chunks": len(refreshed_semble.chunks),
    }


def render_markdown(payload: dict[str, Any]) -> str:
    def winner(left: float, right: float, *, lower_is_better: bool) -> str:
        if left == right:
            return "Tie"
        left_wins = left < right if lower_is_better else left > right
        return "ivygrep" if left_wins else "Semble"

    summary = payload["summary"]
    ivy = summary["ivygrep"]
    semble = summary["semble"]
    refresh = payload["refresh"]
    quality_delta = ivy["ndcg_at_10"] - semble["ndcg_at_10"]
    category_rows = "\n".join(
        f"| {category.title()} | {ivy['by_category'][category]:.3f} | "
        f"{semble['by_category'][category]:.3f} |"
        for category in sorted(ivy["by_category"])
    )
    indexing_rows = "\n".join(
        f"| {repo} | {metrics['ivygrep']['ready_ms']:.0f} ms | "
        f"{metrics['semble']['index_ms']:.0f} ms | "
        f"{metrics['semble']['index_ms'] / metrics['ivygrep']['ready_ms']:.2f}x |"
        for repo, metrics in payload["indexing"].items()
    )
    quality_leader = winner(
        ivy["ndcg_at_10"], semble["ndcg_at_10"], lower_is_better=False
    )
    latency_leader = winner(
        ivy["latency_p50_ms"], semble["latency_p50_ms"], lower_is_better=True
    )
    latency_ratio = max(ivy["latency_p50_ms"], semble["latency_p50_ms"]) / min(
        ivy["latency_p50_ms"], semble["latency_p50_ms"]
    )
    if quality_leader == "Tie" and latency_leader == "Tie":
        quality_latency_verdict = (
            "Overall retrieval quality and warm p50 latency are tied."
        )
    elif quality_leader == "Tie":
        quality_latency_verdict = (
            "Overall retrieval quality is tied; "
            f"{latency_leader} leads warm p50 latency by {latency_ratio:.1f}x."
        )
    elif latency_leader == "Tie":
        quality_latency_verdict = (
            f"{quality_leader} leads overall retrieval quality by "
            f"{abs(quality_delta):.3f} nDCG@10; warm p50 latency is tied."
        )
    elif quality_leader == latency_leader:
        quality_latency_verdict = (
            f"{quality_leader} leads overall retrieval quality by "
            f"{abs(quality_delta):.3f} nDCG@10 and warm p50 latency by "
            f"{latency_ratio:.1f}x."
        )
    else:
        quality_latency_verdict = (
            f"{quality_leader} leads overall retrieval quality by "
            f"{abs(quality_delta):.3f} nDCG@10; {latency_leader} leads warm p50 "
            f"latency by {latency_ratio:.1f}x."
        )
    token_leader = winner(
        ivy["mean_returned_tokens"],
        semble["mean_returned_tokens"],
        lower_is_better=True,
    )
    if token_leader == "Tie":
        token_verdict = "Both tools return the same mean token count in top-10 results."
    elif min(ivy["mean_returned_tokens"], semble["mean_returned_tokens"]) == 0:
        token_verdict = (
            f"{token_leader} returns no tokens in top-10 results; "
            "a multiplicative token ratio is undefined."
        )
    else:
        token_ratio = max(
            ivy["mean_returned_tokens"], semble["mean_returned_tokens"]
        ) / min(ivy["mean_returned_tokens"], semble["mean_returned_tokens"])
        token_verdict = (
            f"{token_leader} returns {token_ratio:.1f}x fewer tokens in top-10 results."
        )
    refresh_leader = winner(
        refresh["ivygrep_full_refresh_ms"],
        refresh["semble_full_refresh_ms"],
        lower_is_better=True,
    )
    if refresh_leader == "Tie":
        refresh_verdict = (
            "Full one-file refresh time is tied; ivygrep exposes lexical changes "
            "before neural refresh completes."
        )
    elif refresh_leader == "ivygrep":
        refresh_ratio = (
            refresh["semble_full_refresh_ms"] / refresh["ivygrep_full_refresh_ms"]
        )
        refresh_verdict = (
            f"ivygrep full one-file refresh is {refresh_ratio:.1f}x faster and "
            "exposes lexical changes before neural refresh completes."
        )
    else:
        refresh_ratio = (
            refresh["ivygrep_full_refresh_ms"] / refresh["semble_full_refresh_ms"]
        )
        refresh_verdict = (
            f"Semble full one-file refresh is {refresh_ratio:.1f}x faster; "
            "ivygrep still exposes lexical changes before neural refresh completes."
        )
    faster_indexing_repos = [
        repo
        for repo, metrics in payload["indexing"].items()
        if metrics["ivygrep"]["ready_ms"] < metrics["semble"]["index_ms"]
    ]
    slower_indexing_repos = [
        repo
        for repo, metrics in payload["indexing"].items()
        if metrics["ivygrep"]["ready_ms"] > metrics["semble"]["index_ms"]
    ]
    tied_indexing_repos = [
        repo
        for repo, metrics in payload["indexing"].items()
        if metrics["ivygrep"]["ready_ms"] == metrics["semble"]["index_ms"]
    ]
    if len(faster_indexing_repos) == len(payload["indexing"]):
        indexing_verdict = (
            "ivygrep hybrid-ready indexing is faster on every benchmark "
            "repository in this run."
        )
    elif len(slower_indexing_repos) == len(payload["indexing"]):
        indexing_verdict = (
            "Semble indexing is faster on every benchmark repository in this run."
        )
    elif len(tied_indexing_repos) == len(payload["indexing"]):
        indexing_verdict = (
            "Initial indexing time is tied on every benchmark repository in this run."
        )
    else:
        outcomes = []
        if faster_indexing_repos:
            outcomes.append(f"ivygrep leads on {', '.join(faster_indexing_repos)}")
        if slower_indexing_repos:
            outcomes.append(f"Semble leads on {', '.join(slower_indexing_repos)}")
        if tied_indexing_repos:
            outcomes.append(f"{', '.join(tied_indexing_repos)} tied")
        indexing_verdict = (
            f"Initial indexing is mixed: {'; '.join(outcomes)} in this run."
        )
    category_deltas = {
        category: semble["by_category"][category] - ivy["by_category"][category]
        for category in ivy["by_category"]
    }
    if any(delta > 0 for delta in category_deltas.values()):
        largest_gap_category = max(category_deltas, key=category_deltas.get)
        closest_gap_category = min(
            category_deltas, key=lambda category: abs(category_deltas[category])
        )
        quality_gap_verdict = (
            f"Largest remaining quality gap is {largest_gap_category} retrieval. "
            f"Exact {closest_gap_category} quality is much closer."
        )
    elif all(delta < 0 for delta in category_deltas.values()):
        quality_gap_verdict = "ivygrep leads every measured quality category."
    elif any(delta < 0 for delta in category_deltas.values()):
        quality_gap_verdict = "ivygrep leads or ties every measured quality category."
    else:
        quality_gap_verdict = "Every measured quality category is tied."
    dirty = " + dirty worktree" if payload["ivygrep"]["dirty"] else ""
    return f"""# ivygrep vs Semble

Generated: {payload["generated_at"]}

Semble: `{payload["semble"]["sha"]}` ({payload["semble"]["version"]})
ivygrep: `{payload["ivygrep"]["sha"]}`{dirty}

| Metric | ivygrep | Semble | Winner |
|---|---:|---:|---|
| nDCG@10 | {ivy["ndcg_at_10"]:.3f} | {semble["ndcg_at_10"]:.3f} | {quality_leader} |
| Warm query p50 | {ivy["latency_p50_ms"]:.2f} ms | {semble["latency_p50_ms"]:.2f} ms | {latency_leader} |
| Warm query p95 | {ivy["latency_p95_ms"]:.2f} ms | {semble["latency_p95_ms"]:.2f} ms | {winner(ivy["latency_p95_ms"], semble["latency_p95_ms"], lower_is_better=True)} |
| Mean returned tokens | {ivy["mean_returned_tokens"]:.0f} | {semble["mean_returned_tokens"]:.0f} | {token_leader} |

## Quality by query type

| Category | ivygrep nDCG@10 | Semble nDCG@10 |
|---|---:|---:|
{category_rows}

## Initial indexing

Full hybrid-ready time includes ivygrep lexical, hash, and neural phases.

| Repository | ivygrep | Semble | Semble / ivygrep |
|---|---:|---:|---:|
{indexing_rows}

## One-file refresh

| Metric | ivygrep | Semble |
|---|---:|---:|
| Searchable lexical refresh | {refresh["ivygrep_lexical_refresh_ms"]:.2f} ms | n/a |
| Full hybrid refresh | {refresh["ivygrep_full_refresh_ms"]:.2f} ms | {refresh["semble_full_refresh_ms"]:.2f} ms |

## Verdict

- {quality_latency_verdict}
- {token_verdict}
- {refresh_verdict}
- {indexing_verdict}
- {quality_gap_verdict}

## Notes

- Same pinned repositories, queries, labels, top-k, and nDCG implementation as Semble.
- Semble runs in-process, matching its official benchmark.
- ivygrep runs through its persistent daemon protocol, excluding CLI process startup.
- Timed ivygrep queries disable daemon result-cache replay.
- Model load is reported separately from per-repository indexing.
- ANN construction can move a small number of semantic ranks between runs; compare repeated builds before treating small deltas as signal.
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--semble-repo", type=Path, required=True)
    parser.add_argument("--ivygrep-binary", type=Path, default=Path("target/release/ig"))
    parser.add_argument("--repo", action="append", default=[])
    parser.add_argument("--query-runs", type=int, default=3)
    parser.add_argument("--top-k", type=int, default=10)
    parser.add_argument("--max-queries", type=int)
    parser.add_argument("--skip-sync", action="store_true")
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("docs/benchmarks/ivygrep-vs-semble.json"),
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    semble_repo = args.semble_repo.resolve()
    binary = (root / args.ivygrep_binary).resolve() if not args.ivygrep_binary.is_absolute() else args.ivygrep_binary
    selected_repos = args.repo or list(DEFAULT_REPOS)
    if not binary.is_file():
        raise FileNotFoundError(binary)

    sys.path[:0] = [str(semble_repo), str(semble_repo / "src")]
    from benchmarks.data import grouped_tasks, load_repo_specs, load_tasks
    from model2vec import StaticModel
    from semble import ContentType, SembleIndex
    from semble.index.create import create_index_from_path
    from semble.utils import DEFAULT_MODEL_NAME
    from semble.version import __version__ as semble_version

    modules = {
        "ContentType": ContentType,
        "SembleIndex": SembleIndex,
        "create_index_from_path": create_index_from_path,
        "DEFAULT_MODEL_NAME": DEFAULT_MODEL_NAME,
    }

    env = ivygrep_benchmark_env()

    if not args.skip_sync:
        sync = [
            "uv",
            "run",
            "--project",
            str(semble_repo),
            "python",
            "-m",
            "benchmarks.sync_repos",
        ]
        for repo in selected_repos:
            sync.extend(["--repo", repo])
        run(sync, cwd=semble_repo, env=env, timeout=1800)

    specs = load_repo_specs()
    missing = [repo for repo in selected_repos if not specs[repo].checkout_dir.exists()]
    if missing:
        raise RuntimeError(f"missing Semble benchmark repos: {', '.join(missing)}")
    selected_specs = {repo: specs[repo] for repo in selected_repos}
    tasks = [
        task for task in load_tasks(selected_specs) if task.repo in selected_specs
    ]
    if args.max_queries:
        limited = []
        counts: dict[str, int] = defaultdict(int)
        for task in tasks:
            if counts[task.repo] < args.max_queries:
                limited.append(task)
                counts[task.repo] += 1
        tasks = limited
    tasks_by_repo = grouped_tasks(tasks)

    started = time.perf_counter()
    model = StaticModel.from_pretrained(DEFAULT_MODEL_NAME)
    semble_model_load_ms = (time.perf_counter() - started) * 1000

    records: list[QueryRecord] = []
    indexing: dict[str, Any] = {}
    with tempfile.TemporaryDirectory(prefix="ivygrep-semble-") as temp:
        temp_path = Path(temp)
        corpora_root = temp_path / "corpora"
        benchmark_specs: dict[str, BenchmarkSpec] = {}
        for repo in selected_repos:
            corpus = corpora_root / repo
            shutil.copytree(
                specs[repo].benchmark_dir,
                corpus,
                ignore=shutil.ignore_patterns(".git"),
            )
            benchmark_specs[repo] = BenchmarkSpec(
                name=repo,
                language=specs[repo].language,
                benchmark_dir=corpus,
            )
        persistence_root = temp_path / "semble-indexes"
        persistence_root.mkdir()
        ivy_corpora_root = temp_path / "ivygrep-corpora"
        ivy_specs: dict[str, BenchmarkSpec] = {}
        ivy_home = temp_path / "ivygrep-home"
        ivy_env = env | {"IVYGREP_HOME": str(ivy_home)}

        for repo in selected_repos:
            spec = benchmark_specs[repo]
            semble_records, semble_index, _index = benchmark_semble_repo(
                spec,
                tasks_by_repo[repo],
                model=model,
                modules=modules,
                query_runs=args.query_runs,
                top_k=args.top_k,
                persistence_root=persistence_root,
            )
            records.extend(semble_records)
            ivy_corpus = ivy_corpora_root / repo
            materialize_indexed_files(spec.benchmark_dir, ivy_corpus, _index)
            ivy_specs[repo] = BenchmarkSpec(
                name=repo,
                language=spec.language,
                benchmark_dir=ivy_corpus,
            )
            ivy_index = index_ivygrep_repo(binary, ivy_corpus, ivy_env, force=True)
            indexing[repo] = {"semble": semble_index, "ivygrep": ivy_index}

        daemon = start_daemon(binary, cwd=root, env=ivy_env, home=ivy_home)
        try:
            with IvygrepDaemonClient(ivy_home) as client:
                client.query(
                    ivy_specs[selected_repos[0]].benchmark_dir,
                    "neural model warmup",
                    args.top_k,
                )
                wait_for_neural_model(daemon)
                for repo in selected_repos:
                    records.extend(
                        benchmark_ivygrep_repo(
                            ivy_specs[repo],
                            tasks_by_repo[repo],
                            client=client,
                            query_runs=args.query_runs,
                            top_k=args.top_k,
                        )
                    )
        finally:
            daemon.stop()

        refresh = benchmark_refresh(
            ivy_specs[selected_repos[0]].benchmark_dir,
            binary=binary,
            base_env=env,
            model=model,
            modules=modules,
            work_root=temp_path,
        )

    payload = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "runtime": {
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "logical_cpus": os.cpu_count(),
        },
        "ivygrep": {
            "sha": git_sha(root),
            "dirty": git_dirty(root),
            "binary": public_binary_label(binary, root),
        },
        "semble": {
            "sha": git_sha(semble_repo),
            "version": semble_version,
            "model": DEFAULT_MODEL_NAME,
            "model_load_ms": semble_model_load_ms,
        },
        "repos": selected_repos,
        "queries": len(tasks),
        "query_runs": args.query_runs,
        "top_k": args.top_k,
        "summary": summarize(records),
        "indexing": indexing,
        "refresh": refresh,
        "details": [
            {
                **asdict(record),
                "hits": [persisted_hit(hit) for hit in record.hits],
            }
            for record in records
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    markdown_path = args.output.with_suffix(".md")
    markdown_path.write_text(render_markdown(payload), encoding="utf-8")
    print(json.dumps(payload["summary"], indent=2))
    print(f"Wrote {args.output}")
    print(f"Wrote {markdown_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
