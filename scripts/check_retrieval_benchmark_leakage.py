#!/usr/bin/env python3
"""Detect benchmark query text or document IDs hard-coded in production Rust."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def normalized_fragments(dataset: Path) -> set[str]:
    fragments: set[str] = set()
    with (dataset / "queries.jsonl").open(encoding="utf-8") as handle:
        for line in handle:
            query = json.loads(line)
            text = " ".join(str(query.get("text") or "").split()).lower()
            if len(text) >= 32:
                fragments.add(text)
    return fragments


def find_leaks(source_root: Path, datasets: list[Path]) -> list[str]:
    sources = {
        path: " ".join(path.read_text(encoding="utf-8").split()).lower()
        for path in sorted(source_root.rglob("*.rs"))
    }
    leaks: list[str] = []
    for dataset in datasets:
        for fragment in normalized_fragments(dataset):
            for path, source in sources.items():
                if fragment in source:
                    leaks.append(f"{path}: contains query text from {dataset.name}")
    return leaks


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, default=Path("src"))
    parser.add_argument("datasets", nargs="+", type=Path)
    args = parser.parse_args()
    leaks = find_leaks(args.source_root, args.datasets)
    if leaks:
        raise SystemExit("retrieval benchmark leakage detected:\n" + "\n".join(leaks))
    print(f"retrieval benchmark leakage check passed ({len(args.datasets)} datasets)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
