#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Evaluate ivygrep on BEIR/CoIR-style retrieval datasets."""

from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import platform
import signal
import subprocess
import tempfile
import time

import public_retrieval_contracts as contracts

try:
    import resource
except ImportError:  # pragma: no cover - unavailable on Windows
    resource = None

RERANK_CONTEXT_LINES = 2
RERANK_PREVIEW_HITS = 3
RERANK_PREVIEW_BYTES = 12_000


def candidate_preview(hits: list[dict]) -> str:
    """Match the runtime reranker's first-three-hit UTF-8 byte budget."""
    preview = ""
    for hit in hits[:RERANK_PREVIEW_HITS]:
        if preview:
            preview = (
                (preview + "\n")
                .encode()[:RERANK_PREVIEW_BYTES]
                .decode("utf-8", errors="ignore")
            )
        remaining = RERANK_PREVIEW_BYTES - len(preview.encode())
        preview += (
            str(hit.get("preview", ""))
            .encode()[:remaining]
            .decode("utf-8", errors="ignore")
        )
    return preview


def candidate_trace_contract(query_expansion: str, reranker_mode: str | None) -> dict:
    return {
        "schema_version": 1,
        "stage": "grouped-runtime-output-not-training",
        "score_semantics": "native_file_total",
        "query_expansion": query_expansion,
        "reranker_mode": reranker_mode or "unknown",
        "context_lines": RERANK_CONTEXT_LINES,
        "preview_max_hits": RERANK_PREVIEW_HITS,
        "preview_max_bytes": RERANK_PREVIEW_BYTES,
    }


