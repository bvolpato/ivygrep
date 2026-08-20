#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Measure or validate versioned, self-contained retrieval evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests" / "fixtures" / "ivygrep_relevance_queries.json"
HARNESS = ROOT / "scripts" / "eval_relevance.py"
DEFAULT_OUTPUT = ROOT / "docs" / "benchmarks" / "current-head-relevance.json"
SOURCE_INPUTS = (
    ".cargo",
    "assets",
    "benchmarks/public/reranker_model.json",
    "Cargo.lock",
    "Cargo.toml",
    "build.rs",
    "rust-toolchain.toml",
    "src",
    "vendor",
)

MIN_RELEVANCE_METRICS = {
    "mean_ndcg10": 0.85,
    "mean_mrr": 0.90,
    "mean_candidate_recall": 0.95,
}


def sha256_file(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def package_version(root: Path = ROOT) -> str:
    with (root / "Cargo.toml").open("rb") as handle:
        return tomllib.load(handle)["package"]["version"]


def source_inputs_sha256(root: Path = ROOT) -> str:
    paths = []
    for name in SOURCE_INPUTS:
        path = root / name
        if path.is_file():
            paths.append(path)
        elif path.is_dir():
            paths.extend(candidate for candidate in path.rglob("*") if candidate.is_file())

    digest = hashlib.sha256()
    for path in sorted(paths, key=lambda candidate: candidate.relative_to(root).as_posix()):
        digest.update(path.relative_to(root).as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest()


def validate_report(report: dict, *, root: Path = ROOT) -> list[str]:
    errors = []
    expected = f"ivygrep {package_version(root)}"
    actual = report.get("binary", {}).get("version")
    if actual != expected:
        errors.append(f"benchmark binary {actual!r} does not match {expected!r}")

    fixture = root / "tests" / "fixtures" / "ivygrep_relevance_queries.json"
    harness = root / "scripts" / "eval_relevance.py"
    for name, path in (("fixture", fixture), ("harness", harness)):
        actual_digest = report.get(name, {}).get("sha256")
        expected_digest = sha256_file(path)
        if actual_digest != expected_digest:
            errors.append(f"{name} SHA-256 no longer matches {path.relative_to(root)}")

    if report.get("source", {}).get("sha256") != source_inputs_sha256(root):
        errors.append("benchmark-relevant source SHA-256 no longer matches the current tree")

    expected_queries = len(json.loads(fixture.read_text(encoding="utf-8"))["queries"])
    modes = report.get("modes", {})
    for mode in ("foreground", "hash-enriched"):
        measured = modes.get(mode)
        if not isinstance(measured, dict):
            errors.append(f"missing {mode} relevance measurement")
        elif measured.get("queries") != expected_queries:
            errors.append(
                f"{mode} measured {measured.get('queries')} queries instead of "
                f"the {expected_queries} current fixture queries"
            )
        else:
            for metric, minimum in MIN_RELEVANCE_METRICS.items():
                value = measured.get(metric)
                if (
                    isinstance(value, bool)
                    or not isinstance(value, (int, float))
                    or not math.isfinite(value)
                    or value < minimum
                ):
                    errors.append(f"{mode} {metric} {value!r} is below minimum {minimum}")

            no_hit_queries = measured.get("no_hit_queries")
            if (
                isinstance(no_hit_queries, bool)
                or not isinstance(no_hit_queries, int)
                or no_hit_queries != 0
            ):
                errors.append(f"{mode} reported {no_hit_queries!r} no-hit queries; expected 0")
    return errors


def measure(binary: Path) -> dict:
    binary = binary.resolve()
    version = subprocess.run(
        [str(binary), "--version"],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    ).stdout.strip()
    expected = f"ivygrep {package_version()}"
    if version != expected:
        raise ValueError(f"binary reports {version!r}, expected {expected!r}")

    modes = {}
    environment = os.environ.copy()
    environment.setdefault("CI", "1")
    for mode, flags in (("foreground", []), ("hash-enriched", ["--enhance-hash"])):
        result = subprocess.run(
            [
                sys.executable,
                str(HARNESS),
                "--repo",
                str(ROOT),
                "--queries",
                str(FIXTURE),
                "--binary",
                str(binary),
                "--skip-build",
                "--json",
                *flags,
            ],
            cwd=ROOT,
            check=True,
            text=True,
            capture_output=True,
            env=environment,
        )
        modes[mode] = json.loads(result.stdout)

    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    ).stdout.strip()
    report = {
        "schema_version": 1,
        "scope": (
            "Labeled self-repository retrieval screen with source-level candidate "
            "recall auditing. This is not a public-code, million-chunk, neural, "
            "or agent-task benchmark."
        ),
        "ivygrep_commit": commit,
        "binary": {"version": version, "sha256": sha256_file(binary)},
        "runtime": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "fixture": {
            "path": str(FIXTURE.relative_to(ROOT)),
            "sha256": sha256_file(FIXTURE),
        },
        "harness": {
            "path": str(HARNESS.relative_to(ROOT)),
            "sha256": sha256_file(HARNESS),
        },
        "source": {
            "inputs": list(SOURCE_INPUTS),
            "sha256": source_inputs_sha256(),
        },
        "modes": modes,
    }
    errors = validate_report(report)
    if errors:
        raise ValueError("; ".join(errors))
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target" / "release" / "ig")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--check", action="store_true", help="validate existing evidence without rerunning it"
    )
    args = parser.parse_args()

    if args.check:
        report = json.loads(args.output.read_text(encoding="utf-8"))
        errors = validate_report(report)
        if errors:
            print("\n".join(errors), file=sys.stderr)
            return 1
        print(f"current-head relevance evidence matches {report['binary']['version']}")
        return 0

    report = measure(args.binary)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"wrote {args.output} for {report['binary']['version']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
