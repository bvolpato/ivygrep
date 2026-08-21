#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Run the previously regressed public queries against their original corpora."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_GATES = ROOT / "benchmarks" / "public" / "relevance_gates.json"


def selected_datasets(gates: dict) -> list[tuple[str, dict]]:
    return [
        (dataset, requirements)
        for dataset, requirements in gates["datasets"].items()
        if requirements.get("retained_query_ids")
    ]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--datasets-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--gates", default=DEFAULT_GATES, type=Path)
    parser.add_argument("--skip-export", action="store_true")
    arguments = parser.parse_args()

    gates = json.loads(arguments.gates.read_text(encoding="utf-8"))
    datasets = selected_datasets(gates)
    if not datasets:
        raise SystemExit("public relevance canary has no retained queries")

    if not arguments.skip_export:
        command = [
            "uv",
            "run",
            str(ROOT / "scripts" / "export_public_retrieval.py"),
            "--profile",
            gates["profile"],
            "--output",
            str(arguments.datasets_root),
        ]
        for dataset, _ in datasets:
            command.extend(["--task", dataset])
        subprocess.run(command, cwd=ROOT, check=True)

    work_root = arguments.output.parent / "public-relevance-canary-runs"
    work_root.mkdir(parents=True, exist_ok=True)
    results = []
    for dataset, requirements in datasets:
        for mode in gates["canary_modes"]:
            output = work_root / f"{dataset}-{mode}.json"
            command = [
                sys.executable,
                str(ROOT / "scripts" / "eval_code_retrieval.py"),
                "--dataset",
                str(arguments.datasets_root / dataset),
                "--binary",
                str(arguments.binary.resolve()),
                "--mode",
                mode,
                "--min-ndcg-at-10",
                str(requirements["canary_minimum_ndcg_at_10"]),
                "--min-recall-at-20",
                "1.0",
                "--require-relevant-results",
                "--output",
                str(output),
            ]
            for query_id in requirements["retained_query_ids"]:
                command.extend(["--query-id", query_id])
            subprocess.run(command, cwd=ROOT, check=True)
            measured = json.loads(output.read_text(encoding="utf-8"))
            results.append(
                {
                    "dataset": dataset,
                    "mode": mode,
                    "queries": measured["queries"],
                    "ndcg_at_10": measured["ndcg_at_10"],
                    "recall_at_20": measured["recall_at_20"],
                    "retained_query_ids": requirements["retained_query_ids"],
                    "dataset_provenance": measured["dataset_provenance"],
                }
            )

    report = {
        "schema_version": 1,
        "profile": gates["profile"],
        "binary": results and measured["binary"],
        "results": results,
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        "public retrieval canary passed "
        f"({len(datasets)} datasets, {len(gates['canary_modes'])} modes, "
        f"{sum(len(item['retained_query_ids']) for _, item in datasets)} retained queries)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
