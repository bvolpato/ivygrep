#!/usr/bin/env python3

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(cmd, cwd=cwd, check=True)


def measure(repo_root: Path, ref: str, bench_target: str, bench_name: str) -> float:
    criterion_dir = repo_root / "target" / "criterion" / bench_name
    shutil.rmtree(criterion_dir, ignore_errors=True)

    run(["git", "checkout", "--detach", ref], cwd=repo_root)
    run(
        [
            "cargo",
            "bench",
            "--bench",
            bench_target,
            bench_name,
            "--",
            "--noplot",
        ],
        cwd=repo_root,
    )

    estimates_path = criterion_dir / "new" / "estimates.json"
    estimates = json.loads(estimates_path.read_text())
    return float(estimates["median"]["point_estimate"])


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare a critical Criterion benchmark against a baseline ref in the same runner."
    )
    parser.add_argument("--baseline-ref", required=True)
    parser.add_argument("--bench-target", default="indexer_bench")
    parser.add_argument("--bench-name", default="indexer/incremental_reindex_no_change")
    parser.add_argument("--threshold", type=float, default=1.15)
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    current_ref = (
        subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        .stdout.strip()
    )

    try:
        current = measure(repo_root, current_ref, args.bench_target, args.bench_name)
        baseline = measure(repo_root, args.baseline_ref, args.bench_target, args.bench_name)
    finally:
        run(["git", "checkout", "--detach", current_ref], cwd=repo_root)

    ratio = current / baseline if baseline else float("inf")
    print(
        json.dumps(
            {
                "bench": args.bench_name,
                "current_ref": current_ref,
                "baseline_ref": args.baseline_ref,
                "current_median_ns": current,
                "baseline_median_ns": baseline,
                "ratio": ratio,
                "threshold": args.threshold,
            },
            indent=2,
        )
    )

    if ratio > args.threshold:
        print(
            f"{args.bench_name} regressed by {ratio:.2f}x, exceeding threshold {args.threshold:.2f}x",
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
