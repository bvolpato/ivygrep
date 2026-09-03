# CoSQA relevance and executed-model validation

The unchanged public relevance gate passes on all four datasets and all five
modes after bounding raw cosine corroboration of direct candidates. No model
weights, query sets, dataset limits, candidate budgets, or thresholds changed.
This is regression evidence, not a state-of-the-art retrieval claim.

The [machine-readable report](cosqa-relevance-2026-09-02.json) contains all
per-dataset/mode quality measurements, binary and source fingerprints, raw-file
checksums, and the exact query IDs gained or lost by forced-neural retrieval.

## Ranking change

The raw cosine term could contribute several reciprocal-rank votes to a
candidate already supported by lexical, literal, path, or symbol evidence.
Its scale depends on the embedding model and corpus. Cap that additional
corroboration at one semantic rank vote; keep the semantic-only discovery
path and the existing hash-tier discount intact.

A regression test holds query coverage constant and demonstrates that maximal
cosine similarity cannot overturn much stronger lexical evidence. Native
capture also showed that the inspected CoSQA camera/PIL query skipped learned
reranking: changing learned model weights would not have addressed that route.

## Fresh public-core comparison

Each binary ran the same pinned 1,000 queries in lexical, hash, hybrid, neural,
and blended modes. These are single-run quality measurements; ANN/index tie
variation is visible even in some unchanged lexical results. Jobs overlapped
other validation on disjoint CPU affinities, so their latency is not used as
performance evidence.

| Forced-neural panel | Baseline nDCG@10 | Candidate nDCG@10 | Baseline Recall@20 | Candidate Recall@20 |
| --- | ---: | ---: | ---: | ---: |
| CoSQA, 500 queries | 0.137507 | 0.145138 | 0.340000 | 0.356000 |
| CodeFeedback, 99 queries | 0.691558 | 0.674022 | 0.787879 | 0.767677 |
| CodeTrans contest, 221 queries | 0.384400 | 0.386244 | 0.515837 | 0.515837 |
| CodeTrans DL, 180 queries | 0.226436 | 0.227414 | 0.566667 | 0.572222 |
| Query-weighted total | 0.262929 | 0.265592 | 0.464000 | 0.471000 |

CoSQA clears its existing 0.14 nDCG and 0.35 recall floors. It gains ten and
loses two successful neural queries at top 20. CodeFeedback loses two of 99
queries (`q82302`, `q94089`): this is a real observed tradeoff, not a universal
improvement. All existing per-dataset floors and retained-query checks pass.
Blended CoSQA also improves: nDCG 0.140686 to 0.146800, recall 0.35 to 0.36.

## Fit-disjoint diagnostic

The separate 520-query `reranker-eval` run has zero repository-qualified ID
overlap with the unchanged model's 481 fit-query ledger. All four result files
report the exact embedded model SHA-256 bound to that ledger and the selected
binary SHA-256. `--require-fit-disjoint` passed. Weighted neural nDCG is
0.253879, Recall@20 is 0.486538, and there are no no-hit queries. A fresh
baseline run scores 0.248942 and 0.471154 respectively; the old binary lacks
runtime model attestation, so it is only a quality comparison, not upgraded
fit-disjoint certification.

The public-core panel is still regression evidence: 431 of its 1,000 queries
overlap the fit ledger. The diagnostic's ID disjointness does not prove
semantic independence, lack of development-time exposure, or learned-model
invocation on every query. It does not replace the public-core gate.
Old reports and binaries without the runtime model digest remain unverified;
matching only a model name cannot upgrade them.

## Validation and reproduction

- 1,085 Rust tests and all 11 explicitly invoked stress tests passed.
- 260 Python tests passed, including mismatched, absent, disabled, partial,
  and forged executed-model attestation cases.
- Strict release Clippy, Rustfmt, and artifact privacy checks passed.
- Seven CLI/daemon equivalence cases and 116 layered-worktree checks passed.
- All 23 self-repository queries passed foreground/hash/neural checks, with
  actual neural execution reported for all 23 neural queries.

Build from this change and retain the build revision with the result files:

```bash
cargo build --locked --release --bin ig
python3 scripts/run_public_benchmark_matrix.py \
  --profile public-core --modes lexical,hash,hybrid,neural,blended --runs 1 \
  --datasets-root /tmp/public-data --work-root /tmp/public-results \
  --binary target/release/ig --skip-build --output /tmp/public-matrix.json
python3 scripts/check_public_relevance.py \
  --matrix /tmp/public-matrix.json --raw-results /tmp/public-results
python3 scripts/run_public_benchmark_matrix.py \
  --profile reranker-eval --modes neural --runs 1 --require-fit-disjoint \
  --datasets-root /tmp/fit-disjoint-data --work-root /tmp/fit-disjoint-results \
  --binary target/release/ig --skip-build --output /tmp/fit-disjoint-matrix.json
```

The recorded candidate was built before committing over clean base `7413229`.
Its source-input and binary checksums distinguish it from the baseline;
the matrices' base-commit labels alone do not identify the modified source.
