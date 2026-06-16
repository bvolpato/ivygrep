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
    "score_preview_coverage",
    "rank_preview_coverage",
    "short_preview_coverage",
    "medium_preview_coverage",
    "long_preview_coverage",
    "short_semantic",
    "long_semantic",
    "short_literal",
    "long_literal",
    "preview_term_precision",
    "preview_term_f1",
    "weighted_preview_coverage",
    "informative_preview_coverage",
    "long_term_preview_coverage",
    "numeric_preview_coverage",
    "query_bigram_preview_coverage",
    "query_line_preview_coverage",
    "path_term_f1",
    "natural_language_preview_f1",
    "code_query_line_coverage",
)

UNINFORMATIVE_TERMS = {
    "and",
    "are",
    "can",
    "class",
    "const",
    "def",
    "else",
    "false",
    "find",
    "fix",
    "for",
    "from",
    "function",
    "how",
    "import",
    "include",
    "int",
    "let",
    "new",
    "not",
    "null",
    "return",
    "should",
    "static",
    "string",
    "struct",
    "the",
    "this",
    "true",
    "use",
    "using",
    "value",
    "var",
    "void",
    "what",
    "when",
    "where",
    "which",
    "with",
}

NATURAL_LANGUAGE_TERMS = {
    "a",
    "an",
    "and",
    "appropriate",
    "can",
    "error",
    "find",
    "fix",
    "for",
    "following",
    "how",
    "in",
    "is",
    "of",
    "please",
    "should",
    "suggest",
    "the",
    "this",
    "to",
    "value",
    "what",
    "when",
    "where",
    "which",
    "with",
}

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


def set_overlap(left: list[str], right: list[str]) -> tuple[float, float, float]:
    left_set = set(left)
    right_set = set(right)
    if not left_set or not right_set:
        return 0.0, 0.0, 0.0
    overlap = len(left_set & right_set)
    recall = overlap / len(left_set)
    precision = overlap / len(right_set)
    if recall + precision == 0.0:
        return recall, precision, 0.0
    return recall, precision, 2.0 * recall * precision / (recall + precision)


def weighted_coverage(terms: list[str], text_terms: list[str]) -> float:
    if not terms:
        return 0.0
    present = set(text_terms)
    weights = [min(len(term), 16) for term in terms]
    return sum(weight for term, weight in zip(terms, weights) if term in present) / sum(
        weights
    )


def bigram_coverage(query: list[str], text: list[str]) -> float:
    query_bigrams = set(zip(query, query[1:]))
    if not query_bigrams:
        return 0.0
    return len(query_bigrams & set(zip(text, text[1:]))) / len(query_bigrams)


def line_coverage(query: str, text: str) -> float:
    lines = {
        " ".join(line.strip().lower().split())
        for line in query.splitlines()
        if len(" ".join(line.strip().split())) >= 8
    }
    if not lines:
        return 0.0
    normalized_text = " ".join(text.lower().split())
    return sum(line in normalized_text for line in lines) / len(lines)


