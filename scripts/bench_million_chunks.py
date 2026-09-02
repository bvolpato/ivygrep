#!/usr/bin/env python3
"""Generate and benchmark a deterministic public million-chunk Rust corpus."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import socket
import statistics
import subprocess
import tempfile
import time


SCHEMA_VERSION = 1
DAEMON_PROTOCOL_VERSION = 3
DEFAULT_FILES = 10_000
DEFAULT_CHUNKS_PER_FILE = 100
QUERY_TEMPLATES = (
    "calculate invoice tax for regional order {index}",
    "retry payment after transient gateway failure {index}",
    "parse tenant configuration and validate limits {index}",
    "record structured audit event for request {index}",
    "apply rate limit to authenticated account {index}",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_revision(root: Path) -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()


def binary_identity(binary: Path) -> dict:
    version = subprocess.run(
        [str(binary), "--version"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    return {"version": version, "sha256": sha256_file(binary)}


def runtime_metadata() -> dict:
    cpu_model = platform.processor().strip()
    if not cpu_model and platform.system() == "Linux":
        for line in Path("/proc/cpuinfo").read_text(errors="replace").splitlines():
            if line.lower().startswith(("model name", "hardware")):
                cpu_model = line.split(":", 1)[-1].strip()
                break
    return {
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "cpu_model": cpu_model or None,
        "logical_cpus": os.cpu_count() or 1,
        "physical_memory_bytes": (
            os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
            if hasattr(os, "sysconf")
            else 0
        ),
        "python": platform.python_version(),
        "load_average": list(os.getloadavg()) if hasattr(os, "getloadavg") else None,
    }


def corpus_manifest(files: int, chunks_per_file: int) -> dict:
    return {
        "schema_version": SCHEMA_VERSION,
        "generator": "ivygrep-public-million-rust-v1",
        "license": "CC0-1.0",
        "files": files,
        "chunks_per_file": chunks_per_file,
        "expected_chunks": files * chunks_per_file,
    }


def source_file(file_index: int, chunks_per_file: int) -> str:
    lines = [
        "// SPDX-License-Identifier: CC0-1.0",
        f"//! Deterministic generated module {file_index}.",
        "",
    ]
    for chunk_index in range(chunks_per_file):
        global_index = file_index * chunks_per_file + chunk_index
        template = QUERY_TEMPLATES[global_index % len(QUERY_TEMPLATES)]
        purpose = template.format(index=global_index)
        lines.extend(
            (
                f"/// {purpose}.",
                f"pub fn generated_operation_{global_index:07}(value: u64) -> u64 {{",
                f"    value.wrapping_mul({global_index % 97 + 1}).wrapping_add({global_index})",
                "}",
                "",
            )
        )
    return "\n".join(lines)


def generate_corpus(root: Path, files: int, chunks_per_file: int) -> dict:
    expected = corpus_manifest(files, chunks_per_file)
    manifest_path = root / "corpus-manifest.json"
    if manifest_path.exists():
        existing = json.loads(manifest_path.read_text(encoding="utf-8"))
        if existing == expected:
            ensure_git_boundary(root)
            return existing
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)
    ensure_git_boundary(root)

    def write_file(file_index: int) -> None:
        shard = root / f"shard_{file_index // 1000:02}"
        shard.mkdir(exist_ok=True)
        path = shard / f"module_{file_index:05}.rs"
        path.write_text(source_file(file_index, chunks_per_file), encoding="utf-8")

    with ThreadPoolExecutor(max_workers=min(32, os.cpu_count() or 1)) as executor:
        list(executor.map(write_file, range(files)))
    manifest_path.write_text(
        json.dumps(expected, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return expected


def ensure_git_boundary(root: Path) -> None:
    if (root / ".git" / "HEAD").is_file():
        return
    subprocess.run(
        ["git", "init", "-q"],
        cwd=root,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )


def percentile(values: list[float], quantile: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * quantile) - 1))
    return ordered[index]


def directory_size(root: Path) -> int:
    total = 0
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        try:
            total += path.stat().st_size
        except FileNotFoundError:
            pass
    return total


def timed(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    monitor_path: Path | None = None,
) -> tuple[subprocess.CompletedProcess[str], dict]:
    with (
        tempfile.NamedTemporaryFile() as stdout,
        tempfile.NamedTemporaryFile() as stderr,
    ):
        started = time.perf_counter()
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=env,
            stdout=stdout,
            stderr=stderr,
        )
        peak_rss_bytes = 0
        read_bytes = 0
        write_bytes = 0
        cpu_seconds = 0.0
        peak_disk_bytes = 0
        last_disk_sample = 0.0
        clock_ticks = os.sysconf("SC_CLK_TCK")
        while process.poll() is None:
            try:
                status = Path(f"/proc/{process.pid}/status").read_text()
                for line in status.splitlines():
                    if line.startswith("VmRSS:"):
                        peak_rss_bytes = max(
                            peak_rss_bytes, int(line.split()[1]) * 1024
                        )
                        break
                io = Path(f"/proc/{process.pid}/io").read_text()
                counters = {
                    line.split(":", 1)[0]: int(line.split(":", 1)[1])
                    for line in io.splitlines()
                }
                read_bytes = max(read_bytes, counters.get("read_bytes", 0))
                write_bytes = max(write_bytes, counters.get("write_bytes", 0))
                stat = Path(f"/proc/{process.pid}/stat").read_text().split()
                cpu_seconds = max(
                    cpu_seconds, (int(stat[13]) + int(stat[14])) / clock_ticks
                )
            except (FileNotFoundError, PermissionError, ProcessLookupError):
                pass
            now = time.monotonic()
            if monitor_path is not None and now - last_disk_sample >= 0.5:
                peak_disk_bytes = max(
                    peak_disk_bytes,
                    directory_size(monitor_path),
                )
                last_disk_sample = now
            time.sleep(0.05)
        return_code = process.wait()
        wall_ms = (time.perf_counter() - started) * 1000.0
        if monitor_path is not None:
            peak_disk_bytes = max(peak_disk_bytes, directory_size(monitor_path))
        stdout.seek(0)
        stderr.seek(0)
        result = subprocess.CompletedProcess(
            command,
            return_code,
            stdout.read().decode(errors="replace"),
            stderr.read().decode(errors="replace"),
        )
        if return_code != 0:
            raise subprocess.CalledProcessError(
                return_code,
                command,
                output=result.stdout,
                stderr=result.stderr,
            )
    return result, {
        "wall_ms": wall_ms,
        "peak_rss_bytes": peak_rss_bytes,
        "filesystem_read_bytes": read_bytes,
        "filesystem_write_bytes": write_bytes,
        "cpu_ms": cpu_seconds * 1000.0,
        "peak_disk_bytes": peak_disk_bytes,
    }


def daemon_endpoint(home: Path) -> Path:
    return home / ("daemon.port" if os.name == "nt" else "daemon.sock")


def start_daemon(
    binary: Path,
    cwd: Path,
    env: dict[str, str],
    home: Path,
    log_name: str = "million-benchmark-daemon.log",
):
    home.mkdir(parents=True, exist_ok=True)
    endpoint = daemon_endpoint(home)
    endpoint.unlink(missing_ok=True)
    log_path = home / log_name
    log = log_path.open("wb")
    process = subprocess.Popen(
        [str(binary), "--daemon"],
        cwd=cwd,
        env=env,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=os.name != "nt",
    )
    deadline = time.monotonic() + 15.0
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError("daemon exited before becoming ready")
        if endpoint.exists():
            return process, log, log_path
        time.sleep(0.05)
    raise TimeoutError("daemon did not become ready")


def stop_daemon(process: subprocess.Popen, log) -> None:
    if process.poll() is None:
        if os.name == "nt":
            process.terminate()
        else:
            os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            if os.name == "nt":
                process.kill()
            else:
                os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)
    log.close()


def run_query(
    binary: Path,
    corpus: Path,
    query: str,
    env: dict[str, str],
    extra: list[str] | None = None,
    force_neural: bool = False,
) -> dict:
    command = [
        str(binary),
        "--force-neural" if force_neural else "--hash",
        "--json",
        "-n",
        "20",
        *(extra or []),
        "--",
        query,
        str(corpus),
    ]
    started = time.perf_counter()
    result = subprocess.run(
        command,
        cwd=corpus,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    output = json.loads(result.stdout)
    hits = [hit for item in output for hit in item.get("hits", [])]
    return {
        "elapsed_ms": elapsed_ms,
        "hit_count": len(hits),
        "paths": [item["file_path"] for item in output],
        "neural_executed": neural_execution(hits, force_neural),
    }


def neural_execution(hits: list[dict], required: bool) -> bool:
    executed = any(hit.get("neural_executed") is True for hit in hits)
    if required and not executed:
        raise RuntimeError("forced-neural benchmark query did not report neural execution")
    return executed


class DaemonClient:
    def __init__(self, home: Path, corpus: Path, force_neural: bool = False):
        self.home = home
        self.corpus = corpus
        self.force_neural = force_neural
        self.protocol_version = DAEMON_PROTOCOL_VERSION
        self.connection: socket.socket | None = None
        self.reader = None

    def __enter__(self):
        self._connect()
        return self

    def _connect(self) -> None:
        if os.name == "nt":
            port = int((self.home / "daemon.port").read_text(encoding="utf-8").strip())
            self.connection = socket.create_connection(("127.0.0.1", port), timeout=120)
        else:
            self.connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.connection.settimeout(120)
            self.connection.connect(str(self.home / "daemon.sock"))
        self.reader = self.connection.makefile("rb")

    def __exit__(self, _exc_type, _exc_value, _traceback):
        self._close()

    def _close(self) -> None:
        if self.reader is not None:
            self.reader.close()
            self.reader = None
        if self.connection is not None:
            self.connection.close()
            self.connection = None

    def _send(self, request: dict) -> tuple[dict, float]:
        payload = json.dumps(request).encode() + b"\n"
        response_bytes = b""
        for attempt in range(2):
            if self.connection is None or self.reader is None:
                self._connect()
            # Time each attempt independently so reconnecting an older
            # one-request daemon is not charged to the successful query.
            started = time.perf_counter()
            try:
                self.connection.sendall(payload)
                response_bytes = self.reader.readline()
                if response_bytes:
                    break
            except (BrokenPipeError, ConnectionResetError):
                pass
            if attempt == 0:
                self._close()
                self._connect()
        if not response_bytes:
            raise RuntimeError("daemon closed the connection without a response")
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        return json.loads(response_bytes), elapsed_ms

    @staticmethod
    def _expected_protocol_version(message: str) -> int | None:
        marker = "; expected "
        if (
            not message.startswith("unsupported daemon protocol version ")
            or marker not in message
        ):
            return None
        try:
            return int(message.rsplit(marker, 1)[1])
        except ValueError:
            return None

    def query(self, query: str, type_filter: str | None = None) -> dict:
        for protocol_attempt in range(2):
            request = {
                "protocol_version": self.protocol_version,
                "type": "search",
                "path": str(self.corpus),
                "query": query,
                "limit": 20,
                "context": 2,
                "type_filter": type_filter,
                "include_globs": [],
                "exclude_globs": [],
                "scope_path": None,
                "scope_is_file": False,
                "skip_gitignore": False,
            }
            if self.force_neural:
                request["force_neural"] = True
            response, elapsed_ms = self._send(request)
            if response.get("type") != "error":
                break
            message = response.get("message", "daemon search failed")
            expected = self._expected_protocol_version(message)
            if protocol_attempt == 0 and expected is not None:
                self.protocol_version = expected
                continue
            raise RuntimeError(message)
        else:
            raise RuntimeError("daemon protocol negotiation failed")
        hits = response.get("hits", [])
        paths = list(dict.fromkeys(hit["file_path"] for hit in hits))
        return {
            "elapsed_ms": elapsed_ms,
            "hit_count": len(hits),
            "paths": paths,
            "neural_executed": neural_execution(hits, self.force_neural),
        }


def run_daemon_query(
    home: Path,
    corpus: Path,
    query: str,
    type_filter: str | None = None,
    force_neural: bool = False,
) -> dict:
    with DaemonClient(home, corpus, force_neural) as client:
        return client.query(query, type_filter)


def query_cases(
    samples: int, total_chunks: int, chunks_per_file: int
) -> list[tuple[str, str]]:
    cases = []
    for sample_index in range(samples):
        chunk_index = sample_index * 9973 % total_chunks
        query = QUERY_TEMPLATES[chunk_index % len(QUERY_TEMPLATES)].format(
            index=chunk_index
        )
        file_index = chunk_index // chunks_per_file
        expected = f"shard_{file_index // 1000:02}/module_{file_index:05}.rs"
        cases.append((query, expected))
    return cases


def summarize_queries(records: list[dict]) -> dict:
    values = [record["elapsed_ms"] for record in records]
    expected_found = sum(
        record["expected_path"] in record["paths"] for record in records
    )
    reciprocal_ranks = []
    for record in records:
        try:
            rank = record["paths"].index(record["expected_path"]) + 1
        except ValueError:
            rank = 0
        reciprocal_ranks.append(1.0 / rank if rank else 0.0)
    return {
        "samples": len(values),
        "median_ms": statistics.median(values),
        "p95_ms": percentile(values, 0.95),
        "minimum_ms": min(values),
        "maximum_ms": max(values),
        "latency_samples_ms": values,
        "mean_hits": statistics.mean(record["hit_count"] for record in records),
        "expected_recall_at_20": expected_found / len(records),
        "expected_mrr_at_20": statistics.mean(reciprocal_ranks),
        "neural_queries_executed": sum(record.get("neural_executed", False) for record in records),
    }


def parse_trace_phases(log_path: Path) -> dict:
    duration_pattern = re.compile(
        r"\b(open_tantivy|literal_pass|lexical|semantic|fuse_rrf|to_hit)="
        r"([0-9.]+)(ns|µs|ms|s)"
    )

    def milliseconds(value: str, unit: str) -> float:
        scale = {"ns": 0.000001, "µs": 0.001, "ms": 1.0, "s": 1000.0}
        return float(value) * scale[unit]

    searches = []
    current: dict[str, float] = {}
    for line in log_path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = duration_pattern.search(line)
        if match is None:
            continue
        phase, value, unit = match.groups()
        if phase == "open_tantivy":
            current = {}
        current[phase] = milliseconds(value, unit)
        if phase == "to_hit":
            searches.append(current)
            current = {}

    searches = searches[1:]
    direct = {
        "open_context": [],
        "literal": [],
        "lexical": [],
        "semantic": [],
        "fusion": [],
        "presentation": [],
        "total": [],
    }
    for search in searches:
        opened = search.get("open_tantivy", 0.0)
        literal = search.get("literal_pass", opened)
        lexical = search.get("lexical", literal)
        semantic = search.get("semantic", lexical)
        fusion = search.get("fuse_rrf", semantic)
        total = search["to_hit"]
        direct["open_context"].append(opened)
        direct["literal"].append(max(0.0, literal - opened))
        direct["lexical"].append(max(0.0, lexical - literal))
        direct["semantic"].append(max(0.0, semantic - lexical))
        direct["fusion"].append(max(0.0, fusion - semantic))
        direct["presentation"].append(max(0.0, total - fusion))
        direct["total"].append(total)

    return {
        phase: {
            "samples": len(values),
            "median_ms": statistics.median(values),
            "p95_ms": percentile(values, 0.95),
        }
        for phase, values in direct.items()
        if values
    }


def profile_query_phases(
    binary: Path,
    corpus: Path,
    env: dict[str, str],
    cases: list[tuple[str, str]],
    force_neural: bool = False,
) -> dict:
    trace_env = {**env, "RUST_LOG": "ivygrep::search=trace"}
    daemon, log, log_path = start_daemon(
        binary,
        corpus,
        trace_env,
        Path(env["IVYGREP_HOME"]),
        "million-benchmark-trace.log",
    )
    try:
        with DaemonClient(Path(env["IVYGREP_HOME"]), corpus, force_neural) as client:
            client.query("warmup generated operation")
            for query, _ in cases[: min(20, len(cases))]:
                client.query(query)
    finally:
        stop_daemon(daemon, log)
    return parse_trace_phases(log_path)


def query_suite(
    binary: Path,
    corpus: Path,
    env: dict[str, str],
    samples: int,
    total_chunks: int,
    chunks_per_file: int,
    force_neural: bool = False,
) -> dict:
    cases = query_cases(samples, total_chunks, chunks_per_file)

    def measure(
        client: DaemonClient,
        case: tuple[str, str],
        type_filter: str | None = None,
    ) -> dict:
        query, expected_path = case
        return {
            **client.query(query, type_filter),
            "expected_path": expected_path,
        }

    process_cold = [
        {
            **run_query(
                binary,
                corpus,
                query,
                {**env, "IVYGREP_NO_AUTOSPAWN": "1"},
                force_neural=force_neural,
            ),
            "expected_path": expected_path,
        }
        for query, expected_path in cases[: min(10, samples)]
    ]
    daemon, log, _ = start_daemon(binary, corpus, env, Path(env["IVYGREP_HOME"]))
    try:
        with DaemonClient(Path(env["IVYGREP_HOME"]), corpus, force_neural) as client:
            client.query("warmup generated operation")
            distinct = [measure(client, case) for case in cases]
            replay = [measure(client, cases[0]) for _ in range(samples)]
            filtered = [measure(client, case, "rust") for case in cases]
        cli_warm = [
            {
                **run_query(binary, corpus, query, env, force_neural=force_neural),
                "expected_path": expected_path,
            }
            for query, expected_path in cases[: min(20, samples)]
        ]

        started = time.perf_counter()

        def concurrent_measure(case: tuple[str, str]) -> dict:
            query, expected_path = case
            return {
                **run_daemon_query(
                    Path(env["IVYGREP_HOME"]),
                    corpus,
                    query,
                    force_neural=force_neural,
                ),
                "expected_path": expected_path,
            }

        with ThreadPoolExecutor(max_workers=8) as executor:
            concurrent = list(
                executor.map(concurrent_measure, cases[: min(64, samples)])
            )
        concurrent_wall_ms = (time.perf_counter() - started) * 1000.0
    finally:
        stop_daemon(daemon, log)

    return {
        "process_cold": summarize_queries(process_cold),
        "warm_distinct": summarize_queries(distinct),
        "cache_replay": summarize_queries(replay),
        "filtered": summarize_queries(filtered),
        "cli_warm_distinct": summarize_queries(cli_warm),
        "concurrent": {
            **summarize_queries(concurrent),
            "workers": 8,
            "wall_ms": concurrent_wall_ms,
            "queries_per_second": len(concurrent) / (concurrent_wall_ms / 1000.0),
        },
        "phase_timings": profile_query_phases(binary, corpus, env, cases, force_neural),
    }


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--corpus",
        type=Path,
        default=Path(os.environ.get("TMPDIR", "/tmp")) / "ivygrep-public-million",
    )
    parser.add_argument(
        "--home",
        type=Path,
        default=Path(os.environ.get("TMPDIR", "/tmp")) / "ivygrep-public-million-home",
    )
    parser.add_argument(
        "--binary", type=Path, default=root / "target" / "release" / "ig"
    )
    parser.add_argument("--files", type=int, default=DEFAULT_FILES)
    parser.add_argument("--chunks-per-file", type=int, default=DEFAULT_CHUNKS_PER_FILE)
    parser.add_argument("--query-samples", type=int, default=100)
    parser.add_argument("--reuse-index", action="store_true")
    parser.add_argument(
        "--enhance-hash",
        action="store_true",
        help="measure foreground hash enhancement before running queries",
    )
    parser.add_argument(
        "--enhance-neural",
        action="store_true",
        help="measure foreground hash + neural enhancement, then require neural query execution",
    )
    parser.add_argument(
        "--source-commit",
        default=None,
        help="commit that produced --binary (defaults to the current checkout)",
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    corpus = args.corpus.resolve()
    home = args.home.resolve()
    binary = args.binary.resolve()
    manifest = generate_corpus(corpus, args.files, args.chunks_per_file)
    env = {
        **os.environ,
        "IVYGREP_HOME": str(home),
        "IVYGREP_NO_AUTOSPAWN": "1",
        "IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT": "1",
        "IVYGREP_ENHANCE_MAX_LOAD_RATIO": "0",
        "IVYGREP_RERANKER": "learned",
    }
    index_metrics = None
    if not args.reuse_index:
        shutil.rmtree(home, ignore_errors=True)
        home.mkdir(parents=True)
        result, index_metrics = timed(
            [
                str(binary),
                "--add",
                str(corpus),
                "--force",
                "--json",
                "--no-watch",
                "--hash",
            ],
            root,
            env,
            monitor_path=home,
        )
        index_summary = json.loads(result.stdout)
    else:
        index_summary = {}
    readiness_metrics = []
    if args.enhance_hash:
        _, metrics = timed(
            [str(binary), "--enhance-hash-internal", str(corpus)],
            root,
            env,
            monitor_path=home,
        )
        readiness_metrics.append({"phase": "hash_enhancement", "metrics": metrics})
    if args.enhance_neural:
        _, metrics = timed(
            [str(binary), "--enhance-internal", str(corpus)],
            root,
            env,
            monitor_path=home,
        )
        readiness_metrics.append(
            {"phase": "hash_and_neural_enhancement", "metrics": metrics}
        )
    status = json.loads(
        subprocess.run(
            [str(binary), "--status", "--json"],
            cwd=root,
            env=env,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout
    )
    workspace = next(item for item in status if Path(item["root"]) == corpus)
    if workspace["chunk_count"] < manifest["expected_chunks"]:
        raise RuntimeError(
            f"expected at least {manifest['expected_chunks']} chunks, "
            f"got {workspace['chunk_count']}"
        )
    report = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "ivygrep_commit": args.source_commit or git_revision(root),
        "harness_sha256": {
            path.name: sha256_file(path)
            for path in (
                root / "scripts" / "bench_million_chunks.py",
                root / "scripts" / "compare_million_benchmarks.py",
            )
        },
        "binary": binary_identity(binary),
        "runtime": runtime_metadata(),
        "corpus": {
            **manifest,
            "manifest_sha256": sha256_file(corpus / "corpus-manifest.json"),
        },
        "index": {
            "metrics": index_metrics,
            "summary": index_summary,
            "chunk_count": workspace["chunk_count"],
            "file_count": workspace["file_count"],
            "size_bytes": workspace["index_size_bytes"],
            "components": workspace.get("index_components", {}),
            "chunks_per_second": (
                workspace["chunk_count"] / (index_metrics["wall_ms"] / 1000.0)
                if index_metrics
                else None
            ),
        },
        "readiness": {
            "phases": readiness_metrics,
            "total_wall_ms": (index_metrics["wall_ms"] if index_metrics else 0.0)
            + sum(phase["metrics"]["wall_ms"] for phase in readiness_metrics),
            "hash_ready": workspace.get("hash_vector_count", 0)
            >= workspace.get("vector_key_count", workspace["chunk_count"])
            > 0,
            "neural_ready": bool(workspace.get("has_neural_vectors")),
        },
        "neural_execution_required": args.enhance_neural,
        "queries": query_suite(
            binary,
            corpus,
            {**env, "IVYGREP_NO_AUTOSPAWN": "0"},
            args.query_samples,
            manifest["expected_chunks"],
            manifest["chunks_per_file"],
            force_neural=args.enhance_neural,
        ),
    }
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "chunks": workspace["chunk_count"],
                "index_ms": index_metrics["wall_ms"] if index_metrics else None,
                "warm_distinct_p95_ms": report["queries"]["warm_distinct"]["p95_ms"],
                "output": args.output.name,
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
