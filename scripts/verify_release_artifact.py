#!/usr/bin/env python3
"""Verify and safely extract an ivygrep release archive."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import tarfile
import zipfile


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_member(name: str) -> bool:
    path = PurePosixPath(name.replace("\\", "/"))
    return bool(path.parts) and not path.is_absolute() and ".." not in path.parts


def extract_archive(archive: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as handle:
            members = handle.getmembers()
            if any(not safe_member(member.name) for member in members):
                raise ValueError("archive contains an unsafe path")
            handle.extractall(destination, members=members, filter="data")
        return
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as handle:
            if any(not safe_member(name) for name in handle.namelist()):
                raise ValueError("archive contains an unsafe path")
            handle.extractall(destination)
        return
    raise ValueError(f"unsupported archive format: {archive.name}")


def safe_child(root: Path, *parts: str) -> Path:
    if any(not safe_member(part) or len(PurePosixPath(part).parts) != 1 for part in parts):
        raise ValueError("provenance contains an unsafe archive path")
    resolved_root = root.resolve()
    candidate = resolved_root.joinpath(*parts).resolve()
    if candidate.parent != resolved_root.joinpath(*parts[:-1]).resolve():
        raise ValueError("provenance archive path escapes extraction root")
    return candidate


def verify_checksum(archive: Path, checksum: Path) -> str:
    fields = checksum.read_text(encoding="utf-8").strip().split()
    if len(fields) != 2 or fields[1].lstrip("*") != archive.name:
        raise ValueError("checksum sidecar does not name the archive")
    actual = sha256_file(archive)
    if fields[0].lower() != actual:
        raise ValueError("archive checksum mismatch")
    return actual


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--checksum", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--sbom", type=Path, required=True)
    parser.add_argument("--extract-dir", type=Path, required=True)
    parser.add_argument("--expected-target", required=True)
    parser.add_argument("--expected-commit")
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()

    archive_sha256 = verify_checksum(args.archive, args.checksum)
    provenance = json.loads(args.provenance.read_text(encoding="utf-8"))
    sbom = json.loads(args.sbom.read_text(encoding="utf-8"))
    artifact = provenance["artifact"]
    binary_record = provenance["binary"]

    if provenance.get("schema_version") != 1:
        raise ValueError("unsupported provenance schema")
    if artifact.get("name") != args.archive.name:
        raise ValueError("provenance archive name mismatch")
    if artifact.get("sha256") != archive_sha256:
        raise ValueError("provenance archive checksum mismatch")
    if provenance.get("build", {}).get("target") != args.expected_target:
        raise ValueError("provenance target mismatch")
    if (
        args.expected_commit
        and provenance.get("source", {}).get("commit") != args.expected_commit
    ):
        raise ValueError("provenance source commit mismatch")
    if provenance.get("sbom", {}).get("sha256") != sha256_file(args.sbom):
        raise ValueError("provenance SBOM checksum mismatch")
    if not str(sbom.get("spdxVersion", "")).startswith("SPDX-"):
        raise ValueError("SBOM is not SPDX JSON")

    extract_archive(args.archive, args.extract_dir)
    binary = safe_child(
        args.extract_dir,
        artifact["root"],
        binary_record["name"],
    )
    if not binary.is_file():
        raise ValueError("archive does not contain the recorded binary")
    if sha256_file(binary) != binary_record.get("sha256"):
        raise ValueError("extracted binary checksum mismatch")

    result = {
        "archive": args.archive.name,
        "archive_sha256": archive_sha256,
        "binary": binary.as_posix(),
        "target": args.expected_target,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as handle:
            handle.write(f"binary={binary.as_posix()}\n")
            handle.write(f"archive_sha256={archive_sha256}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
