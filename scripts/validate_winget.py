#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "jsonschema",
#     "PyYAML",
#     "requests",
# ]
# ///
"""Validate a multi-file WinGet manifest against its declared schemas."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

import requests
import yaml
from jsonschema.validators import validator_for


SCHEMA_PATTERN = re.compile(r"^# yaml-language-server: \$schema=(\S+)$", re.MULTILINE)


def validate_manifest(path: Path) -> None:
    source = path.read_text(encoding="utf-8")
    match = SCHEMA_PATTERN.search(source)
    if match is None:
        raise ValueError(f"{path}: missing yaml-language-server schema")

    response = requests.get(match.group(1), timeout=30)
    response.raise_for_status()
    schema = response.json()
    validator = validator_for(schema)(schema)
    errors = sorted(validator.iter_errors(yaml.safe_load(source)), key=str)
    if errors:
        details = "\n".join(f"  {error.json_path}: {error.message}" for error in errors)
        raise ValueError(f"{path}: schema validation failed\n{details}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest_directory", type=Path)
    args = parser.parse_args()
    manifests = sorted(args.manifest_directory.glob("*.yaml"))
    if not manifests:
        raise ValueError(f"no YAML manifests in {args.manifest_directory}")
    for manifest in manifests:
        validate_manifest(manifest)
        print(f"valid: {manifest.name}")


if __name__ == "__main__":
    main()
