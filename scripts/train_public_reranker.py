#!/usr/bin/env python3
"""Train a compact linear reranker from disjoint public retrieval traces."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import subprocess

import eval_code_retrieval


FEATURE_NAMES = (
    "log_total_score",
    "reciprocal_rank",
    "hit_count",
    "source_count",
    "source_lexical",
    "source_semantic",
    "source_literal",
    "source_path",
    "source_symbol",
    "query_preview_coverage",
    "query_path_coverage",
    "exact_query_preview",
    "exact_query_path",
    "support_path",
    "primary_source",
    "shallow_path",
    "query_length",
    "preview_length",
    "lexical_semantic",
    "literal_exact",
    "semantic_only",
)

CODE_EXTENSIONS = {
    "c",
    "cc",
    "cpp",
    "cs",
    "go",
    "h",
    "hpp",
    "java",
    "js",
    "jsx",
    "kt",
    "kts",
    "php",
    "py",
    "rb",
    "rs",
    "scala",
    "swift",
    "ts",
    "tsx",
}


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


def query_terms(text: str) -> list[str]:
    terms = []
    current = []
    for character in text.lower():
        if character.isascii() and character.isalnum():
            current.append(character)
        elif current:
            term = "".join(current)
            if len(term) >= 2 and term not in terms:
                terms.append(term)
            current = []
    if current:
        term = "".join(current)
        if len(term) >= 2 and term not in terms:
            terms.append(term)
    return terms


def coverage(terms: list[str], text: str) -> float:
    if not terms:
        return 0.0
    lower = text.lower()
    return sum(term in lower for term in terms) / len(terms)


def is_support_path(path: str) -> bool:
    return eval_code_retrieval.is_support_path(path)


def feature_vector(query: str, candidate: dict, rank: int) -> list[float]:
    path = str(candidate.get("file_path", "")).replace("\\", "/")
    preview = str(candidate.get("preview", ""))
    sources = set(candidate.get("sources", []))
    terms = query_terms(query)
    query_lower = query.strip().lower()
    path_lower = path.lower()
    preview_lower = preview.lower()
    extension = Path(path_lower).suffix.lstrip(".")
    support = is_support_path(path_lower)
    lexical = "lexical" in sources
    semantic = "semantic" in sources
    literal = "literal" in sources
    exact_preview = bool(query_lower and query_lower in preview_lower)
    exact_path = bool(query_lower and query_lower in path_lower)
    return [
        min(math.log1p(max(0.0, float(candidate.get("total_score", 0.0)))), 4.0)
        / 4.0,
        1.0 / (rank + 1.0),
        min(float(candidate.get("hit_count", 0)), 4.0) / 4.0,
        min(len(sources), 5) / 5.0,
        float(lexical),
        float(semantic),
        float(literal),
        float("path" in sources),
        float("symbol" in sources),
        coverage(terms, preview_lower),
        coverage(terms, path_lower),
        float(exact_preview),
        float(exact_path),
        float(support),
        float(extension in CODE_EXTENSIONS and not support),
        1.0 / (1.0 + path_lower.count("/")),
        min(len(terms), 20) / 20.0,
        min(len(preview), 12000) / 12000.0,
        float(lexical and semantic),
        float(literal and (exact_preview or exact_path)),
        float(semantic and not (lexical or literal or "path" in sources or "symbol" in sources)),
    ]


def parse_pair(value: str) -> tuple[Path, Path]:
    dataset, separator, result = value.partition("=")
    if not separator:
        raise ValueError(f"expected DATASET=RESULT, got {value!r}")
    return Path(dataset), Path(result)


def load_examples(pairs: list[tuple[Path, Path]]) -> tuple[list[dict], list[dict]]:
    examples = []
    provenance = []
    for dataset, result_path in pairs:
        result = json.loads(result_path.read_text(encoding="utf-8"))
        queries = {
            str(query["_id"]): str(query.get("text") or query.get("query") or "")
            for query in eval_code_retrieval.load_jsonl(dataset / "queries.jsonl")
        }
        qrels = eval_code_retrieval.load_qrels(dataset / "qrels.tsv")
        for detail in result["details"]:
            query_id = str(detail["query_id"])
            candidates = []
            for rank, candidate in enumerate(detail.get("ranked_hits", [])):
                candidates.append(
                    {
                        "document_id": str(candidate["document_id"]),
                        "features": feature_vector(queries[query_id], candidate, rank),
                        "grade": qrels.get(query_id, {}).get(
                            str(candidate["document_id"]), 0
                        ),
                        "rank": rank,
                    }
                )
            if candidates:
                examples.append(
                    {
                        "dataset": dataset.name,
                        "query_id": query_id,
                        "candidates": candidates,
                        "judgments": qrels.get(query_id, {}),
                    }
                )
        provenance.append(
            {
                "dataset": dataset.name,
                "dataset_provenance_sha256": sha256_file(dataset / "provenance.json"),
                "result_sha256": sha256_file(result_path),
                "binary": result["binary"],
                "queries": result["queries"],
            }
        )
    return examples, provenance


def split_examples(examples: list[dict]) -> tuple[list[dict], list[dict]]:
    train = []
    validation = []
    for example in examples:
        key = f"{example['dataset']}:{example['query_id']}".encode()
        bucket = int.from_bytes(hashlib.sha256(key).digest()[:4], "big") % 5
        (validation if bucket == 0 else train).append(example)
    return train, validation


def training_pairs(examples: list[dict]) -> list[tuple[list[float], float]]:
    pairs = []
    for example in examples:
        candidates = example["candidates"]
        for preferred in candidates:
            for other in candidates:
                grade_delta = preferred["grade"] - other["grade"]
                if grade_delta <= 0:
                    continue
                pairs.append(
                    (
                        [
                            left - right
                            for left, right in zip(
                                preferred["features"], other["features"], strict=True
                            )
                        ],
                        float(grade_delta),
                    )
                )
    return pairs


def train_weights(
    examples: list[dict], learning_rate: float, regularization: float, epochs: int
) -> list[float]:
    pairs = training_pairs(examples)
    if not pairs:
        raise ValueError("training traces contain no ranked relevance pairs")
    weights = [0.0] * len(FEATURE_NAMES)
    weights[FEATURE_NAMES.index("log_total_score")] = 1.0
    weights[FEATURE_NAMES.index("reciprocal_rank")] = 0.25
    generator = random.Random(20260615)
    for epoch in range(epochs):
        generator.shuffle(pairs)
        rate = learning_rate / math.sqrt(epoch + 1.0)
        for difference, importance in pairs:
            margin = sum(
                weight * value for weight, value in zip(weights, difference, strict=True)
            )
            probability = 1.0 / (1.0 + math.exp(min(40.0, max(-40.0, margin))))
            for index, value in enumerate(difference):
                weights[index] += rate * (
                    importance * probability * value - regularization * weights[index]
                )
    return weights


def score_candidate(candidate: dict, weights: list[float]) -> float:
    return sum(
        weight * value
        for weight, value in zip(weights, candidate["features"], strict=True)
    )


def evaluate(examples: list[dict], weights: list[float] | None) -> dict[str, float]:
    scores = []
    for example in examples:
        candidates = list(example["candidates"])
        if weights is not None:
            candidates.sort(
                key=lambda candidate: (
                    -score_candidate(candidate, weights),
                    candidate["rank"],
                )
            )
        ranked = [candidate["document_id"] for candidate in candidates]
        scores.append(
            eval_code_retrieval.score_query(ranked, example["judgments"])
        )
    return eval_code_retrieval.aggregate(scores)


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--pair", action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    pairs = [parse_pair(value) for value in args.pair]
    examples, provenance = load_examples(pairs)
    train, validation = split_examples(examples)
    if not train or not validation:
        raise ValueError("training and validation splits must both be non-empty")

    candidates = []
    for learning_rate in (0.02, 0.05, 0.1):
        for regularization in (0.0001, 0.001, 0.01):
            weights = train_weights(train, learning_rate, regularization, 80)
            metrics = evaluate(validation, weights)
            candidates.append(
                {
                    "learning_rate": learning_rate,
                    "regularization": regularization,
                    "epochs": 80,
                    "weights": weights,
                    "metrics": metrics,
                }
            )
    selected = max(
        candidates,
        key=lambda candidate: (
            candidate["metrics"]["ndcg_at_10"],
            candidate["metrics"]["mrr_at_10"],
        ),
    )
    weights = train_weights(
        examples,
        selected["learning_rate"],
        selected["regularization"],
        selected["epochs"],
    )
    report = {
        "schema_version": 1,
        "model_id": "public-linear-reranker-v1",
        "feature_schema": list(FEATURE_NAMES),
        "weights": weights,
        "training": {
            "ivygrep_commit": git_revision(root),
            "queries": len(examples),
            "train_queries": len(train),
            "validation_queries": len(validation),
            "sources": provenance,
            "selected_hyperparameters": {
                "learning_rate": selected["learning_rate"],
                "regularization": selected["regularization"],
                "epochs": selected["epochs"],
            },
            "baseline_validation": evaluate(validation, None),
            "learned_validation": selected["metrics"],
            "baseline_all": evaluate(examples, None),
            "learned_all": evaluate(examples, weights),
        },
    }
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(report["training"], indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
