#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "huggingface-hub[hf-xet]==1.23.0",
# ]
# ///
"""Cache pinned ivygrep neural assets through Hugging Face's Xet client."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
from pathlib import Path


@dataclass(frozen=True)
class ModelProfile:
    repo_id: str
    revision: str
    assets: tuple[str, ...]
    weights_asset: str
    weights_sha256: str


PROFILES = {
    "static": ModelProfile(
        repo_id="sentence-transformers/static-retrieval-mrl-en-v1",
        revision="f60985c706f192d45d218078e49e5a8b6f15283a",
        assets=(
            "0_StaticEmbedding/tokenizer.json",
            "0_StaticEmbedding/model.safetensors",
        ),
        weights_asset="0_StaticEmbedding/model.safetensors",
        weights_sha256="164fc63ee9f9267be7378fcbd7df99d09788a2f45244c92aa99ae5a574925716",
    ),
    "general": ModelProfile(
        repo_id="sentence-transformers/all-MiniLM-L6-v2",
        revision="1110a243fdf4706b3f48f1d95db1a4f5529b4d41",
        assets=("config.json", "tokenizer.json", "model.safetensors"),
        weights_asset="model.safetensors",
        weights_sha256="53aa51172d142c89d9012cce15ae4d6cc0ca6895895114379cacb4fab128d9db",
    ),
}


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_rust_revision_ref(
    profile: ModelProfile, downloaded: dict[str, Path]
) -> Path:
    snapshot_roots = {
        path.parents[len(Path(asset).parts) - 1]
        for asset, path in downloaded.items()
    }
    if len(snapshot_roots) != 1:
        raise SystemExit(f"model assets span multiple snapshots for {profile.repo_id}")
    snapshot = snapshot_roots.pop()
    if snapshot.parent.name != "snapshots" or snapshot.name != profile.revision:
        raise SystemExit(
            f"unexpected model snapshot for {profile.repo_id}: {snapshot}"
        )
    ref = snapshot.parent.parent / "refs" / profile.revision
    ref.parent.mkdir(parents=True, exist_ok=True)
    ref.write_text(snapshot.name, encoding="utf-8")
    return ref


def cache_profile(name: str, cache: Path) -> None:
    from huggingface_hub import hf_hub_download

    profile = PROFILES[name]
    cache.mkdir(parents=True, exist_ok=True)
    downloaded = {
        asset: Path(
            hf_hub_download(
                repo_id=profile.repo_id,
                filename=asset,
                revision=profile.revision,
                cache_dir=cache / "hub",
            )
        )
        for asset in profile.assets
    }
    weights = downloaded[profile.weights_asset]
    digest = file_sha256(weights)
    if digest != profile.weights_sha256:
        raise SystemExit(
            f"model checksum mismatch for {profile.repo_id}: "
            f"expected {profile.weights_sha256}, got {digest}"
        )
    write_rust_revision_ref(profile, downloaded)
    print(f"cached {name} neural model at {cache}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(PROFILES), required=True)
    parser.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args()
    cache_profile(args.profile, args.cache.resolve())


if __name__ == "__main__":
    main()
