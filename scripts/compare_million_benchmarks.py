#!/usr/bin/env python3
"""Compare frozen public million-chunk benchmark artifacts."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import random
import statistics


def percentile(values: list[float], quantile: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * quantile) - 1))
    return ordered[index]


def bootstrap_p95_ratio(
    baseline: list[float],
    current: list[float],
    repetitions: int = 5_000,
) -> dict:
    rng = random.Random(20260615)
    ratios = []
    for _ in range(repetitions):
        baseline_sample = [rng.choice(baseline) for _ in baseline]
        current_sample = [rng.choice(current) for _ in current]
        baseline_p95 = percentile(baseline_sample, 0.95)
        current_p95 = percentile(current_sample, 0.95)
        ratios.append(current_p95 / baseline_p95)
    return {
        "observed": percentile(current, 0.95) / percentile(baseline, 0.95),
        "ci95_lower": percentile(ratios, 0.025),
        "ci95_upper": percentile(ratios, 0.975),
        "bootstrap_repetitions": repetitions,
    }


def bootstrap_median_ratio(
    baseline: list[float],
    current: list[float],
    repetitions: int = 5_000,
) -> dict:
    rng = random.Random(20260615)
    ratios = []
    for _ in range(repetitions):
        baseline_sample = [rng.choice(baseline) for _ in baseline]
        current_sample = [rng.choice(current) for _ in current]
        ratios.append(
            statistics.median(current_sample) / statistics.median(baseline_sample)
        )
    return {
        "observed": statistics.median(current) / statistics.median(baseline),
        "ci95_lower": percentile(ratios, 0.025),
        "ci95_upper": percentile(ratios, 0.975),
        "bootstrap_repetitions": repetitions,
    }


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def compare_runs(
    baseline_runs: list[dict],
    current_runs: list[dict],
    significant_regression_ratio: float,
    required_warm_ratio: float | None,
    required_index_ratio: float | None,
    maximum_quality_loss: float,
) -> dict:
    if not baseline_runs or not current_runs:
        raise ValueError("at least one baseline and current run is required")
    baseline_warm = [run["queries"]["warm_distinct"] for run in baseline_runs]
    current_warm = [run["queries"]["warm_distinct"] for run in current_runs]
    latency = bootstrap_p95_ratio(
        [sample for run in baseline_warm for sample in run["latency_samples_ms"]],
        [sample for run in current_warm for sample in run["latency_samples_ms"]],
    )
    latency["significant_regression"] = (
        latency["ci95_lower"] > significant_regression_ratio
    )

    baseline_throughputs = [
        run["index"]["chunks_per_second"]
        for run in baseline_runs
        if run["index"]["chunks_per_second"] is not None
    ]
    current_throughputs = [
        run["index"]["chunks_per_second"]
        for run in current_runs
        if run["index"]["chunks_per_second"] is not None
    ]
    index_ratio = (
        bootstrap_median_ratio(baseline_throughputs, current_throughputs)
        if baseline_throughputs and current_throughputs
        else None
    )
    if index_ratio is not None:
        index_ratio["significant_regression"] = (
            index_ratio["ci95_upper"] < 1.0 / significant_regression_ratio
        )
    baseline_recall = statistics.median(
        [run["expected_recall_at_20"] for run in baseline_warm]
    )
    current_recall = statistics.median(
        [run["expected_recall_at_20"] for run in current_warm]
    )
    quality_loss = baseline_recall - current_recall

    failures = []
    if latency["significant_regression"]:
        failures.append(
            "warm distinct-query p95 has a statistically significant regression"
        )
    if index_ratio is not None and index_ratio["significant_regression"]:
        failures.append(
            "fresh-index throughput has a statistically significant regression"
        )
    if required_warm_ratio is not None and latency["observed"] > required_warm_ratio:
        failures.append(
            f"warm p95 ratio {latency['observed']:.3f} exceeds "
            f"required {required_warm_ratio:.3f}"
        )
    if required_index_ratio is not None and (
        index_ratio is None or index_ratio["observed"] < required_index_ratio
    ):
        failures.append(
            f"index throughput ratio "
            f"{index_ratio['observed'] if index_ratio else 0.0:.3f} is below "
            f"required {required_index_ratio:.3f}"
        )
    if quality_loss > maximum_quality_loss:
        failures.append(
            f"expected recall@20 loss {quality_loss:.4f} exceeds "
            f"allowed {maximum_quality_loss:.4f}"
        )

    return {
        "schema_version": 1,
        "baseline_commit": baseline_runs[0]["ivygrep_commit"],
        "current_commit": current_runs[0]["ivygrep_commit"],
        "baseline_runs": len(baseline_runs),
        "current_runs": len(current_runs),
        "warm_distinct_p95_ratio": latency,
        "index_throughput_ratio": index_ratio,
        "expected_recall_at_20_loss": quality_loss,
        "passed": not failures,
        "failures": failures,
    }


def compare(
    baseline: dict,
    current: dict,
    significant_regression_ratio: float,
    required_warm_ratio: float | None,
    required_index_ratio: float | None,
    maximum_quality_loss: float,
) -> dict:
    return compare_runs(
        [baseline],
        [current],
        significant_regression_ratio,
        required_warm_ratio,
        required_index_ratio,
        maximum_quality_loss,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, nargs="+", required=True)
    parser.add_argument("--current", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--significant-regression-ratio", type=float, default=1.15)
    parser.add_argument("--require-warm-p95-ratio", type=float)
    parser.add_argument("--require-index-throughput-ratio", type=float)
    parser.add_argument("--maximum-quality-loss", type=float, default=0.0)
    args = parser.parse_args()

    result = compare_runs(
        [load(path) for path in args.baseline],
        [load(path) for path in args.current],
        args.significant_regression_ratio,
        args.require_warm_p95_ratio,
        args.require_index_throughput_ratio,
        args.maximum_quality_loss,
    )
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
