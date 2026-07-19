#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Compare indexed literal search with exact-search CLI baselines."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import time


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def percentile(values: list[float], quantile: float) -> float:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * quantile) - 1)]


def timed(command: list[str], cwd: Path, env: dict[str, str]) -> tuple[bytes, float]:
    started = time.perf_counter()
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    elapsed_ms = (time.perf_counter() - started) * 1_000.0
    if completed.returncode not in {0, 1}:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(command)}\n"
            f"{completed.stderr.decode(errors='replace')}"
        )
    return completed.stdout, elapsed_ms


def summarize(samples: list[float]) -> dict[str, float | int]:
    return {
        "samples": len(samples),
        "median_ms": statistics.median(samples),
        "p95_ms": percentile(samples, 0.95),
        "mean_ms": statistics.mean(samples),
        "minimum_ms": min(samples),
        "maximum_ms": max(samples),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--home", type=Path, required=True)
    parser.add_argument("--query", required=True)
    parser.add_argument("--samples", type=int, default=20)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.samples < 2:
        parser.error("--samples must be at least 2")

    binary = args.binary.resolve()
    repo = args.repo.resolve()
    home = args.home.resolve()
    if not shutil.which("rg"):
        raise RuntimeError("rg is required")

    env = {
        **os.environ,
        "IVYGREP_HOME": str(home),
        "IVYGREP_NO_AUTOSPAWN": "1",
        "IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT": "1",
    }
    commands = {
        "ivygrep_literal": [
            str(binary),
            "--literal",
            "--file-name-only",
            "--no-limit",
            "--json",
            "--",
            args.query,
            str(repo),
        ],
        "ripgrep": ["rg", "-F", "-l", "--", args.query, str(repo)],
        "git_grep": ["git", "grep", "-F", "-l", "--", args.query],
    }

    results = {}
    latency_samples = {name: [] for name in commands}
    for name, command in commands.items():
        stdout, _ = timed(command, repo, env)
        if name == "ivygrep_literal":
            matches = len(json.loads(stdout))
        else:
            matches = len(stdout.splitlines())
        results[name] = {
            "command": command,
            "matches": matches,
        }

    names = list(commands)
    for sample in range(args.samples):
        order = names if sample % 2 == 0 else list(reversed(names))
        for name in order:
            latency_samples[name].append(timed(commands[name], repo, env)[1])
    for name in names:
        results[name]["latency"] = summarize(latency_samples[name])

    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo,
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    ).stdout.strip()
    report = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "repo": {"path": str(repo), "commit": revision},
        "query": args.query,
        "binary": {"path": str(binary), "sha256": sha256(binary)},
        "tools": {
            "ivygrep": subprocess.run(
                [str(binary), "--version"],
                text=True,
                stdout=subprocess.PIPE,
                check=True,
            ).stdout.strip(),
            "ripgrep": subprocess.run(
                ["rg", "--version"],
                text=True,
                stdout=subprocess.PIPE,
                check=True,
            ).stdout.splitlines()[0],
            "git": subprocess.run(
                ["git", "--version"],
                text=True,
                stdout=subprocess.PIPE,
                check=True,
            ).stdout.strip(),
        },
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
