# Portable embedding model bake-off

Generated from pinned public CoIR samples. No private corpus, local path, hostname, query text, or source text is retained.

- Commit: `2c735847d43edbe8a31d516b0fbb7c22b20105c2`
- Binary SHA-256: `2d5864761a9ad0ec8f0407cd31662d4eab5995d3d1b76fccf2e9bc2c3bbd321b`
- Selected default: `static-retrieval-v1`

| Profile | Status | nDCG@10 | MRR@10 | R@20 | Warm p95 | Neural build | Peak RSS | Index size |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| static-retrieval-v1 | selected-default | 0.5910 | 0.5190 | 0.8800 | 294.63 ms | 2969.03 ms | 316.52 MiB | 11.59 MiB |
| code-minilm-l12-v1 | resource-stop | 0.5273 | 0.4075 | 0.9600 | 1847.21 ms | 319223.00 ms | 1.34 GiB | 4.71 MiB |
| code-minilm-l6-v1 | rejected | 0.5330 | 0.4248 | 1.0000 | 1155.55 ms | 187393.68 ms | 1.02 GiB | 4.66 MiB |
| general | rejected | 0.4466 | 0.3240 | 0.9600 | 1309.51 ms | 196297.97 ms | 1.00 GiB | 4.70 MiB |
| Salesforce/SFR-Embedding-Code-400M_R | excluded | - | - | - | - | - | - | - |
| jinaai/jina-code-embeddings-1.5b | excluded | - | - | - | - | - | - | - |
| nomic-ai/CodeRankEmbed | excluded | - | - | - | - | - | - | - |

## Decision

The static retrieval profile is the portable Pareto winner and the only candidate promoted through the complete screening matrix. Transformer candidates that crossed a laptop screening limit were stopped after one completed task, so their partial results stay single-task results.

The selected model was promoted to the full 1,000-query public matrix; screening-only results are not used as headline quality numbers.

## Reproduce

```bash
uv run scripts/export_public_retrieval.py --profile model-bakeoff \
  --output /tmp/ivygrep-model-bakeoff-datasets
IVYGREP_MODEL_PROFILE=static uv run scripts/run_public_benchmark_matrix.py \
  --profile model-bakeoff --modes neural --runs 1 \
  --datasets-root /tmp/ivygrep-model-bakeoff-datasets \
  --work-root /tmp/ivygrep-model-bakeoff-static \
  --output /tmp/ivygrep-model-bakeoff-static.json
```