def query_shape(terms: list[str], query: str) -> tuple[float, float]:
    if not terms:
        return 0.0, 0.0
    natural_language = min(
        1.0,
        sum(term in NATURAL_LANGUAGE_TERMS for term in terms) / max(3.0, len(terms) / 2),
    )
    punctuation = sum(character in "{}[]();=<>:+-*/" for character in query)
    code = min(1.0, punctuation / max(4.0, len(query) / 40.0))
    return natural_language, code


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
    preview_terms = query_terms(preview)
    path_terms = query_terms(path)
    extension = Path(path_lower).suffix.lstrip(".")
    support = is_support_path(path_lower)
    lexical = "lexical" in sources
    semantic = "semantic" in sources
    literal = "literal" in sources
    exact_preview = bool(query_lower and query_lower in preview_lower)
    exact_path = bool(query_lower and query_lower in path_lower)
    preview_coverage = coverage(terms, preview_lower)
    _, preview_precision, preview_f1 = set_overlap(terms, preview_terms)
    _, _, path_f1 = set_overlap(terms, path_terms)
    informative_terms = [
        term for term in terms if len(term) >= 4 and term not in UNINFORMATIVE_TERMS
    ]
    long_terms = [term for term in terms if len(term) >= 7]
    numeric_terms = [term for term in terms if term.isdigit()]
    exact_line_coverage = line_coverage(query, preview)
    natural_language, code_query = query_shape(terms, query)
    score = min(
        math.log1p(max(0.0, float(candidate.get("total_score", 0.0)))),
        4.0,
    ) / 4.0
    reciprocal_rank = 1.0 / (rank + 1.0)
    short_query = len(terms) <= 5
    long_query = len(terms) >= 13
    medium_query = not short_query and not long_query
    return [
        score,
        reciprocal_rank,
        min(float(candidate.get("hit_count", 0)), 4.0) / 4.0,
        min(len(sources), 5) / 5.0,
        float(lexical),
        float(semantic),
        float(literal),
        float("path" in sources),
        float("symbol" in sources),
        preview_coverage,
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
        score * preview_coverage,
        reciprocal_rank * preview_coverage,
        float(short_query) * preview_coverage,
        float(medium_query) * preview_coverage,
        float(long_query) * preview_coverage,
        float(short_query and semantic),
        float(long_query and semantic),
        float(short_query and literal),
        float(long_query and literal),
        preview_precision,
        preview_f1,
        weighted_coverage(terms, preview_terms),
        coverage(informative_terms, preview_lower),
        coverage(long_terms, preview_lower),
        coverage(numeric_terms, preview_lower),
        bigram_coverage(terms, preview_terms),
        exact_line_coverage,
        path_f1,
        natural_language * preview_f1,
        code_query * exact_line_coverage,
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
                        "query": queries[query_id],
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


def evaluation_report(
    examples: list[dict],
    provenance: list[dict],
    weights: list[float],
    minimum_relative_gain: float,
    maximum_task_loss: float,
) -> dict:
    baseline = evaluate(examples, None)
    learned = evaluate(examples, weights)
    relative_ndcg = learned["ndcg_at_10"] / baseline["ndcg_at_10"] - 1.0
    relative_mrr = learned["mrr_at_10"] / baseline["mrr_at_10"] - 1.0
    tasks = {}
    for dataset in sorted({example["dataset"] for example in examples}):
        task_examples = [
            example for example in examples if example["dataset"] == dataset
        ]
        task_baseline = evaluate(task_examples, None)
        task_learned = evaluate(task_examples, weights)
        tasks[dataset] = {
            "queries": len(task_examples),
            "baseline": task_baseline,
            "learned": task_learned,
            "ndcg_absolute_delta": (
                task_learned["ndcg_at_10"] - task_baseline["ndcg_at_10"]
            ),
            "mrr_absolute_delta": (
                task_learned["mrr_at_10"] - task_baseline["mrr_at_10"]
            ),
        }
    task_gate_passed = all(
        task["ndcg_absolute_delta"] >= -maximum_task_loss
        and task["mrr_absolute_delta"] >= -maximum_task_loss
        for task in tasks.values()
    )
    aggregate_gate_passed = (
        relative_ndcg >= minimum_relative_gain
        or relative_mrr >= minimum_relative_gain
    )
    return {
        "queries": len(examples),
        "sources": provenance,
        "baseline": baseline,
        "learned": learned,
        "relative_ndcg": relative_ndcg,
        "relative_mrr": relative_mrr,
        "tasks": tasks,
        "gate": {
            "minimum_relative_ndcg_or_mrr_gain": minimum_relative_gain,
            "maximum_absolute_task_loss": maximum_task_loss,
            "aggregate_passed": aggregate_gate_passed,
            "per_task_passed": task_gate_passed,
            "passed": aggregate_gate_passed and task_gate_passed,
        },
    }


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--pair", action="append", required=True)
    parser.add_argument("--evaluation-pair", action="append", default=[])
    parser.add_argument("--minimum-relative-gain", type=float, default=0.05)
    parser.add_argument("--maximum-task-loss", type=float, default=0.02)
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
        "schema_version": 2,
        "model_id": "public-linear-reranker-v2",
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
    if args.evaluation_pair:
        evaluation_pairs = [
            parse_pair(value) for value in args.evaluation_pair
        ]
        evaluation_examples, evaluation_provenance = load_examples(evaluation_pairs)
        report["evaluation"] = evaluation_report(
            evaluation_examples,
            evaluation_provenance,
            weights,
            args.minimum_relative_gain,
            args.maximum_task_loss,
        )
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    summary = {"training": report["training"]}
    if "evaluation" in report:
        summary["evaluation"] = report["evaluation"]
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if report.get("evaluation", {}).get("gate", {}).get("passed", True) else 1


if __name__ == "__main__":
    raise SystemExit(main())
