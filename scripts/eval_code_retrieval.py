#!/usr/bin/env python3
"""Evaluate ivygrep on BEIR/CoIR-style code retrieval datasets."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time


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
        (rank for rank, doc_id in enumerate(ranked[:10], start=1) if doc_id in relevant),
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
    return {
        key: sum(score[key] for score in scores) / len(scores)
        for key in scores[0]
    }


def percentile(values: list[float], percentile_value: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = math.ceil(percentile_value * len(ordered)) - 1
    return ordered[max(0, min(index, len(ordered) - 1))]


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


def run_json(command: list[str], cwd: Path, env: dict[str, str]) -> tuple[object, float]:
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
    if mode == "neural":
        return []
    raise ValueError(f"unsupported mode {mode}")


def daemon_endpoint_path(home: Path) -> Path:
    return home / ("daemon.port" if os.name == "nt" else "daemon.sock")


def warm_query_path(mode: str) -> str:
    return "local-process" if mode == "lexical" else "daemon"


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


def evaluate(args: argparse.Namespace) -> dict:
    dataset = args.dataset.resolve()
    binary = args.binary.resolve()
    with tempfile.TemporaryDirectory(prefix="ivygrep-retrieval-") as temp:
        temp_path = Path(temp)
        repo = temp_path / "repo"
        home = temp_path / "home"
        path_to_id = materialize_corpus(dataset, repo)
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

        if args.mode in {"hash", "hybrid", "neural"}:
            subprocess.run(
                [str(binary), "--enhance-hash-internal", str(repo)],
                cwd=repo,
                env=env,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
        if args.mode == "neural":
            neural_env = env.copy()
            subprocess.run(
                [str(binary), "--enhance-internal", str(repo)],
                cwd=repo,
                env=neural_env,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            status, _ = run_json(
                [str(binary), "--status", "--json"], repo, neural_env
            )
            workspace = next(item for item in status if Path(item["root"]) == repo)
            if not workspace["has_neural_vectors"]:
                raise RuntimeError(
                    "neural mode requested but no neural vectors were built"
                )

        daemon_env = env.copy()
        daemon_env.pop("IVYGREP_NO_AUTOSPAWN", None)
        daemon_log_path = temp_path / "daemon.log"
        daemon_log = daemon_log_path.open("wb")
        popen_options = {"start_new_session": True} if os.name != "nt" else {}
        daemon = subprocess.Popen(
            [str(binary), "--daemon"],
            cwd=repo,
            env=daemon_env,
            stdout=daemon_log,
            stderr=subprocess.STDOUT,
            **popen_options,
        )
        try:
            endpoint = daemon_endpoint_path(home)
            deadline = time.time() + 10
            while not endpoint.exists() and time.time() < deadline:
                if daemon.poll() is not None:
                    raise RuntimeError("ivygrep daemon exited before becoming ready")
                time.sleep(0.05)
            if not endpoint.exists():
                raise TimeoutError("timed out waiting for ivygrep daemon")

            if args.mode == "neural":
                run_json(
                    [str(binary), "--json", "-n", "1", "neural model warmup"],
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
                        in daemon_log_path.read_text(
                            encoding="utf-8", errors="replace"
                        )
                    ):
                        break
                    time.sleep(0.1)
                else:
                    raise TimeoutError(
                        "timed out waiting for the daemon neural model to become ready"
                    )

            scores: list[dict[str, float]] = []
            cold_latencies: list[float] = []
            warm_latencies: list[float] = []
            details = []
            for query in queries:
                query_id = str(query["_id"])
                text = query.get("text") or query.get("query") or ""
                command = [
                    str(binary),
                    "--json",
                    "-n",
                    str(args.limit),
                    *query_args(args.mode),
                    text,
                ]

                cold_env = env.copy()
                _, cold_ms = run_json(command, repo, cold_env)
                warm_output, warm_ms = run_json(command, repo, daemon_env)
                ranked = []
                for item in warm_output:
                    result_path = Path(item["file_path"])
                    try:
                        relative = result_path.relative_to(repo).as_posix()
                    except ValueError:
                        relative = result_path.as_posix()
                    if relative in path_to_id:
                        ranked.append(path_to_id[relative])
                query_score = score_query(ranked, qrels.get(query_id, {}))
                scores.append(query_score)
                cold_latencies.append(cold_ms)
                warm_latencies.append(warm_ms)
                details.append(
                    {
                        "query_id": query_id,
                        "ranked": ranked,
                        "cold_latency_ms": cold_ms,
                        "warm_latency_ms": warm_ms,
                        **query_score,
                    }
                )

            status, _ = run_json([str(binary), "--status", "--json"], repo, daemon_env)
            workspace = next(item for item in status if Path(item["root"]) == repo)
            return {
                "dataset": dataset.name,
                "mode": args.mode,
                "queries": len(queries),
                "index_ms": index_ms,
                "index_size_bytes": workspace["index_size_bytes"],
                "cold_latency_p50_ms": percentile(cold_latencies, 0.50),
                "cold_latency_p95_ms": percentile(cold_latencies, 0.95),
                "warm_latency_p50_ms": percentile(warm_latencies, 0.50),
                "warm_latency_p95_ms": percentile(warm_latencies, 0.95),
                "warm_query_path": warm_query_path(args.mode),
                **aggregate(scores),
                "details": details,
            }
        finally:
            stop_process(daemon)
            daemon_log.close()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=Path("target/release/ig"))
    parser.add_argument(
        "--mode",
        choices=["lexical", "hash", "hybrid", "neural"],
        default="hash",
    )
    parser.add_argument("--limit", type=int, default=20)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--min-ndcg-at-10", type=float, default=0.0)
    parser.add_argument("--min-mrr-at-10", type=float, default=0.0)
    parser.add_argument("--min-precision-at-5", type=float, default=0.0)
    parser.add_argument("--min-recall-at-20", type=float, default=0.0)
    args = parser.parse_args()

    result = evaluate(args)
    payload = json.dumps(result, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
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
