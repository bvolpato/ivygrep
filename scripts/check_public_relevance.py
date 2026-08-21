#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Enforce dataset-level public relevance and previously successful queries."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_GATES = ROOT / "benchmarks" / "public" / "relevance_gates.json"


def validate_matrix(matrix: dict, gates: dict, raw_results: Path) -> list[str]:
    errors: list[str] = []
    expected_profile = gates["profile"]
    if matrix.get("profile") != expected_profile:
        errors.append(
            f"benchmark profile {matrix.get('profile')!r} does not match "
            f"{expected_profile!r}"
        )

    actual_modes = set(matrix.get("modes", []))
    missing_modes = set(gates["required_modes"]) - actual_modes
    if missing_modes:
        errors.append("missing required retrieval modes: " + ", ".join(sorted(missing_modes)))

    summary = matrix.get("task_summary", {})
    results = matrix.get("results", [])
    for dataset, requirements in gates["datasets"].items():
        dataset_summary = summary.get(dataset)
        if not isinstance(dataset_summary, dict):
            errors.append(f"missing public benchmark dataset: {dataset}")
            continue

        for mode in gates["required_modes"]:
            metrics = dataset_summary.get(mode)
            if not isinstance(metrics, dict):
                errors.append(f"{dataset}/{mode}: missing public relevance measurement")
                continue
            for metric in ("ndcg_at_10", "recall_at_20"):
                minimum = requirements[f"minimum_{metric}"]
                value = metrics.get(metric, {}).get("mean")
                if (
                    isinstance(value, bool)
                    or not isinstance(value, (int, float))
                    or not math.isfinite(value)
                    or value < minimum
                ):
                    errors.append(f"{dataset}/{mode}: {metric}={value!r} < {minimum}")

            retained = set(requirements.get("retained_query_ids", []))
            if not retained or mode not in requirements.get("retention_modes", []):
                continue
            matching_runs = [
                result
                for result in results
                if result.get("dataset") == dataset and result.get("mode") == mode
            ]
            if not matching_runs:
                errors.append(f"{dataset}/{mode}: no raw result runs were recorded")
                continue
            for result in matching_runs:
                run = result.get("run")
                path = raw_results / f"{dataset}-{mode}-run-{run}.json"
                if not path.is_file():
                    errors.append(f"{dataset}/{mode}/run-{run}: missing raw result file")
                    continue
                details = json.loads(path.read_text(encoding="utf-8"))["details"]
                by_query = {str(detail["query_id"]): detail for detail in details}
                missing_queries = retained - set(by_query)
                if missing_queries:
                    errors.append(
                        f"{dataset}/{mode}/run-{run}: missing retained queries: "
                        + ", ".join(sorted(missing_queries))
                    )
                lost_queries = sorted(
                    query_id
                    for query_id in retained & set(by_query)
                    if by_query[query_id].get("recall_at_20", 0.0) <= 0.0
                )
                if lost_queries:
                    errors.append(
                        f"{dataset}/{mode}/run-{run}: lost every relevant result for "
                        + ", ".join(lost_queries)
                    )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", required=True, type=Path)
    parser.add_argument("--raw-results", required=True, type=Path)
    parser.add_argument("--gates", default=DEFAULT_GATES, type=Path)
    arguments = parser.parse_args()

    matrix = json.loads(arguments.matrix.read_text(encoding="utf-8"))
    gates = json.loads(arguments.gates.read_text(encoding="utf-8"))
    errors = validate_matrix(matrix, gates, arguments.raw_results)
    if errors:
        raise SystemExit("public retrieval relevance gate failed:\n" + "\n".join(errors))
    print(
        "public retrieval relevance gate passed "
        f"({len(gates['datasets'])} datasets, {len(gates['required_modes'])} modes)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
