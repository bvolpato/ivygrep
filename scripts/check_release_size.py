#!/usr/bin/env python3
"""Fail when a release binary exceeds the portability size budget."""

from __future__ import annotations

import argparse
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--max-mib", type=float, default=80.0)
    args = parser.parse_args()

    size = args.binary.stat().st_size
    limit = int(args.max_mib * 1024 * 1024)
    print(f"{args.binary}: {size / 1024 / 1024:.2f} MiB (budget {args.max_mib:.2f} MiB)")
    if size > limit:
        raise SystemExit(
            f"release binary exceeds size budget: {size} bytes > {limit} bytes"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
