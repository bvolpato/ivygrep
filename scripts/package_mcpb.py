#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Build cross-platform ivygrep MCPB and matching MCP Registry metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import stat
import tarfile
import tempfile
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MCPB_ROOT = ROOT / "packaging" / "mcpb"
REGISTRY_SCHEMA = (
    "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json"
)
SERVER_NAME = "io.github.bvolpato/ivygrep"

TARGETS = {
    "linux-x86_64-musl": ("linux-x64", "ig"),
    "linux-aarch64-musl": ("linux-arm64", "ig"),
    "macos-x86_64": ("darwin-x64", "ig"),
    "macos-aarch64": ("darwin-arm64", "ig"),
    "windows-x86_64": ("win32-x64", "ig.exe"),
}


def archive_for(artifacts: Path, version: str, suffix: str) -> Path:
    extension = ".zip" if suffix.startswith("windows-") else ".tar.gz"
    name = f"ivygrep-v{version}-{suffix}{extension}"
    matches = list(artifacts.rglob(name))
    if len(matches) != 1:
        raise ValueError(f"expected one {name} below {artifacts}, found {len(matches)}")
    return matches[0]


def extract_binary(archive: Path, binary_name: str, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            matches = [
                name for name in source.namelist() if Path(name).name == binary_name
            ]
            if len(matches) != 1:
                raise ValueError(
                    f"expected one {binary_name} in {archive}, found {len(matches)}"
                )
            with source.open(matches[0]) as reader, destination.open("wb") as writer:
                shutil.copyfileobj(reader, writer)
    else:
        with tarfile.open(archive, "r:gz") as source:
            matches = [
                member
                for member in source.getmembers()
                if member.isfile() and Path(member.name).name == binary_name
            ]
            if len(matches) != 1:
                raise ValueError(
                    f"expected one {binary_name} in {archive}, found {len(matches)}"
                )
            reader = source.extractfile(matches[0])
            if reader is None:
                raise ValueError(f"could not read {matches[0].name} from {archive}")
            with reader, destination.open("wb") as writer:
                shutil.copyfileobj(reader, writer)
    destination.chmod(
        destination.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
    )


def write_bundle(stage: Path, output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(
        output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as bundle:
        for path in sorted(stage.rglob("*")):
            if path.is_file():
                bundle.write(path, path.relative_to(stage).as_posix())


def registry_document(version: str, bundle: Path) -> dict[str, object]:
    digest = hashlib.sha256(bundle.read_bytes()).hexdigest()
    artifact = f"ivygrep-mcp-v{version}.mcpb"
    return {
        "$schema": REGISTRY_SCHEMA,
        "name": SERVER_NAME,
        "title": "ivygrep",
        "description": "Local code search and task context packs for coding agents",
        "version": version,
        "websiteUrl": "https://bvolpato.github.io/ivygrep/integrations/mcp.html",
        "repository": {
            "url": "https://github.com/bvolpato/ivygrep",
            "source": "github",
        },
        "packages": [
            {
                "registryType": "mcpb",
                "identifier": f"https://github.com/bvolpato/ivygrep/releases/download/v{version}/{artifact}",
                "fileSha256": digest,
                "transport": {"type": "stdio"},
            }
        ],
    }


def build(artifacts: Path, version: str, output: Path, server_json: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="ivygrep-mcpb-") as temporary:
        stage = Path(temporary)
        manifest = json.loads(
            (MCPB_ROOT / "manifest.template.json").read_text(encoding="utf-8")
        )
        manifest["version"] = version
        (stage / "manifest.json").write_text(
            json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
        )
        shutil.copy2(ROOT / "assets" / "logo.png", stage / "icon.png")
        shutil.copytree(MCPB_ROOT / "server", stage / "server", dirs_exist_ok=True)

        for suffix, (target_dir, binary_name) in TARGETS.items():
            archive = archive_for(artifacts, version, suffix)
            extract_binary(
                archive,
                binary_name,
                stage / "server" / "bin" / target_dir / binary_name,
            )

        write_bundle(stage, output)

    server_json.parent.mkdir(parents=True, exist_ok=True)
    server_json.write_text(
        json.dumps(registry_document(version, output), indent=2) + "\n",
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--server-json", type=Path, required=True)
    args = parser.parse_args()
    build(args.artifacts, args.version.removeprefix("v"), args.output, args.server_json)


if __name__ == "__main__":
    main()