def load_jsonl(path: Path) -> list[dict]:
    with path.open(encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def load_qrels(path: Path) -> dict[str, dict[str, int]]:
    qrels: dict[str, dict[str, int]] = {}
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            fields = line.rstrip("\n").split("\t")
            if not fields or fields[0] in {"query-id", "query_id"}:
                continue
            if len(fields) < 3:
                raise ValueError(f"invalid qrels row: {line.rstrip()}")
            query_id, corpus_id, score = fields[:3]
            qrels.setdefault(query_id, {})[corpus_id] = int(score)
    return qrels


def selected_queries(queries: list[dict], query_ids: list[str]) -> list[dict]:
    if not query_ids:
        return queries

    required = set(query_ids)
    selected = [query for query in queries if str(query["_id"]) in required]
    missing = required - {str(query["_id"]) for query in selected}
    if missing:
        raise ValueError("requested query IDs are missing: " + ", ".join(sorted(missing)))
    return selected


def score_query(ranked: list[str], judgments: dict[str, int]) -> dict[str, float]:
    relevant = {doc_id for doc_id, grade in judgments.items() if grade > 0}

    def dcg(ids: list[str], cutoff: int) -> float:
        return sum(
            (2 ** judgments.get(doc_id, 0) - 1) / math.log2(rank + 2)
            for rank, doc_id in enumerate(ids[:cutoff])
        )

    ideal_grades = sorted(judgments.values(), reverse=True)
    ideal_dcg = sum(
        (2**grade - 1) / math.log2(rank + 2)
        for rank, grade in enumerate(ideal_grades[:10])
    )
    first_relevant = next(
        (
            rank
            for rank, doc_id in enumerate(ranked[:10], start=1)
            if doc_id in relevant
        ),
        None,
    )

    def recall(cutoff: int) -> float:
        if not relevant:
            return 0.0
        return sum(doc_id in relevant for doc_id in ranked[:cutoff]) / len(relevant)

    def exact(cutoff: int) -> float:
        return float(bool(relevant) and relevant.issubset(ranked[:cutoff]))

    return {
        "ndcg_at_10": dcg(ranked, 10) / ideal_dcg if ideal_dcg else 0.0,
        "mrr_at_10": 1.0 / first_relevant if first_relevant else 0.0,
        "precision_at_5": sum(doc_id in relevant for doc_id in ranked[:5]) / 5.0,
        "recall_at_5": recall(5),
        "recall_at_10": recall(10),
        "recall_at_20": recall(20),
        "exact_at_5": exact(5),
        "exact_at_10": exact(10),
        "exact_at_20": exact(20),
    }


def aggregate(scores: list[dict[str, float]]) -> dict[str, float]:
    if not scores:
        return {
            "ndcg_at_10": 0.0,
            "mrr_at_10": 0.0,
            "precision_at_5": 0.0,
            "recall_at_5": 0.0,
            "recall_at_10": 0.0,
            "recall_at_20": 0.0,
            "exact_at_5": 0.0,
            "exact_at_10": 0.0,
            "exact_at_20": 0.0,
        }
    return {key: sum(score[key] for score in scores) / len(scores) for key in scores[0]}


def percentile(values: list[float], percentile_value: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = math.ceil(percentile_value * len(ordered)) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_provenance(dataset: Path) -> dict | None:
    path = dataset / "provenance.json"
    if not path.exists():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def runtime_metadata() -> dict:
    cpu_model = platform.processor().strip()
    if not cpu_model and platform.system() == "Linux":
        for line in (
            Path("/proc/cpuinfo")
            .read_text(encoding="utf-8", errors="replace")
            .splitlines()
        ):
            if line.lower().startswith(("model name", "hardware")):
                cpu_model = line.split(":", 1)[-1].strip()
                break
    physical_memory_bytes = None
    if hasattr(os, "sysconf"):
        try:
            physical_memory_bytes = os.sysconf("SC_PAGE_SIZE") * os.sysconf(
                "SC_PHYS_PAGES"
            )
        except (OSError, ValueError):
            pass
    return {
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "cpu_model": cpu_model or None,
        "logical_cpus": os.cpu_count(),
        "physical_memory_bytes": physical_memory_bytes,
        "python": platform.python_version(),
    }


def expected_execution_request(
    dataset: Path,
    binary_sha256: str,
    mode: str,
    max_query_chars: int | None,
    *,
    runtime: dict | None = None,
    harness: dict | None = None,
    dataset_content: dict | None = None,
    **options,
) -> dict:
    return contracts.execution_request(
        dataset,
        binary_sha256,
        mode,
        {**options, "max_query_chars": max_query_chars},
        os.environ,
        runtime if runtime is not None else runtime_metadata(),
        harness
        if harness is not None
        else contracts.execution_harness(Path(__file__).resolve().parents[1]),
        dataset_content=dataset_content,
    )


def source_revision(override: str | None) -> str:
    if override:
        return override
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=Path(__file__).resolve().parents[1],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    ).stdout.strip()


def peak_child_rss_bytes() -> int | None:
    if resource is None:
        return None
    maximum = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    if platform.system() == "Darwin":
        return int(maximum)
    return int(maximum * 1024)


def binary_identity(binary: Path) -> dict:
    completed = subprocess.run(
        [str(binary), "--version"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    return {
        "version": completed.stdout.strip(),
        "sha256": sha256_file(binary),
    }


def materialize_corpus(dataset: Path, repo: Path) -> dict[str, str]:
    repo.mkdir(parents=True, exist_ok=True)
    (repo / ".git").mkdir(exist_ok=True)
    path_to_id: dict[str, str] = {}
    for position, document in enumerate(load_jsonl(dataset / "corpus.jsonl")):
        doc_id = str(document["_id"])
        relative = document_relative_path(document, position)
        relative_path = Path(relative)
        target = repo / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        title = document.get("title") or ""
        text = document.get("text") or ""
        target.write_text(f"{title}\n{text}".lstrip(), encoding="utf-8")
        path_to_id[relative] = doc_id
    return path_to_id


def document_relative_path(document: dict, position: int) -> str:
    doc_id = str(document["_id"])
    relative = (document.get("metadata") or {}).get(
        "path"
    ) or f"documents/{position:06d}-{doc_id}.txt"
    path = Path(str(relative))
    if path.is_absolute() or ".." in path.parts:
        path = Path("documents") / f"{position:06d}-{doc_id}.txt"
    return path.as_posix()


def corpus_path_map(dataset: Path) -> dict[str, str]:
    return {
        document_relative_path(document, position): str(document["_id"])
        for position, document in enumerate(load_jsonl(dataset / "corpus.jsonl"))
    }


def is_support_path(path: str) -> bool:
    normalized = "/" + path.lower().replace("\\", "/")
    name = Path(normalized).name
    return (
        any(
            marker in normalized
            for marker in (
                "/doc/",
                "/docs/",
                "/example/",
                "/examples/",
                "/sample/",
                "/samples/",
                "/test/",
                "/tests/",
                "/spec/",
                "/specs/",
            )
        )
        or name.startswith(("readme", "test_", "example_"))
        or any(marker in name for marker in ("_test.", ".test.", "_spec.", ".spec."))
    )


def query_targets_support(text: str) -> bool:
    terms = {
        term.strip(".,:;!?()[]{}\"'").lower()
        for term in text.split()
        if term.strip(".,:;!?()[]{}\"'")
    }
    return bool(
        terms
        & {
            "doc",
            "docs",
            "documentation",
            "readme",
            "test",
            "tests",
            "testing",
            "example",
            "examples",
            "sample",
            "samples",
        }
    )


def run_json(
    command: list[str], cwd: Path, env: dict[str, str]
) -> tuple[object, float]:
    started = time.perf_counter()
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    if completed.returncode != 0:
        stderr = completed.stderr.strip()
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(command)}"
            + (f"\n{stderr}" if stderr else "")
        )
    return json.loads(completed.stdout), elapsed_ms


def run_captured_query(
    command: list[str], cwd: Path, env: dict[str, str], query: str, receipt: Path
) -> tuple[list[dict], float, dict]:
    """Collect one local process's opt-in native record, preserving raw failures."""
    home = Path(env["IVYGREP_HOME"])
    if daemon_endpoint_path(home).exists():
        raise ValueError(
            "native capture requires a fresh local process, not an existing daemon"
        )
    receipt.parent.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=os.name != "nt",
    )
    try:
        receipt.with_suffix(".command.json").write_text(
            json.dumps(
                {
                    "argv": command,
                    "process_id": process.pid,
                    "query": query,
                    "cwd": str(cwd),
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        stdout, stderr = process.communicate(timeout=600)
    except BaseException:
        stop_process(process)
        stdout, stderr = process.communicate()
        receipt.with_suffix(".stdout.json").write_bytes(stdout)
        receipt.with_suffix(".stderr.log").write_bytes(stderr)
        raise
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    receipt.with_suffix(".stdout.json").write_bytes(stdout)
    receipt.with_suffix(".stderr.log").write_bytes(stderr)
    receipt.with_suffix(".exit.json").write_text(
        json.dumps({"process_id": process.pid, "returncode": process.returncode}) + "\n"
    )
    if process.returncode:
        raise RuntimeError(
            f"native capture query failed ({process.returncode}); see {receipt}.stderr.log"
        )
    if daemon_endpoint_path(home).exists():
        raise ValueError(
            "native capture unexpectedly created or used a daemon endpoint"
        )
    record = contracts.parse_native_capture(stderr.decode("utf-8"), query, process.pid)
    output = json.loads(stdout.decode("utf-8"))
    if not isinstance(output, list):
        raise ValueError(
            "native capture query did not return the normal grouped JSON array"
        )
    capture = {
        "record": record,
        "process_id": process.pid,
        "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
        "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
        "receipt_name": receipt.name,
    }
    return output, elapsed_ms, capture


def captured_document_ids(
    record: dict, repo: Path, path_to_id: dict[str, str]
) -> list[str]:
    document_ids = []
    base = str(repo).replace("\\", "/").removeprefix("//?/").rstrip("/")
    for candidate in record["candidates"]:
        value = candidate["file_path"].replace("\\", "/").removeprefix("//?/")
        path = PurePosixPath(value)
        if ".." in path.parts:
            raise ValueError("native capture candidate escapes the indexed corpus")
        if value.startswith(base + "/"):
            relative = value[len(base) + 1 :]
        elif path.is_absolute() or (len(value) >= 3 and value[1:3] == ":/"):
            raise ValueError("native capture candidate escapes the indexed corpus")
        else:
            relative = path.as_posix()
        if relative not in path_to_id:
            raise ValueError(
                "native capture candidate is not mapped to a corpus document"
            )
        document_ids.append(path_to_id[relative])
    if len(document_ids) != len(set(document_ids)):
        raise ValueError("native capture has ambiguous duplicate corpus document IDs")
    return document_ids


def run_search_commands(
    commands: list[list[str]],
    cwd: Path,
    env: dict[str, str],
    max_workers: int,
) -> tuple[list[object], float]:
    started = time.perf_counter()
    if len(commands) == 1 or max_workers == 1:
        outputs = [run_json(command, cwd, env)[0] for command in commands]
    else:
        with ThreadPoolExecutor(
            max_workers=min(max_workers, len(commands))
        ) as executor:
            outputs = list(
                executor.map(
                    lambda command: run_json(command, cwd, env)[0],
                    commands,
                )
            )
    return outputs, (time.perf_counter() - started) * 1000.0


def query_args(mode: str) -> list[str]:
    if mode == "lexical":
        return ["--lexical-only"]
    if mode == "hash":
        return ["--hash"]
    if mode == "hybrid":
        return []
    if mode == "blended":
        return []
    if mode == "neural":
        return ["--force-neural"]
    raise ValueError(f"unsupported mode {mode}")


def query_text(query: dict, max_query_chars: int | None) -> str:
    text = str(query.get("text") or query.get("query") or "")
    if max_query_chars is not None:
        return text[:max_query_chars]
    return text


def expanded_query_texts(
    text: str,
    profile: str,
    probe_query_chars: int | None = None,
) -> list[str]:
    if profile == "none":
        return [text]
    probe_text = text[:probe_query_chars] if probe_query_chars is not None else text
    facets = {
        "memory-context": (
            "Personal context, prior preferences, constraints, and commitments "
            f"relevant to: {probe_text}"
        ),
        "memory-history": (
            "Past events, current plans, dependencies, and unresolved decisions "
            f"relevant to: {probe_text}"
        ),
        "memory-action": (
            f"Information needed before deciding, responding, or acting on: {probe_text}"
        ),
    }
    if profile in facets:
        return [text, facets[profile]]
    combinations = {
        "memory-context-history": ("memory-context", "memory-history"),
        "memory-context-action": ("memory-context", "memory-action"),
        "memory-history-action": ("memory-history", "memory-action"),
        "memory-facets": tuple(facets),
    }
    if profile in combinations:
        return [text, *(facets[name] for name in combinations[profile])]
    if profile == "retrieval-facets":
        return [
            text,
            f"Relevant entities, facts, attributes, and relationships for: {probe_text}",
            f"Chronology, causes, effects, changes, and dependencies for: {probe_text}",
            f"Constraints, tradeoffs, decisions, and next actions for: {probe_text}",
        ]
    raise ValueError(f"unsupported query expansion profile {profile}")


def fuse_search_outputs(
    outputs: list[list[dict]],
    *,
    rrf_k: float = 60.0,
    original_weight: float = 1.0,
) -> list[dict]:
    if len(outputs) == 1:
        return [dict(item) for item in outputs[0]]
    scores: dict[str, float] = {}
    selected: dict[str, dict] = {}
    for output_index, output in enumerate(outputs):
        weight = original_weight if output_index == 0 else 1.0
        for rank, item in enumerate(output):
            path = str(item.get("file_path", ""))
            if not path:
                continue
            scores[path] = scores.get(path, 0.0) + weight / (rrf_k + rank + 1.0)
            selected.setdefault(path, item)
    ranked = sorted(scores, key=lambda path: (-scores[path], path))
    fused = []
    for path in ranked:
        item = dict(selected[path])
        item["fusion_score"] = scores[path]
        fused.append(item)
    return fused


def query_scope(query: dict, repo: Path) -> str | None:
    raw_scope = (query.get("metadata") or {}).get("scope")
    if raw_scope is None:
        return None
    relative = Path(str(raw_scope))
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError(f"unsafe query scope: {raw_scope}")
    scope = repo / relative
    if not scope.is_dir():
        raise ValueError(f"query scope does not exist: {raw_scope}")
    return f"{relative.as_posix().rstrip('/')}/**"


def query_exclude_globs(query: dict, repo: Path) -> list[str]:
    raw_globs = (query.get("metadata") or {}).get("exclude_globs") or []
    excludes = []
    for raw_glob in raw_globs:
        relative = Path(str(raw_glob))
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"unsafe query exclude glob: {raw_glob}")
        if not (repo / relative).is_file():
            raise ValueError(f"query exclude path does not exist: {raw_glob}")
        excludes.append(relative.as_posix())
    return excludes


def search_command(
    binary: Path,
    mode: str,
    limit: int,
    query: str,
    scope: str | None = None,
    exclude_globs: list[str] | None = None,
    disable_memory_expansion: bool = False,
) -> list[str]:
    command = [
        str(binary),
        "--json",
        "--context",
        str(RERANK_CONTEXT_LINES),
        "-n",
        str(limit),
        *query_args(mode),
    ]
    if scope is not None:
        command.extend(["--include", scope])
    for exclude_glob in exclude_globs or []:
        command.extend(["--exclude", exclude_glob])
    if disable_memory_expansion:
        command.append("--no-memory-expansion-internal")
    command.extend(["--", query])
    return command


def daemon_endpoint_path(home: Path) -> Path:
    return home / ("daemon.port" if os.name == "nt" else "daemon.sock")


def warm_query_path(mode: str) -> str:
    return "local-process" if mode == "lexical" else "daemon"


def neural_execution_status(hits: list[dict]) -> bool | None:
    if not hits:
        return None
    return any(hit.get("neural_executed") is True for hit in hits)


def process_cold_queries(mode: str, queries: list[dict]) -> list[dict]:
    # Neural process startup includes loading model weights. Measuring that for
    # every quality query turns a retrieval benchmark into a model-load loop.
    return queries[:1] if mode in {"blended", "neural"} else queries


def stop_process(process: subprocess.Popen) -> None:
    if process.poll() is not None:
        return
    if os.name == "nt":
        process.terminate()
    else:
        process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def stop_daemon_and_measure_peak_rss(
    daemon: subprocess.Popen, daemon_log
) -> int | None:
    stop_process(daemon)
    daemon_log.close()
    return peak_child_rss_bytes()


def evaluate(args: argparse.Namespace) -> dict:
    dataset = args.dataset.resolve()
    binary = args.binary.resolve()
    provenance = load_provenance(dataset)
    identity = binary_identity(binary)
    request = expected_execution_request(
        dataset,
        identity["sha256"],
        args.mode,
        args.max_query_chars,
        **{
            key: getattr(args, key, default)
            for key, default in contracts.EVALUATION_DEFAULTS.items()
            if key != "max_query_chars"
        },
    )
    execution_source = source_revision(getattr(args, "source_commit", None))
    executed_at = datetime.now(timezone.utc).isoformat()
    capture_enabled = getattr(args, "capture_reranker", False)
    capture_directory = None
    if capture_enabled:
        if args.query_expansion != "none" or args.output is None:
            raise ValueError(
                "--capture-reranker requires --output and no query expansion"
            )
        capture_directory = args.output.with_suffix(
            args.output.suffix + ".native-captures"
        )
        capture_directory.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="ivygrep-retrieval-") as temp:
        temp_path = Path(temp)
        repo = temp_path / "repo"
        home = temp_path / "home"
        path_to_id = materialize_corpus(dataset, repo)
        id_to_path = {document_id: path for path, document_id in path_to_id.items()}
        queries = selected_queries(
            load_jsonl(dataset / "queries.jsonl"),
            args.query_id,
        )
        qrels = load_qrels(dataset / "qrels.tsv")
        env = os.environ.copy()
        env["IVYGREP_HOME"] = str(home)
        env["IVYGREP_NO_AUTOSPAWN"] = "1"
        env["IVYGREP_ENHANCE_MAX_LOAD_RATIO"] = "0"
        env["IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT"] = "1"
        if capture_enabled:
            env["IVYGREP_RERANKER_CAPTURE"] = "1"

        started = time.perf_counter()
        subprocess.run(
            [str(binary), "--add", str(repo), "--no-watch", "--hash"],
            cwd=repo,
            env=env,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        index_ms = (time.perf_counter() - started) * 1000.0

        hash_enhancement_ms = 0.0
        if args.mode in {"hash", "hybrid", "blended", "neural"}:
            started = time.perf_counter()
            subprocess.run(
                [str(binary), "--enhance-hash-internal", str(repo)],
                cwd=repo,
                env=env,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            hash_enhancement_ms = (time.perf_counter() - started) * 1000.0
        neural_enhancement_ms = 0.0
        if args.mode in {"blended", "neural"}:
            neural_env = env.copy()
            started = time.perf_counter()
            subprocess.run(
                [str(binary), "--enhance-internal", str(repo)],
                cwd=repo,
                env=neural_env,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            neural_enhancement_ms = (time.perf_counter() - started) * 1000.0
            status, _ = run_json([str(binary), "--status", "--json"], repo, neural_env)
            workspace = next(item for item in status if Path(item["root"]) == repo)
            if not workspace["has_neural_vectors"]:
                raise RuntimeError(
                    "neural mode requested but no neural vectors were built"
                )

        cold_latencies: dict[str, float] = {}
        for query in (
            [] if capture_enabled else process_cold_queries(args.mode, queries)
        ):
            query_id = str(query["_id"])
            text = query_text(query, args.max_query_chars)
            cold_ms = 0.0
            for expansion_index, expanded_text in enumerate(
                expanded_query_texts(
                    text,
                    args.query_expansion,
                    args.probe_query_chars,
                )
            ):
                command = search_command(
                    binary,
                    args.mode,
                    (
                        args.limit
                        if expansion_index == 0
                        else args.probe_limit or args.limit
                    ),
                    expanded_text,
                    query_scope(query, repo),
                    query_exclude_globs(query, repo),
                    args.disable_memory_expansion or args.query_expansion != "none",
                )
                _, elapsed_ms = run_json(command, repo, env)
                cold_ms += elapsed_ms
            cold_latencies[query_id] = cold_ms

        daemon_env = env.copy()
        daemon_log_path = temp_path / "daemon.log"
        daemon = None
        daemon_log = None
        daemon_startup_ms = 0.0
        neural_model_ready_ms = 0.0
        if not capture_enabled:
            daemon_env.pop("IVYGREP_NO_AUTOSPAWN", None)
            daemon_log = daemon_log_path.open("wb")
            popen_options = {"start_new_session": True} if os.name != "nt" else {}
            daemon_started = time.perf_counter()
            daemon = subprocess.Popen(
                [str(binary), "--daemon"],
                cwd=repo,
                env=daemon_env,
                stdout=daemon_log,
                stderr=subprocess.STDOUT,
                **popen_options,
            )
        result = None
        try:
            if daemon is not None:
                endpoint = daemon_endpoint_path(home)
                deadline = time.time() + 10
                while not endpoint.exists() and time.time() < deadline:
                    if daemon.poll() is not None:
                        raise RuntimeError(
                            "ivygrep daemon exited before becoming ready"
                        )
                    time.sleep(0.05)
                if not endpoint.exists():
                    raise TimeoutError("timed out waiting for ivygrep daemon")
                daemon_startup_ms = (time.perf_counter() - daemon_started) * 1000.0

            if daemon is not None and args.mode in {"blended", "neural"}:
                model_started = time.perf_counter()
                run_json(
                    search_command(binary, args.mode, 1, "neural model warmup"),
                    repo,
                    daemon_env,
                )
                deadline = time.time() + 120
                while time.time() < deadline:
                    if daemon.poll() is not None:
                        raise RuntimeError(
                            "ivygrep daemon exited while loading the neural model"
                        )
                    if (
                        daemon_log_path.exists()
                        and "embedding model ready"
                        in daemon_log_path.read_text(encoding="utf-8", errors="replace")
                    ):
                        break
                    time.sleep(0.1)
                else:
                    raise TimeoutError(
                        "timed out waiting for the daemon neural model to become ready"
                    )
                neural_model_ready_ms = (time.perf_counter() - model_started) * 1000.0

            scores: list[dict[str, float]] = []
            warm_latencies: list[float] = []
            details = []
            no_hit_queries = 0
            support_file_hits = 0
            support_file_candidates = 0
            queries_with_hash_results = 0
            queries_with_neural_results = 0
            queries_with_neural_execution = 0
            queries_with_unobservable_neural_execution = 0
            missing_neural_execution = []
            for query_number, query in enumerate(queries):
                query_id = str(query["_id"])
                text = query_text(query, args.max_query_chars)
                cold_ms = cold_latencies.get(query_id)
                commands = [
                    search_command(
                        binary,
                        args.mode,
                        (
                            args.limit
                            if expansion_index == 0
                            else args.probe_limit or args.limit
                        ),
                        expanded_text,
                        query_scope(query, repo),
                        query_exclude_globs(query, repo),
                        args.disable_memory_expansion or args.query_expansion != "none",
                    )
                    for expansion_index, expanded_text in enumerate(
                        expanded_query_texts(
                            text,
                            args.query_expansion,
                            args.probe_query_chars,
                        )
                    )
                ]
                native_capture = None
                if capture_enabled:
                    output, warm_ms, native_capture = run_captured_query(
                        commands[0],
                        repo,
                        daemon_env,
                        text,
                        capture_directory / f"q{query_number:06d}",
                    )
                    native_capture["candidate_document_ids"] = captured_document_ids(
                        native_capture["record"], repo, path_to_id
                    )
                    warm_outputs = [output]
                else:
                    warm_outputs, warm_ms = run_search_commands(
                        commands,
                        repo,
                        daemon_env,
                        args.query_expansion_workers,
                    )
                warm_output = fuse_search_outputs(
                    warm_outputs,
                    rrf_k=args.rrf_k,
                    original_weight=args.original_weight,
                )
                ranked = []
                ranked_hits = []
                seen: set[str] = set()
                query_hits = [
                    hit
                    for output in warm_outputs
                    for item in output
                    for hit in item.get("hits", [])
                ]
                query_sources = {
                    str(source)
                    for hit in query_hits
                    for source in hit.get("sources", [])
                }
                execution_status = neural_execution_status(query_hits)
                neural_executed = execution_status is True
                if "hash" in query_sources:
                    queries_with_hash_results += 1
                if "neural" in query_sources:
                    queries_with_neural_results += 1
                if execution_status is True:
                    queries_with_neural_execution += 1
                elif execution_status is None:
                    queries_with_unobservable_neural_execution += 1
                elif args.mode == "neural":
                    missing_neural_execution.append(query_id)
                for item in warm_output:
                    result_path = Path(item["file_path"])
                    try:
                        relative = result_path.relative_to(repo).as_posix()
                    except ValueError:
                        relative = result_path.as_posix()
                    if relative in path_to_id:
                        document_id = path_to_id[relative]
                        if document_id not in seen:
                            seen.add(document_id)
                            ranked.append(document_id)
                            hits = item.get("hits") or []
                            sources = sorted(
                                {
                                    source
                                    for hit in hits
                                    for source in hit.get("sources", [])
                                }
                            )
                            ranked_hits.append(
                                {
                                    "document_id": document_id,
                                    "file_path": relative,
                                    "total_score": float(item.get("total_score", 0.0)),
                                    "hit_count": int(item.get("hit_count", len(hits))),
                                    "sources": sources,
                                    "preview": candidate_preview(hits),
                                    **(
                                        {"fusion_score": item["fusion_score"]}
                                        if "fusion_score" in item
                                        else {}
                                    ),
                                }
                            )
                query_score = score_query(ranked, qrels.get(query_id, {}))
                if not ranked:
                    no_hit_queries += 1
                eligible_for_support_spam = not query_targets_support(text)
                query_support_hits = None
                if eligible_for_support_spam:
                    top_paths = [
                        id_to_path[document_id]
                        for document_id in ranked[:10]
                        if document_id in id_to_path
                    ]
                    query_support_hits = sum(
                        is_support_path(path) for path in top_paths
                    )
                    support_file_candidates += len(top_paths)
                    support_file_hits += query_support_hits
                scores.append(query_score)
                warm_latencies.append(warm_ms)
                details.append(
                    {
                        "query_id": query_id,
                        "retrieval_sources": sorted(query_sources),
                        "neural_executed": neural_executed,
                        "neural_execution_observable": execution_status is not None,
                        "ranked": ranked,
                        "ranked_hits": ranked_hits,
                        "cold_latency_ms": cold_ms,
                        "warm_latency_ms": warm_ms,
                        "no_hit": not ranked,
                        "support_file_hits_at_10": query_support_hits,
                        **(
                            {"native_capture": native_capture}
                            if native_capture is not None
                            else {}
                        ),
                        **query_score,
                    }
                )

            if missing_neural_execution:
                raise RuntimeError(
                    "neural mode did not execute neural retrieval for queries: "
                    + ", ".join(missing_neural_execution)
                )

            status, _ = run_json([str(binary), "--status", "--json"], repo, daemon_env)
            workspace = next(item for item in status if Path(item["root"]) == repo)
            index_configuration = {
                key: workspace[key]
                for key in (
                    "chunk_count",
                    "file_count",
                    "index_components",
                    "vector_key_count",
                    "has_neural_vectors",
                    "neural_vector_count",
                    "neural_coverage_percent",
                    "neural_dimensions",
                    "neural_profile",
                    "neural_model",
                    "neural_backend",
                    "reranker_candidate_limit",
                    "reranker_mode",
                    "reranker_model",
                    "reranker_error",
                )
                if key in workspace
            }
            result = {
                "dataset": dataset.name,
                "dataset_provenance": provenance,
                "mode": args.mode,
                "queries": len(queries),
                "binary": identity,
                "runtime": request["runtime"],
                "index_ms": index_ms,
                "hash_enhancement_ms": hash_enhancement_ms,
                "neural_enhancement_ms": neural_enhancement_ms,
                "index_size_bytes": workspace["index_size_bytes"],
                "index_configuration": index_configuration,
                "candidate_trace": candidate_trace_contract(
                    args.query_expansion, index_configuration.get("reranker_mode")
                ),
                "cold_latency_samples": len(cold_latencies),
                "cold_latency_p50_ms": percentile(list(cold_latencies.values()), 0.50),
                "cold_latency_p95_ms": percentile(list(cold_latencies.values()), 0.95),
                "warm_latency_p50_ms": percentile(warm_latencies, 0.50),
                "warm_latency_p95_ms": percentile(warm_latencies, 0.95),
                "daemon_startup_ms": daemon_startup_ms,
                "neural_model_ready_ms": neural_model_ready_ms,
                "warm_query_path": "local-process-native-capture"
                if capture_enabled
                else warm_query_path(args.mode),
                "measurement_scope": "native-training-capture"
                if capture_enabled
                else "retrieval-benchmark",
                "query_text_limit": args.max_query_chars,
                "query_expansion": args.query_expansion,
                "query_expansion_workers": args.query_expansion_workers,
                "probe_limit": args.probe_limit or args.limit,
                "probe_query_chars": args.probe_query_chars,
                "rrf_k": args.rrf_k,
                "original_weight": args.original_weight,
                "memory_expansion_disabled": args.disable_memory_expansion,
                "retrieval_provenance": {
                    "force_neural": args.mode == "neural",
                    "mode_semantics": (
                        "blended-routing"
                        if args.mode == "blended"
                        else "forced-neural"
                        if args.mode == "neural"
                        else args.mode
                    ),
                    "queries_with_hash_results": queries_with_hash_results,
                    "queries_with_neural_results": queries_with_neural_results,
                    "queries_with_neural_execution": queries_with_neural_execution,
                    "queries_with_unobservable_neural_execution": (
                        queries_with_unobservable_neural_execution
                    ),
                },
                "no_hit_rate": no_hit_queries / len(queries) if queries else 0.0,
                "support_file_spam_rate_at_10": (
                    support_file_hits / support_file_candidates
                    if support_file_candidates
                    else 0.0
                ),
                **aggregate(scores),
                "details": details,
            }
            if capture_enabled:
                records = [detail["native_capture"]["record"] for detail in details]
                if any(
                    record["status"] == "applied"
                    and record["model_id"] != index_configuration.get("reranker_model")
                    for record in records
                ):
                    raise ValueError(
                        "native capture model does not match observed runtime identity"
                    )
                result["native_capture_contract"] = {
                    "schema_version": 1,
                    "stage": contracts.CAPTURE_STAGE,
                    "transport": "fresh-process-stderr",
                    "ranking_context_lines": 2,
                    "feature_schema": list(contracts.RERANK_FEATURE_SCHEMA),
                    "receipt_directory": capture_directory.name,
                    "applied_queries": sum(
                        record["status"] == "applied" for record in records
                    ),
                    "skipped_queries": sum(
                        record["status"] == "skipped" for record in records
                    ),
                    "skip_reasons": dict(
                        Counter(
                            record["reason"]
                            for record in records
                            if record["status"] == "skipped"
                        )
                    ),
                }
        finally:
            peak_rss = (
                stop_daemon_and_measure_peak_rss(daemon, daemon_log)
                if daemon is not None
                else peak_child_rss_bytes()
            )

        result["peak_child_rss_bytes"] = peak_rss
        if contracts.dataset_fingerprint(dataset) != request["dataset_content"]:
            raise ValueError("dataset inputs changed during evaluation")
        if (
            contracts.execution_harness(Path(__file__).resolve().parents[1])
            != request["harness_sha256"]
        ):
            raise ValueError("execution harness changed during evaluation")
        observed = contracts.observed_configuration(result["index_configuration"])
        contracts.validate_observed_configuration(request, observed)
        result["execution_provenance"] = {
            "schema_version": contracts.EXECUTION_SCHEMA_VERSION,
            "request": request,
            "request_sha256": contracts.canonical_sha256(request),
            "source_commit": execution_source,
            "executed_at": executed_at,
            "observed_configuration": observed,
        }
        if sha256_file(binary) != identity["sha256"]:
            raise ValueError("binary changed during evaluation")
        return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=Path("target/release/ig"))
    parser.add_argument(
        "--source-commit",
        help="Commit used to build --binary; retained as original execution provenance.",
    )
    parser.add_argument(
        "--capture-reranker",
        action="store_true",
        help="Explicit training diagnostic: collect native pre-learned features from fresh local process stderr. Requires a capture-capable binary, --output and no expansion; latency includes process/model startup and is not the normal warm benchmark path.",
    )
    parser.add_argument(
        "--mode",
        choices=["lexical", "hash", "hybrid", "blended", "neural"],
        default="hash",
    )
    parser.add_argument("--limit", type=int, default=20)
    parser.add_argument("--query-id", action="append", default=[])
    parser.add_argument("--max-query-chars", type=int)
    parser.add_argument(
        "--query-expansion",
        choices=[
            "none",
            "memory-context",
            "memory-history",
            "memory-action",
            "memory-context-history",
            "memory-context-action",
            "memory-history-action",
            "memory-facets",
            "retrieval-facets",
        ],
        default="none",
    )
    parser.add_argument("--query-expansion-workers", type=int, default=4)
    parser.add_argument("--probe-limit", type=int)
    parser.add_argument("--probe-query-chars", type=int)
    parser.add_argument("--rrf-k", type=float, default=60.0)
    parser.add_argument("--original-weight", type=float, default=1.0)
    parser.add_argument("--disable-memory-expansion", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--min-ndcg-at-10", type=float, default=0.0)
    parser.add_argument("--min-mrr-at-10", type=float, default=0.0)
    parser.add_argument("--min-precision-at-5", type=float, default=0.0)
    parser.add_argument("--min-recall-at-20", type=float, default=0.0)
    parser.add_argument("--require-relevant-results", action="store_true")
    args = parser.parse_args()
    if args.query_expansion_workers < 1:
        parser.error("--query-expansion-workers must be positive")
    if args.probe_limit is not None and args.probe_limit < args.limit:
        parser.error("--probe-limit must be at least --limit")
    if args.probe_query_chars is not None and args.probe_query_chars < 1:
        parser.error("--probe-query-chars must be positive")
    if args.rrf_k <= 0:
        parser.error("--rrf-k must be positive")
    if args.original_weight <= 0:
        parser.error("--original-weight must be positive")
    if args.max_query_chars is not None and args.max_query_chars < 1:
        raise SystemExit("--max-query-chars must be positive")
    if args.capture_reranker and (
        args.query_expansion != "none" or args.output is None
    ):
        parser.error("--capture-reranker requires --output and --query-expansion none")

    result = evaluate(args)
    payload = json.dumps(result, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    if args.output:
        print(
            json.dumps(
                {
                    "dataset": result["dataset"],
                    "mode": result["mode"],
                    "queries": result["queries"],
                    "ndcg_at_10": result["ndcg_at_10"],
                    "mrr_at_10": result["mrr_at_10"],
                    "recall_at_20": result["recall_at_20"],
                    "output": args.output.name,
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(payload)
    thresholds = {
        "ndcg_at_10": args.min_ndcg_at_10,
        "mrr_at_10": args.min_mrr_at_10,
        "precision_at_5": args.min_precision_at_5,
        "recall_at_20": args.min_recall_at_20,
    }
    failures = [
        f"{metric}={result[metric]:.4f} < {minimum:.4f}"
        for metric, minimum in thresholds.items()
        if result[metric] < minimum
    ]
    if args.require_relevant_results:
        missing = [
            detail["query_id"]
            for detail in result["details"]
            if detail["recall_at_20"] <= 0.0
        ]
        if missing:
            failures.append("queries lost every relevant result: " + ", ".join(missing))
    if failures:
        raise SystemExit("retrieval quality gate failed: " + ", ".join(failures))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
