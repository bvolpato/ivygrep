#!/usr/bin/env python3
"""Train a compact linear reranker from disjoint public retrieval traces."""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import random
import subprocess

import eval_code_retrieval
import public_retrieval_contracts as contracts


FEATURE_NAMES = contracts.RERANK_FEATURE_SCHEMA
# Features computed from the candidate's file path, or set by the path search
# pass. The public exporter writes every document to
# `documents/<position>.<extension>`, so on those corpora they describe the
# export, not the document: `primary_source` is the dataset's language tag, and
# path coverage is a number in the query that happens to occur in a position.
# In a repository the same features separate code from docs and templates, so a
# weight fit on exported paths ranks real files by noise.
PATH_FEATURES = (
    "source_path",
    "query_path_coverage",
    "exact_query_path",
    "support_path",
    "primary_source",
    "shallow_path",
    "path_term_f1",
)
PATH_FEATURE_INDEXES = tuple(FEATURE_NAMES.index(name) for name in PATH_FEATURES)


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


def model_weights_sha256(feature_schema, weights) -> str:
    """Identity of a ranking function: the feature order and the weights. Every
    metrics record in a model file names the function it was computed for, so a
    later edit of the weights cannot keep a record it did not produce."""
    return contracts.canonical_sha256(
        {
            "feature_schema": [str(name) for name in feature_schema],
            "weights": [float(weight) for weight in weights],
        }
    )


def synthetic_corpus_paths(paths) -> bool:
    """Whether every document sits in the flat `documents/` directory that the
    public exporter and the evaluator's fallback write, not in a repository
    layout."""
    paths = list(paths)
    return bool(paths) and all(
        PurePosixPath(path).parent.as_posix() == "documents" for path in paths
    )


def fixed_zero_features(examples: list[dict]) -> dict | None:
    """Name the path features when no fit example was allowed to move them."""
    if not all(example.get("synthetic_paths") for example in examples):
        return None
    return {
        "rule": "synthetic-corpus-paths",
        "features": sorted(PATH_FEATURES),
        "reason": (
            "Every fit corpus stores its documents at synthetic paths, so features "
            "computed from the file path carry no relevance information there. "
            "Their pair differences are left out of the fit and their weights stay zero."
        ),
    }


def parse_pair(value: str) -> tuple[Path, Path]:
    dataset, separator, result = value.partition("=")
    if not separator:
        raise ValueError(f"expected DATASET=RESULT, got {value!r}")
    return Path(dataset), Path(result)


