#!/usr/bin/env python3
"""Evaluate ivygrep on BEIR/CoIR-style code retrieval datasets."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import signal
import subprocess
import tempfile
import time

try:
    import resource
except ImportError:  # pragma: no cover - unavailable on Windows
    resource = None


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
    return {
        "ndcg_at_10": dcg(ranked, 10) / ideal_dcg if ideal_dcg else 0.0,
        "mrr_at_10": 1.0 / first_relevant if first_relevant else 0.0,
        "precision_at_5": sum(doc_id in relevant for doc_id in ranked[:5]) / 5.0,
        "recall_at_20": (
            sum(doc_id in relevant for doc_id in ranked[:20]) / len(relevant)
            if relevant
            else 0.0
        ),
    }


def aggregate(scores: list[dict[str, float]]) -> dict[str, float]:
    if not scores:
        return {
            "ndcg_at_10": 0.0,
            "mrr_at_10": 0.0,
            "precision_at_5": 0.0,
            "recall_at_20": 0.0,
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
        metadata = document.get("metadata") or {}
        relative = metadata.get("path") or f"documents/{position:06d}-{doc_id}.txt"
        relative_path = Path(str(relative))
        if relative_path.is_absolute() or ".." in relative_path.parts:
            relative_path = Path("documents") / f"{position:06d}-{doc_id}.txt"
        relative = relative_path.as_posix()
        target = repo / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        title = document.get("title") or ""
        text = document.get("text") or ""
        target.write_text(f"{title}\n{text}".lstrip(), encoding="utf-8")
        path_to_id[relative] = doc_id
    return path_to_id


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
        check=True,
    )
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    return json.loads(completed.stdout), elapsed_ms


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


def search_command(binary: Path, mode: str, limit: int, query: str) -> list[str]:
    return [
        str(binary),
        "--json",
        "-n",
        str(limit),
        *query_args(mode),
        "--",
        query,
    ]


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
    with tempfile.TemporaryDirectory(prefix="ivygrep-retrieval-") as temp:
        temp_path = Path(temp)
        repo = temp_path / "repo"
        home = temp_path / "home"
        path_to_id = materialize_corpus(dataset, repo)
        id_to_path = {document_id: path for path, document_id in path_to_id.items()}
        queries = load_jsonl(dataset / "queries.jsonl")
        qrels = load_qrels(dataset / "qrels.tsv")
        env = os.environ.copy()
        env["IVYGREP_HOME"] = str(home)
        env["IVYGREP_NO_AUTOSPAWN"] = "1"
        env["IVYGREP_ENHANCE_MAX_LOAD_RATIO"] = "0"
        env["IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT"] = "1"

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
        for query in process_cold_queries(args.mode, queries):
            query_id = str(query["_id"])
            text = query_text(query, args.max_query_chars)
            command = search_command(binary, args.mode, args.limit, text)
            _, cold_latencies[query_id] = run_json(command, repo, env)

        daemon_env = env.copy()
        daemon_env.pop("IVYGREP_NO_AUTOSPAWN", None)
        daemon_log_path = temp_path / "daemon.log"
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
            endpoint = daemon_endpoint_path(home)
            deadline = time.time() + 10
            while not endpoint.exists() and time.time() < deadline:
                if daemon.poll() is not None:
                    raise RuntimeError("ivygrep daemon exited before becoming ready")
                time.sleep(0.05)
            if not endpoint.exists():
                raise TimeoutError("timed out waiting for ivygrep daemon")
            daemon_startup_ms = (time.perf_counter() - daemon_started) * 1000.0

            neural_model_ready_ms = 0.0
            if args.mode in {"blended", "neural"}:
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
            for query in queries:
                query_id = str(query["_id"])
                text = query_text(query, args.max_query_chars)
                command = search_command(binary, args.mode, args.limit, text)

                cold_ms = cold_latencies.get(query_id)
                warm_output, warm_ms = run_json(command, repo, daemon_env)
                ranked = []
                ranked_hits = []
                seen: set[str] = set()
                query_hits = [
                    hit for item in warm_output for hit in item.get("hits", [])
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
                                    "preview": "\n".join(
                                        str(hit.get("preview", "")) for hit in hits[:3]
                                    )[:12000],
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
                    query_support_hits = sum(is_support_path(path) for path in top_paths)
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
                "runtime": runtime_metadata(),
                "index_ms": index_ms,
                "hash_enhancement_ms": hash_enhancement_ms,
                "neural_enhancement_ms": neural_enhancement_ms,
                "index_size_bytes": workspace["index_size_bytes"],
                "index_configuration": index_configuration,
                "cold_latency_samples": len(cold_latencies),
                "cold_latency_p50_ms": percentile(list(cold_latencies.values()), 0.50),
                "cold_latency_p95_ms": percentile(list(cold_latencies.values()), 0.95),
                "warm_latency_p50_ms": percentile(warm_latencies, 0.50),
                "warm_latency_p95_ms": percentile(warm_latencies, 0.95),
                "daemon_startup_ms": daemon_startup_ms,
                "neural_model_ready_ms": neural_model_ready_ms,
                "warm_query_path": warm_query_path(args.mode),
                "query_text_limit": args.max_query_chars,
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
        finally:
            peak_rss = stop_daemon_and_measure_peak_rss(daemon, daemon_log)

        result["peak_child_rss_bytes"] = peak_rss
        return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=Path("target/release/ig"))
    parser.add_argument(
        "--mode",
        choices=["lexical", "hash", "hybrid", "blended", "neural"],
        default="hash",
    )
    parser.add_argument("--limit", type=int, default=20)
    parser.add_argument("--max-query-chars", type=int)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--min-ndcg-at-10", type=float, default=0.0)
    parser.add_argument("--min-mrr-at-10", type=float, default=0.0)
    parser.add_argument("--min-precision-at-5", type=float, default=0.0)
    parser.add_argument("--min-recall-at-20", type=float, default=0.0)
    args = parser.parse_args()
    if args.max_query_chars is not None and args.max_query_chars < 1:
        raise SystemExit("--max-query-chars must be positive")

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
    if failures:
        raise SystemExit("retrieval quality gate failed: " + ", ".join(failures))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
