#!/usr/bin/env python3
"""Check release identity and current-source neural evidence before building archives."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import tomllib

import run_current_head_benchmark as relevance


ROOT = Path(__file__).resolve().parents[1]
PLUGIN_MANIFESTS = (
    "plugins/ivygrep/.claude-plugin/plugin.json",
    "plugins/ivygrep/.codex-plugin/plugin.json",
)


def validate_release(root: Path, tag: str) -> list[str]:
    if re.fullmatch(r"v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", tag) is None:
        return ["release tag must be a version such as v1.2.13"]
    version = tag[1:]
    errors = []
    actual = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if actual != version:
        errors.append(f"Cargo.toml version {actual!r} does not match {tag}")
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    locked = [p["version"] for p in lock["package"] if p["name"] == "ivygrep"]
    if locked != [version]:
        errors.append(f"Cargo.lock ivygrep versions {locked!r} do not match {tag}")
    for path in PLUGIN_MANIFESTS:
        plugin_version = json.loads((root / path).read_text())["version"]
        if plugin_version != version:
            errors.append(f"{path} version {plugin_version!r} does not match {tag}")
    if re.search(rf">\s*v{re.escape(version)}\s*<", (root / "docs/index.html").read_text()) is None:
        errors.append(f"docs/index.html version badge does not match {tag}")

    notes = re.search(
        rf"^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## |\Z)",
        (root / "CHANGELOG.md").read_text(), re.MULTILINE | re.DOTALL,
    )
    if notes is None or re.search(r"^[-*] \S", notes[1], re.MULTILINE) is None:
        errors.append(f"CHANGELOG.md has no release notes for {version}")
    report = json.loads((root / "docs/benchmarks/current-head-relevance.json").read_text())
    errors.extend(relevance.validate_report(report, root=root, require_neural=True))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    args = parser.parse_args()
    try:
        errors = validate_release(ROOT, args.tag)
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise SystemExit(f"release preflight could not read its required inputs: {error}") from error
    if errors:
        raise SystemExit("release preflight failed:\n" + "\n".join(errors))
    print(f"release identity and source-matched neural evidence passed for {args.tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
