#!/usr/bin/env python3
"""Generate a path-neutral provenance sidecar for a release archive."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--archive-root", required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--sbom", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--features", default="")
    parser.add_argument("--cargo-flags", default="")
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-ref", required=True)
    parser.add_argument("--workflow-run-id", required=True)
    parser.add_argument("--rustc-version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    feature_values = sorted(
        value
        for value in args.features.replace(",", " ").split()
        if value
    )
    cargo_flags = args.cargo_flags.split()
    document = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "source": {
            "repository": "https://github.com/bvolpato/ivygrep",
            "commit": args.source_commit,
            "ref": args.source_ref,
            "workflow_run_id": args.workflow_run_id,
        },
        "build": {
            "target": args.target,
            "features": feature_values,
            "cargo_flags": cargo_flags,
            "rustc": args.rustc_version,
            "version": args.version,
        },
        "artifact": {
            "name": args.archive.name,
            "root": args.archive_root,
            "sha256": sha256_file(args.archive),
            "size_bytes": args.archive.stat().st_size,
        },
        "binary": {
            "name": args.binary.name,
            "sha256": sha256_file(args.binary),
            "size_bytes": args.binary.stat().st_size,
        },
        "sbom": {
            "name": args.sbom.name,
            "format": "spdx-json",
            "sha256": sha256_file(args.sbom),
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