def load_examples(pairs: list[tuple[Path, Path]]) -> tuple[list[dict], list[dict]]:
    examples = []
    provenance = []
    datasets_seen = set()
    for dataset, result_path in pairs:
        result = json.loads(result_path.read_text(encoding="utf-8"))
        if dataset.name in datasets_seen:
            raise ValueError("training requires one native capture result per dataset")
        datasets_seen.add(dataset.name)
        capture_contract = result.get("native_capture_contract") or {}
        if (
            type(capture_contract.get("schema_version")) is not int
            or capture_contract.get("schema_version") != 1
            or capture_contract.get("stage") != contracts.CAPTURE_STAGE
            or capture_contract.get("transport") != "fresh-process-stderr"
            or capture_contract.get("ranking_context_lines") != 2
            or capture_contract.get("feature_schema") != list(FEATURE_NAMES)
        ):
            raise ValueError(
                "native pre-learned capture is required; legacy deterministic or learned grouped scores "
                "are ambiguous. Recollect with --capture-reranker and a capture-capable binary"
            )
        request = result.get("execution_provenance", {}).get("request", {})
        if (
            not request.get("options", {}).get("capture_reranker")
            or result.get("query_expansion") != "none"
            or result.get("measurement_scope") != "native-training-capture"
        ):
            raise ValueError(
                "training requires explicit native capture without query expansion"
            )
        contracts.validate_execution(result, request)
        if (
            result.get("dataset") != dataset.name
            or request.get("dataset") != dataset.name
            or contracts.dataset_fingerprint(dataset) != request.get("dataset_content")
        ):
            raise ValueError("training dataset differs from the native capture inputs")
        if result.get("query_text_limit") != request["options"]["max_query_chars"]:
            raise ValueError("training query limit differs from native execution")
        query_rows = eval_code_retrieval.selected_queries(
            eval_code_retrieval.load_jsonl(dataset / "queries.jsonl"),
            request["options"]["query_id"],
        )
        queries = {str(query["_id"]): query for query in query_rows}
        details = result.get("details", [])
        detail_ids = [str(detail["query_id"]) for detail in details]
        if (
            len(queries) != len(query_rows)
            or len(detail_ids) != len(set(detail_ids))
            or set(detail_ids) != set(queries)
            or result.get("queries") != len(details)
        ):
            raise ValueError(
                "native capture query records are incomplete or duplicated"
            )
        receipt_name = capture_contract.get("receipt_directory", "")
        if (
            not receipt_name
            or Path(receipt_name).name != receipt_name
            or receipt_name in {".", ".."}
        ):
            raise ValueError(
                "native capture receipt directory must be a sibling basename"
            )
        receipts = result_path.parent / receipt_name
        path_to_id = eval_code_retrieval.corpus_path_map(dataset)
        synthetic_paths = synthetic_corpus_paths(path_to_id)
        source_provenance = json.loads(
            (dataset / "provenance.json").read_text(encoding="utf-8")
        )
        query_repository = (source_provenance.get("query_corpus") or {}).get(
            "repository", f"dataset:{dataset.name}"
        )
        qrels = eval_code_retrieval.load_qrels(dataset / "qrels.tsv")
        skipped = []
        fit_ids = []
        for query_number, detail in enumerate(details):
            query_id = str(detail["query_id"])
            text = eval_code_retrieval.query_text(
                queries[query_id], result.get("query_text_limit")
            )
            capture = detail.get("native_capture") or {}
            name = capture.get("receipt_name", "")
            if name != f"q{query_number:06d}":
                raise ValueError("native capture receipt name is invalid")
            receipt = receipts / name
            command = json.loads(
                receipt.with_suffix(".command.json").read_text(encoding="utf-8")
            )
            exit_status = json.loads(
                receipt.with_suffix(".exit.json").read_text(encoding="utf-8")
            )
            stderr_path = receipt.with_suffix(".stderr.log")
            stdout_path = receipt.with_suffix(".stdout.json")
            if (
                command.get("process_id") != capture.get("process_id")
                or command.get("query") != text
                or exit_status
                != {"process_id": capture.get("process_id"), "returncode": 0}
                or sha256_file(stderr_path) != capture.get("stderr_sha256")
                or sha256_file(stdout_path) != capture.get("stdout_sha256")
            ):
                raise ValueError(
                    "native capture process/raw-output provenance is missing or inconsistent"
                )
            record = contracts.parse_native_capture(
                stderr_path.read_text(encoding="utf-8"), text, capture["process_id"]
            )
            if record != capture.get("record"):
                raise ValueError(
                    "native capture features differ from the original stderr record"
                )
            document_ids = eval_code_retrieval.captured_document_ids(
                record, Path(command["cwd"]), path_to_id
            )
            if document_ids != capture.get("candidate_document_ids"):
                raise ValueError(
                    "native capture document mapping differs from the indexed corpus"
                )
            if record["status"] == "skipped":
                skipped.append({"query_id": query_id, "reason": record["reason"]})
                continue
            if record["model_id"] != result["index_configuration"].get(
                "reranker_model"
            ):
                raise ValueError(
                    "native capture model differs from observed runtime identity"
                )
            candidates = []
            for candidate, document_id in zip(
                record["candidates"], document_ids, strict=True
            ):
                candidates.append(
                    {
                        "document_id": document_id,
                        "features": list(candidate["native_features"]),
                        "grade": qrels.get(query_id, {}).get(document_id, 0),
                        "rank": candidate["baseline_rank"],
                    }
                )
            if candidates:
                examples.append(
                    {
                        "dataset": dataset.name,
                        "query_repository": query_repository,
                        "query_id": query_id,
                        "query": text,
                        "candidates": candidates,
                        "judgments": qrels.get(query_id, {}),
                        "synthetic_paths": synthetic_paths,
                    }
                )
                fit_ids.append(query_id)
        if (
            capture_contract.get("applied_queries") != len(fit_ids)
            or capture_contract.get("skipped_queries") != len(skipped)
            or capture_contract.get("skip_reasons")
            != dict(Counter(row["reason"] for row in skipped))
        ):
            raise ValueError(
                "native capture eligibility totals differ from recorded queries"
            )
        provenance.append(
            {
                "dataset": dataset.name,
                "dataset_provenance_sha256": sha256_file(dataset / "provenance.json"),
                "dataset_provenance_canonical_sha256": contracts.pretty_json_sha256(
                    source_provenance
                ),
                "result_sha256": sha256_file(result_path),
                "binary": result["binary"],
                "queries": len(fit_ids),
                "query_repository": query_repository,
                "observed_queries": result["queries"],
                "fit_query_ids": sorted(fit_ids),
                "skipped_queries": skipped,
                "native_capture_schema_version": 1,
                "synthetic_corpus_paths": synthetic_paths,
            }
        )
    return examples, provenance


