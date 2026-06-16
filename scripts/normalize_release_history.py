#!/usr/bin/env python3
"""Normalize GitHub release API data into stable archive-size history."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import tarfile
import zipfile


def binary_size(archive: Path) -> int | None:
    expected = {"ig", "ig.exe"}
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as handle:
            matches = [
                member.size
                for member in handle.getmembers()
                if member.isfile() and Path(member.name).name in expected
            ]
    elif archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as handle:
            matches = [
                member.file_size
                for member in handle.infolist()
                if not member.is_dir() and Path(member.filename).name in expected
            ]
    else:
        return None
    if len(matches) != 1:
        raise ValueError(
            f"{archive}: expected exactly one release binary, found {len(matches)}"
        )
    return matches[0]


def normalize(releases: list[dict], asset_dir: Path | None = None) -> dict:
    history = []
    for release in releases:
        tag = release.get("tag_name", "")
        assets = release.get("assets", [])
        asset_names = {asset.get("name", "") for asset in assets}
        archives = []
        for asset in assets:
            name = asset.get("name", "")
            if not (name.endswith(".tar.gz") or name.endswith(".zip")):
                continue
            prefix = f"ivygrep-{tag}-"
            target = name[len(prefix) :] if name.startswith(prefix) else name
            target = target.removesuffix(".tar.gz").removesuffix(".zip")
            local_archive = asset_dir / name if asset_dir else None
            archives.append(
                {
                    "name": name,
                    "target": target,
                    "size_bytes": asset.get("size", 0),
                    "binary_size_bytes": (
                        binary_size(local_archive)
                        if local_archive and local_archive.is_file()
                        else None
                    ),
                    "download_url": asset.get("browser_download_url"),
                    "checksum": f"{name}.sha256" in asset_names,
                    "sbom": f"{name.removesuffix('.tar.gz').removesuffix('.zip')}.spdx.json"
                    in asset_names,
                    "provenance": f"{name.removesuffix('.tar.gz').removesuffix('.zip')}.provenance.json"
                    in asset_names,
                }
            )
        if archives:
            history.append(
                {
                    "tag": tag,
                    "published_at": release.get("published_at"),
                    "release_url": release.get("html_url"),
                    "archives": sorted(archives, key=lambda item: item["target"]),
                    "total_archive_bytes": sum(item["size_bytes"] for item in archives),
                }
            )
    return {
        "schema_version": 1,
        "source": "https://api.github.com/repos/bvolpato/ivygrep/releases",
        "releases": history,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--asset-dir",
        type=Path,
        help="Optional directory containing downloaded public release archives.",
    )
    args = parser.parse_args()
    releases = json.loads(args.input.read_text(encoding="utf-8"))
    document = normalize(releases, args.asset_dir)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
