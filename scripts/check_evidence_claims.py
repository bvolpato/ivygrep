#!/usr/bin/env python3
"""Reject public claims that are not supported by the generated dashboard."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re


REGULATED = {
    "state_of_the_art": re.compile(
        r"state[- ]of[- ]the[- ]art|\bsota\b(?!-challenge)",
        re.IGNORECASE,
    ),
    "competitive": re.compile(r"\bcompetitive\b", re.IGNORECASE),
    "portable": re.compile(r"\bportable\b", re.IGNORECASE),
}

CLAIM_CONTROL_FILES = {
    "claims-policy.md",
    "evidence-dashboard.html",
    "evidence-dashboard.json",
    "evidence-dashboard.md",
}


def scans_regulated_claims(path: Path) -> bool:
    return path.name not in CLAIM_CONTROL_FILES


def check(dashboard: dict, marketing_paths: list[Path]) -> list[str]:
    failures = []
    for claim, pattern in REGULATED.items():
        if dashboard["claims"][claim]["supported"]:
            continue
        for path in marketing_paths:
            if not scans_regulated_claims(path):
                continue
            text = path.read_text(encoding="utf-8")
            if pattern.search(text):
                failures.append(f"{path}: unsupported {claim} claim")

    daemon = next(
        item["summary"]
        for item in dashboard["evidence"]
        if item["id"] == "daemon-cache"
    )
    for path in marketing_paths:
        text = path.read_text(encoding="utf-8")
        if "sub-100-ms warm daemon replay" in text.lower():
            p95 = daemon.get("retained_warm_p95_ms")
            if p95 is None or p95 >= 100.0:
                failures.append(f"{path}: sub-100-ms daemon claim lacks evidence")
    return failures


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--dashboard",
        type=Path,
        default=root / "docs" / "benchmarks" / "evidence-dashboard.json",
    )
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        default=[root / "README.md", root / "docs" / "index.html"],
    )
    args = parser.parse_args()
    dashboard = json.loads(args.dashboard.read_text(encoding="utf-8"))
    failures = check(dashboard, args.paths)
    if failures:
        raise SystemExit("evidence claim check failed:\n" + "\n".join(failures))
    print(f"evidence claim check passed ({len(args.paths)} marketing files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
