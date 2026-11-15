# CUDA release validation

The published **v1.3.4 Linux x86_64 CUDA archive** passed strict GPU enhancement
and forced neural retrieval on 2026-11-14. CPU fallback was not accepted.
This report describes v1.3.4 only; it does not validate a later release.

## Artifact and device

| Item | Verified value |
| --- | --- |
| Source | `e146a71dfb5fe07056ed97baefe698c711877261`, tag `v1.3.4` |
| Archive | `ivygrep-v1.3.4-linux-x86_64-cuda.tar.gz` |
| Archive SHA-256 | `74226c83363027e2de07f7d6889c99bca7ee55c0432e13d3ab3c17b6eeaab15f` |
| Binary SHA-256 | `868a9505eb31b71e50e7fdb0f7ccea72e10500ed06d185cc69f827ab5ca47de9` |
| GPU | NVIDIA GeForce RTX 5070 Ti, compute capability 12.0, 16 GiB VRAM |
| Driver and toolkit | NVIDIA 595.71.05; CUDA 13.2.78 |
| Model | `general`: `sentence-transformers/all-MiniLM-L6-v2` |
| Model revision | `1110a243fdf4706b3f48f1d95db1a4f5529b4d41` |

The [archive and checksum](https://github.com/bvolpato/ivygrep/releases/tag/v1.3.4)
matched. GitHub's signed attestation verified the source commit, tag,
`.github/workflows/release.yml` signer, and GitHub-hosted builder for
[release run 43161685715](https://github.com/bvolpato/ivygrep/actions/runs/43161685715).
The archive's provenance also matched the extracted binary digest.

## Result

The existing backend procedure indexed an isolated fixture, persisted neural
vectors, and required the CUDA backend in status output. Its forced neural query
returned `src/lib.rs` and reported `neural_executed: true`:

```text
Semantic retrieval procedure passed: src/lib.rs
Neural backend procedure passed: BERT embedding via Candle CUDA
```

During enhancement, `nvidia-smi` observed the released `ig` process using 430 MiB
of GPU memory. This was one observation, not a peak-memory measurement. Model
assets were cached separately and checksum-verified before the procedure ran
with HTTP(S) proxies pointing to the unavailable local endpoint `127.0.0.1:9`.
The procedure used a temporary workspace and app home; it did not alter existing
indexes or daemon state.

## Reproduce

From a v1.3.4 checkout, download and extract the CUDA archive into `$artifact_dir`,
verify its published checksum, and verify the signed source identity:

```sh
gh attestation verify "$artifact_dir/ivygrep-v1.3.4-linux-x86_64-cuda.tar.gz" \
  --repo bvolpato/ivygrep \
  --signer-workflow bvolpato/ivygrep/.github/workflows/release.yml \
  --source-ref refs/tags/v1.3.4 \
  --source-digest e146a71dfb5fe07056ed97baefe698c711877261 \
  --deny-self-hosted-runners

uv run scripts/cache_neural_model.py --profile general --cache "$artifact_dir/hf-cache"
HF_HOME="$artifact_dir/hf-cache" CUDA_VISIBLE_DEVICES=0 \
  IVYGREP_E2E_DOWNLOAD_ATTEMPTS=1 \
  HTTP_PROXY=http://127.0.0.1:9 HTTPS_PROXY=http://127.0.0.1:9 \
  ALL_PROXY=http://127.0.0.1:9 NO_PROXY='' \
  ./scripts/e2e_neural_backend.sh \
    --binary "$artifact_dir/ivygrep-v1.3.4-linux-x86_64-cuda/ig" \
    --model-profile general \
    --expect-backend 'BERT embedding via Candle CUDA' \
    --expect-file src/lib.rs
```

This checks one GPU and transformer profile. It does not establish performance,
sustained-load behavior, other CUDA architectures, or other model profiles.
The default static embedding model continues to run on CPU.
