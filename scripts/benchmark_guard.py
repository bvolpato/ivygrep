#!/usr/bin/env python3

import argparse
import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(cmd, cwd=cwd, check=True)


def output(cmd: list[str], cwd: Path) -> str:
    return subprocess.run(
        cmd,
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


@dataclass(frozen=True)
class Checkout:
    ref: str
    detached: bool


def current_checkout(repo_root: Path) -> Checkout:
    symbolic = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", "HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if symbolic.returncode == 0:
        return Checkout(symbolic.stdout.strip(), detached=False)
    return Checkout(output(["git", "rev-parse", "HEAD"], repo_root), detached=True)


def ensure_clean_worktree(repo_root: Path) -> None:
    if output(["git", "status", "--porcelain"], repo_root):
        raise SystemExit("benchmark guard requires a clean worktree")


def restore_checkout(repo_root: Path, checkout: Checkout) -> None:
    cmd = ["git", "checkout", "--quiet"]
    if checkout.detached:
        cmd.append("--detach")
    cmd.append(checkout.ref)
    run(cmd, cwd=repo_root)


def measure(repo_root: Path, ref: str, bench_target: str, bench_name: str) -> float:
    criterion_dir = repo_root / "target" / "criterion" / bench_name
    shutil.rmtree(criterion_dir, ignore_errors=True)

    run(["git", "checkout", "--quiet", "--detach", ref], cwd=repo_root)
    run(
        [
            "cargo",
            "bench",
            "--locked",
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


def ratio(current: float, baseline: float) -> float:
    return current / baseline if baseline else float("inf")


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
    ensure_clean_worktree(repo_root)
    original_checkout = current_checkout(repo_root)
    current_ref = output(["git", "rev-parse", "HEAD"], repo_root)

    try:
        current = measure(repo_root, current_ref, args.bench_target, args.bench_name)
        baseline = measure(repo_root, args.baseline_ref, args.bench_target, args.bench_name)
        initial_ratio = ratio(current, baseline)
        confirmation = None
        if initial_ratio > args.threshold:
            print(
                f"{args.bench_name} exceeded the threshold on the first pass; "
                "confirming in reverse order",
                file=sys.stderr,
            )
            confirmed_baseline = measure(
                repo_root, args.baseline_ref, args.bench_target, args.bench_name
            )
            confirmed_current = measure(
                repo_root, current_ref, args.bench_target, args.bench_name
            )
            confirmation = {
                "current_median_ns": confirmed_current,
                "baseline_median_ns": confirmed_baseline,
                "ratio": ratio(confirmed_current, confirmed_baseline),
            }
    finally:
        restore_checkout(repo_root, original_checkout)

    result = {
        "bench": args.bench_name,
        "current_ref": current_ref,
        "baseline_ref": args.baseline_ref,
        "current_median_ns": current,
        "baseline_median_ns": baseline,
        "ratio": initial_ratio,
        "threshold": args.threshold,
        "confirmation": confirmation,
    }
    print(json.dumps(result, indent=2))

    if confirmation is not None and confirmation["ratio"] > args.threshold:
        print(
            f"{args.bench_name} confirmed a regression at "
            f"{confirmation['ratio']:.2f}x, exceeding threshold "
            f"{args.threshold:.2f}x",
            file=sys.stderr,
        )
        return 1

    if confirmation is not None:
        print(
            f"{args.bench_name} was not confirmed as a regression "
            f"({initial_ratio:.2f}x then {confirmation['ratio']:.2f}x)",
            file=sys.stderr,
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
