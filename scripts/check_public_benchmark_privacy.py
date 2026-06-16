#!/usr/bin/env python3
"""Reject local identifiers and caller-supplied terms in public artifacts."""

from __future__ import annotations

import argparse
from pathlib import Path
import re


LOCAL_PATH_PATTERNS = (
    re.compile(r"/home/[^/\s]+/"),
    re.compile(r"/Users/[^/\s]+/"),
    re.compile(r"[A-Za-z]:\\Users\\[^\\\s]+\\"),
)


def violations(
    paths: list[Path], forbidden_terms: list[str] | None = None
) -> list[str]:
    patterns = list(LOCAL_PATH_PATTERNS)
    patterns.extend(
        re.compile(re.escape(term), re.IGNORECASE)
        for term in forbidden_terms or []
        if term
    )
    found: list[str] = []
    for root in paths:
        files = (
            [root]
            if root.is_file()
            else sorted(path for path in root.rglob("*") if path.is_file())
        )
        for path in files:
            text = path.read_text(encoding="utf-8", errors="replace")
            for line_number, line in enumerate(text.splitlines(), start=1):
                for pattern in patterns:
                    if pattern.search(line):
                        found.append(f"{path}:{line_number}: matched {pattern.pattern}")
    return found


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument(
        "--forbidden-term",
        action="append",
        default=[],
        help="case-insensitive sensitive term to reject; may be repeated",
    )
    args = parser.parse_args()
    found = violations(args.paths, args.forbidden_term)
    if found:
        raise SystemExit(
            "publishable benchmark privacy check failed:\n" + "\n".join(found)
        )
    print(f"public benchmark privacy check passed ({len(args.paths)} roots)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
