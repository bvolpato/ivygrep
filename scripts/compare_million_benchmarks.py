#!/usr/bin/env python3
"""Compare frozen public million-chunk benchmark artifacts."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import random


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


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def compare(
    baseline: dict,
    current: dict,
    significant_regression_ratio: float,
    required_warm_ratio: float | None,
    required_index_ratio: float | None,
    maximum_quality_loss: float,
) -> dict:
    baseline_warm = baseline["queries"]["warm_distinct"]
    current_warm = current["queries"]["warm_distinct"]
    latency = bootstrap_p95_ratio(
        baseline_warm["latency_samples_ms"],
        current_warm["latency_samples_ms"],
    )
    latency["significant_regression"] = (
        latency["ci95_lower"] > significant_regression_ratio
    )

    baseline_throughput = baseline["index"]["chunks_per_second"]
    current_throughput = current["index"]["chunks_per_second"]
    index_ratio = (
        current_throughput / baseline_throughput
        if baseline_throughput and current_throughput
        else None
    )
    quality_loss = (
        baseline_warm["expected_recall_at_20"]
        - current_warm["expected_recall_at_20"]
    )

    failures = []
    if latency["significant_regression"]:
        failures.append(
            "warm distinct-query p95 has a statistically significant regression"
        )
    if required_warm_ratio is not None and latency["observed"] > required_warm_ratio:
        failures.append(
            f"warm p95 ratio {latency['observed']:.3f} exceeds "
            f"required {required_warm_ratio:.3f}"
        )
    if (
        required_index_ratio is not None
        and (index_ratio is None or index_ratio < required_index_ratio)
    ):
        failures.append(
            f"index throughput ratio {index_ratio or 0.0:.3f} is below "
            f"required {required_index_ratio:.3f}"
        )
    if quality_loss > maximum_quality_loss:
        failures.append(
            f"expected recall@20 loss {quality_loss:.4f} exceeds "
            f"allowed {maximum_quality_loss:.4f}"
        )

    return {
        "schema_version": 1,
        "baseline_commit": baseline["ivygrep_commit"],
        "current_commit": current["ivygrep_commit"],
        "warm_distinct_p95_ratio": latency,
        "index_throughput_ratio": index_ratio,
        "expected_recall_at_20_loss": quality_loss,
        "passed": not failures,
        "failures": failures,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--current", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--significant-regression-ratio", type=float, default=1.15)
    parser.add_argument("--require-warm-p95-ratio", type=float)
    parser.add_argument("--require-index-throughput-ratio", type=float)
    parser.add_argument("--maximum-quality-loss", type=float, default=0.0)
    args = parser.parse_args()

    result = compare(
        load(args.baseline),
        load(args.current),
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
