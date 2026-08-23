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


def benchmark_binary(repo_root: Path, revision: str, bench_target: str) -> Path:
    cached_binary = repo_root / "target" / "benchmark-guard" / revision / bench_target
    if cached_binary.is_file():
        return cached_binary

    build = subprocess.run(
        [
            "cargo",
            "bench",
            "--locked",
            "--bench",
            bench_target,
            "--no-run",
            "--message-format=json",
        ],
        cwd=repo_root,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    for line in build.stdout.splitlines():
        artifact = json.loads(line)
        if (
            artifact.get("reason") == "compiler-artifact"
            and artifact.get("target", {}).get("name") == bench_target
            and artifact.get("executable")
        ):
            cached_binary.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(artifact["executable"], cached_binary)
            return cached_binary

    raise RuntimeError(f"cargo did not produce the {bench_target} benchmark executable")


def measure(repo_root: Path, ref: str, bench_target: str, bench_name: str) -> float:
    criterion_dir = repo_root / "target" / "criterion" / bench_name
    shutil.rmtree(criterion_dir, ignore_errors=True)

    run(["git", "checkout", "--quiet", "--detach", ref], cwd=repo_root)
    revision = output(["git", "rev-parse", "HEAD"], repo_root)
    binary = benchmark_binary(repo_root, revision, bench_target)
    run([str(binary), bench_name, "--bench", "--noplot"], cwd=repo_root)

    estimates_path = criterion_dir / "new" / "estimates.json"
    estimates = json.loads(estimates_path.read_text())
    return float(estimates["median"]["point_estimate"])


def ratio(current: float, baseline: float) -> float:
    return current / baseline if baseline else float("inf")


def write_result(path: Path, result: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare a critical Criterion benchmark against a baseline ref in the same runner."
    )
    parser.add_argument("--baseline-ref", required=True)
    parser.add_argument("--bench-target", default="indexer_bench")
    parser.add_argument("--bench-name", default="indexer/incremental_reindex_no_change")
    parser.add_argument("--threshold", type=float, default=1.15)
    parser.add_argument(
        "--max-median-ms",
        type=float,
        default=None,
        help=(
            "absolute budget for the head median; replaces the paired ratio for "
            "benchmarks too small to compare reliably on shared runners (the "
            "baseline is still measured for the report)"
        ),
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    max_median_ns = (
        args.max_median_ms * 1_000_000 if args.max_median_ms is not None else None
    )
    ensure_clean_worktree(repo_root)
    original_checkout = current_checkout(repo_root)
    current_ref = output(["git", "rev-parse", "HEAD"], repo_root)

    try:
        current = measure(repo_root, current_ref, args.bench_target, args.bench_name)
        baseline = measure(repo_root, args.baseline_ref, args.bench_target, args.bench_name)
        initial_ratio = ratio(current, baseline)
        confirmation = None
        if max_median_ns is not None:
            # Budget mode: identical code has measured 1.5-1.8x apart within one
            # job on shared runners for millisecond-scale benchmarks, so the
            # paired ratio carries no signal there. The budget catches the
            # regression class this guard exists for (the fast path doing real
            # work) and a second head measurement rules out a single bad sample.
            if current > max_median_ns:
                print(
                    f"{args.bench_name} exceeded its {args.max_median_ms:.2f} ms budget "
                    "on the first pass; measuring the head again",
                    file=sys.stderr,
                )
                confirmed_current = measure(
                    repo_root, current_ref, args.bench_target, args.bench_name
                )
                confirmation = {
                    "current_median_ns": confirmed_current,
                    "baseline_median_ns": baseline,
                    "ratio": ratio(confirmed_current, baseline),
                }
        elif initial_ratio > args.threshold:
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
        "max_median_ms": args.max_median_ms,
        "confirmation": confirmation,
    }
    if args.output is not None:
        write_result(args.output, result)
    print(json.dumps(result, indent=2))

    if max_median_ns is not None:
        if confirmation is not None and confirmation["current_median_ns"] > max_median_ns:
            print(
                f"{args.bench_name} confirmed over budget at "
                f"{confirmation['current_median_ns'] / 1_000_000:.2f} ms, exceeding "
                f"{args.max_median_ms:.2f} ms",
                file=sys.stderr,
            )
            return 1
        if confirmation is not None:
            print(
                f"{args.bench_name} was not confirmed over budget "
                f"({current / 1_000_000:.2f} ms then "
                f"{confirmation['current_median_ns'] / 1_000_000:.2f} ms)",
                file=sys.stderr,
            )
        return 0

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