def ensure_fit_disjoint(training: list[dict], evaluation: list[dict]) -> None:
    def key(example):
        return (
            example.get("query_repository", example["dataset"]),
            example["query_id"],
        )

    overlap = {key(example) for example in training} & {
        key(example) for example in evaluation
    }
    if overlap:
        raise ValueError(
            f"evaluation overlaps {len(overlap)} actual native model-fit query IDs"
        )


def write_training_json(path: Path, value: dict) -> None:
    """Keep checksum-bound model and ledger bytes identical across platforms."""
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def write_fit_ledger(
    report: dict, model_path: Path, pairs: list[tuple[Path, Path]], output: Path
) -> None:
    sources = {source["dataset"]: source for source in report["training"]["sources"]}
    records = []
    for dataset, _ in pairs:
        source = sources[dataset.name]
        provenance = json.loads(
            (dataset / "provenance.json").read_text(encoding="utf-8")
        )
        if not (provenance.get("query_corpus") or {}).get("repository"):
            raise ValueError(
                "fit-ledger output requires repository-qualified query provenance"
            )
        if (
            sha256_file(dataset / "provenance.json")
            != source["dataset_provenance_sha256"]
            or contracts.pretty_json_sha256(provenance)
            != source["dataset_provenance_canonical_sha256"]
        ):
            raise ValueError(
                "training dataset provenance changed before fit-ledger output"
            )
        ids = source["fit_query_ids"]
        records.append(
            {
                "dataset": dataset.name,
                "dataset_provenance_sha256": source["dataset_provenance_sha256"],
                "result_sha256": source["result_sha256"],
                "provenance": provenance,
                "query_ids": ids,
                "query_ids_sha256": contracts.pretty_json_sha256(ids),
            }
        )
    ledger = {
        "schema_version": 1,
        "model_id": report["model_id"],
        "model_sha256": sha256_file(model_path),
        "model_training_commit": report["training"]["ivygrep_commit"],
        "queries": report["training"]["queries"],
        "sources": records,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    write_training_json(output, ledger)


def split_examples(examples: list[dict]) -> tuple[list[dict], list[dict]]:
    train = []
    validation = []
    for example in examples:
        key = f"{example['dataset']}:{example['query_id']}".encode()
        bucket = int.from_bytes(hashlib.sha256(key).digest()[:4], "big") % 5
        (validation if bucket == 0 else train).append(example)
    return train, validation


def ranking_features(example: dict, candidate: dict) -> list[float]:
    """The candidate's features as the fit and every evaluation of it see them.
    Path features of a corpus with synthetic paths are zero (see PATH_FEATURES):
    they must not move a weight, and when a fit that also had a real-path corpus
    gave them weights, a file name like `documents/000000123.py` must not pick
    the hyperparameters or pass the acceptance gate either."""
    features = candidate["features"]
    if not example.get("synthetic_paths"):
        return features
    features = list(features)
    for index in PATH_FEATURE_INDEXES:
        features[index] = 0.0
    return features


def training_pairs(examples: list[dict]) -> list[tuple[list[float], float]]:
    pairs = []
    for example in examples:
        candidates = [
            (candidate["grade"], ranking_features(example, candidate))
            for candidate in example["candidates"]
        ]
        for preferred_grade, preferred in candidates:
            for other_grade, other in candidates:
                grade_delta = preferred_grade - other_grade
                if grade_delta <= 0:
                    continue
                pairs.append(
                    (
                        [
                            left - right
                            for left, right in zip(preferred, other, strict=True)
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


def score_candidate(example: dict, candidate: dict, weights: list[float]) -> float:
    return sum(
        weight * value
        for weight, value in zip(
            weights, ranking_features(example, candidate), strict=True
        )
    )


def evaluate(examples: list[dict], weights: list[float] | None) -> dict[str, float]:
    scores = []
    for example in examples:
        candidates = list(example["candidates"])
        if weights is not None:
            candidates.sort(
                key=lambda candidate: (
                    -score_candidate(example, candidate, weights),
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
        "weights_sha256": model_weights_sha256(FEATURE_NAMES, weights),
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


FIT_METRICS = (
    "baseline_validation",
    "learned_validation",
    "baseline_all",
    "learned_all",
)


def reevaluate_model(
    model: dict,
    ledger: dict,
    pairs: list[tuple[Path, Path]],
    minimum_relative_gain: float,
    maximum_task_loss: float,
    evaluated_at: str,
) -> dict:
    """Replace the model's evaluation record with one computed for its weights.

    For weights that changed after the fit (see fixed_zero_features). Records
    computed for other weights move to `original_fit`, labelled as history."""
    if model.get("feature_schema") != list(FEATURE_NAMES):
        raise ValueError("model feature schema differs from this trainer")
    weights = [float(weight) for weight in model["weights"]]
    current = model_weights_sha256(FEATURE_NAMES, weights)
    examples, provenance = load_examples(pairs)
    if not examples:
        raise ValueError("evaluation captures contain no applied candidate pools")
    overlap = contracts.audit_fit_queries(
        ledger, [dataset for dataset, _ in pairs], "fit-disjoint-diagnostic"
    )
    evaluation = evaluation_report(
        examples, provenance, weights, minimum_relative_gain, maximum_task_loss
    )
    commits = set()
    for source, (_, result_path) in zip(evaluation["sources"], pairs, strict=True):
        result = json.loads(result_path.read_text(encoding="utf-8"))
        commits.add(result["execution_provenance"]["source_commit"])
        # The result checksum binds the query IDs; an embedded model stays small.
        source["skipped_queries"] = dict(
            Counter(row["reason"] for row in source["skipped_queries"])
        )
        del source["fit_query_ids"]
    evaluation["scope"] = (
        "Evaluation of the weights in this file on the native captures named in "
        "sources, made with --reevaluate and not as part of a fit."
    )
    evaluation["evaluated_at"] = evaluated_at
    evaluation["capture_commits"] = sorted(commits)
    evaluation["fit_overlap_queries"] = overlap["overlap_queries"]

    previous = model.get("evaluation")
    stale_fit_metrics = {
        name: model["training"].pop(name)
        for name in FIT_METRICS
        if name in model["training"]
        and model["training"].get("weights_sha256") != current
    }
    if "original_fit" not in model and (
        stale_fit_metrics
        or (previous is not None and previous.get("weights_sha256") != current)
    ):
        fitted = list(weights)
        replaced = (model.get("fixed_zero_features") or {}).get(
            "replaced_fitted_weights", {}
        )
        for name, value in replaced.items():
            fitted[FEATURE_NAMES.index(name)] = float(value)
        model["original_fit"] = {
            "note": (
                "History. These records were computed for the weights the fit "
                "produced (weights_sha256 below), before fixed_zero_features was "
                "applied. They do not describe the weights in this file."
            ),
            "weights_sha256": model_weights_sha256(FEATURE_NAMES, fitted),
            "training_metrics": stale_fit_metrics,
            "evaluation": previous,
        }
    model["evaluation"] = evaluation
    return model


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("--pair", action="append", default=[])
    parser.add_argument("--evaluation-pair", action="append", default=[])
    parser.add_argument("--minimum-relative-gain", type=float, default=0.05)
    parser.add_argument("--maximum-task-loss", type=float, default=0.02)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--fit-ledger-output",
        type=Path,
        help="Write the exact native fit-ID ledger bound to the newly written model.",
    )
    parser.add_argument(
        "--reevaluate",
        type=Path,
        metavar="MODEL",
        help=(
            "Fit nothing. Evaluate MODEL's weights on the --evaluation-pair captures, "
            "write the model with that evaluation record to --output, and bind "
            "--fit-ledger to the new model bytes."
        ),
    )
    parser.add_argument(
        "--fit-ledger",
        type=Path,
        help="With --reevaluate: MODEL's fit ledger, rewritten in place.",
    )
    args = parser.parse_args()
    if args.reevaluate:
        if args.pair or not args.evaluation_pair or not args.fit_ledger:
            parser.error(
                "--reevaluate takes --evaluation-pair and --fit-ledger, and no --pair"
            )
        ledger = contracts.load_fit_ledger(
            args.reevaluate, args.fit_ledger, contracts.sha256_file(args.fit_ledger)
        )
        model = reevaluate_model(
            json.loads(args.reevaluate.read_text(encoding="utf-8")),
            ledger,
            [parse_pair(value) for value in args.evaluation_pair],
            args.minimum_relative_gain,
            args.maximum_task_loss,
            datetime.now(timezone.utc).date().isoformat(),
        )
        write_training_json(args.output, model)
        ledger["model_sha256"] = sha256_file(args.output)
        write_training_json(args.fit_ledger, ledger)
        print(
            json.dumps(
                {
                    "evaluation": model["evaluation"],
                    "fit_ledger_sha256": sha256_file(args.fit_ledger),
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0 if model["evaluation"]["gate"]["passed"] else 1
    if not args.pair:
        parser.error("--pair is required")
    pairs = [parse_pair(value) for value in args.pair]
    examples, provenance = load_examples(pairs)
    if not examples:
        raise ValueError(
            "native captures contain no applied pre-learned candidate pools; inspect recorded skipped reasons"
        )
    evaluation_examples = None
    evaluation_provenance = None
    if args.evaluation_pair:
        evaluation_examples, evaluation_provenance = load_examples(
            [parse_pair(value) for value in args.evaluation_pair]
        )
        ensure_fit_disjoint(examples, evaluation_examples)
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
            "weights_sha256": model_weights_sha256(FEATURE_NAMES, weights),
            "queries": len(examples),
            "train_queries": len(train),
            "validation_queries": len(validation),
            "sources": provenance,
            "candidate_scope": "native pre-learned accepted files; skipped runtime routes are recorded, not reconstructed",
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
    fixed = fixed_zero_features(examples)
    if fixed is not None:
        report["fixed_zero_features"] = fixed
    if evaluation_examples is not None:
        report["evaluation"] = evaluation_report(
            evaluation_examples,
            evaluation_provenance,
            weights,
            args.minimum_relative_gain,
            args.maximum_task_loss,
        )
    write_training_json(args.output, report)
    if args.fit_ledger_output:
        write_fit_ledger(report, args.output, pairs, args.fit_ledger_output)
    summary = {"training": report["training"]}
    if "evaluation" in report:
        summary["evaluation"] = report["evaluation"]
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if report.get("evaluation", {}).get("gate", {}).get("passed", True) else 1


if __name__ == "__main__":
    raise SystemExit(main())
