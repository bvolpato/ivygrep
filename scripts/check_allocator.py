#!/usr/bin/env python3
"""Assert the allocator that an `ig` binary reports through `ig --json hardware`.

The 64-bit musl archives use jemalloc, whose page size is fixed when it is
built. A jemalloc built for 4 KiB pages aborts at startup on an aarch64 kernel
with 16 or 64 KiB pages, and QEMU user mode cannot show that, because it runs
with the host's 4 KiB pages. The binary therefore reports what it was built
with, and the release and E2E workflows assert it here.
"""

from __future__ import annotations

import argparse
import json
import subprocess


def reported_allocator(binary: str) -> dict:
    completed = subprocess.run(
        [binary, "--json", "hardware"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
        timeout=300,
    )
    return json.loads(completed.stdout).get("allocator") or {}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--name", required=True, choices=("jemalloc", "system"))
    parser.add_argument(
        "--page-size",
        type=int,
        help="page size in bytes that jemalloc must be built for; omit for the system allocator",
    )
    args = parser.parse_args()
    expected = {"name": args.name, "page_size": args.page_size}
    actual = reported_allocator(args.binary)
    if actual != expected:
        raise SystemExit(f"{args.binary} reports allocator {actual}, expected {expected}")
    print(f"allocator check passed: {actual}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
