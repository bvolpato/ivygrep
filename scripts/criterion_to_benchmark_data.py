#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Convert cargo-criterion JSON output to github-action-benchmark input."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def benchmark_estimate_ns(obj: dict[str, Any]) -> float | None:
    for key in ("median", "mean", "typical"):
        metric = obj.get(key)
        if not isinstance(metric, dict):
            continue
        estimate = metric.get("estimate", metric.get("point_estimate"))
        if isinstance(estimate, int | float):
            return float(estimate)
    return None


def convert(path: Path) -> list[dict[str, Any]]:
    data: list[dict[str, Any]] = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if obj.get("reason") != "benchmark-complete":
            continue

        name = obj.get("id")
        if not isinstance(name, str) or not name:
            raise SystemExit(f"{path}:{line_no}: benchmark-complete missing id")

        estimate_ns = benchmark_estimate_ns(obj)
        if estimate_ns is None:
            raise SystemExit(f"{path}:{line_no}: {name} missing median/mean estimate")

        data.append({"name": name, "value": round(estimate_ns / 1000, 2), "unit": "\u00b5s"})
    return data


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--min-benchmarks", type=int, default=1)
    args = parser.parse_args()

    data = convert(args.input)
    if len(data) < args.min_benchmarks:
        raise SystemExit(
            f"expected at least {args.min_benchmarks} benchmark results, got {len(data)}"
        )

    args.output.write_text(json.dumps(data), encoding="utf-8")
    print(f"wrote {len(data)} benchmark results to {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
