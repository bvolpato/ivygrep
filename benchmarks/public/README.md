# Public evaluation contracts

This directory pins the public datasets, profiles, gates, and reranker artifacts
behind ivygrep's retrieval benchmarks. Published reports live in
[`docs/benchmarks/`](../../docs/benchmarks/).

## Files

| File | Purpose |
| --- | --- |
| `manifest.json` | Dataset sources, task options, benchmark profiles, and the fit-ledger checksum. |
| `relevance_gates.json` | Per-dataset quality thresholds for `public-core`, checked by `scripts/check_public_relevance.py`. |
| `reranker_model.json` | Embedded learned-reranker weights, feature schema, and training metadata. |
| `reranker_fit_query_ids.json` | Fit ledger: every query ID used to fit that model, bound to its SHA-256. |
| `model_candidates.json` | Embedding-profile candidates for the `model-bakeoff` screening report. |
| `file_localization_tasks.jsonl` | Issue-text-to-fixed-files tasks for `scripts/bench_file_localization.py`. |

Profiles in `manifest.json` include `public-core` (1,000-query regression panel
and release gate), `sota-challenge` (harder, disjoint task families),
`reranker-fit`, `reranker-eval`, `reranker-train`, `model-bakeoff`, and `full`.

## Run a matrix

Requires [uv](https://docs.astral.sh/uv/). The script exports pinned datasets
into `--datasets-root` unless `--skip-export` is set, builds
`target/release/ig` unless `--skip-build` is set, and writes aggregated results
to `--output`:

```bash
uv run scripts/run_public_benchmark_matrix.py \
  --profile public-core \
  --modes lexical,hash,hybrid,blended,neural --runs 3 \
  --datasets-root /tmp/ivygrep-public-datasets \
  --work-root /tmp/ivygrep-public-results \
  --output public-code-retrieval-results.json

python3 scripts/render_public_benchmark.py \
  --input public-code-retrieval-results.json \
  --baseline docs/benchmarks/public-code-retrieval-baseline-results.json \
  --html public-code-retrieval.html
```

Default modes are `lexical,hash,hybrid`. The release gate runs all five modes
three times.

## Terms

- **Checkout-reference model**: `reranker_model.json` plus its fit ledger, as
  checked out at the benchmarked revision. It describes the intended model; it
  does not prove which model the executed binary embeds.
- **Fit-ID audit**: `fit_query_audit` in each matrix JSON. It compares the
  profile's repository-qualified query IDs with the fit ledger and reports
  overlap.
- **Schema 2**: `fit_query_audit.schema_version` 2. It separates the verified
  `reference` model and ledger from `executed_binary` applicability, which stays
  `unverified` unless every result attests the matching embedded-model checksum.
- **C2 evidence**: learned-reranker features computed from fixed two-line
  context (`RANKING_CONTEXT_LINES = 2` in `src/reranker.rs`), independent of
  display `-C`. Native capture records must use it.

## `public-core` scope

`public-core` is the existing 1,000-query regression panel. Its query sets,
dataset limits, relevance thresholds and release-CI role are unchanged. It is
not an unseen-query generalization set for the checkout-reference learned reranker.

## Actual model-fit query IDs

`reranker_fit_query_ids.json` records all 481 fit IDs for the unchanged
checkout-reference model. The manifest pins this ledger's checksum. The ledger
binds the model bytes, each training source's provenance and result checksums, and the
exact query IDs. The four original source-provenance hashes were reconstructed
from pinned data, including the sampled codefeedback source.

Every public matrix records a checkout-reference fit-ID audit. Regression
profiles report overlap without dropping queries. The separately named `reranker-eval` diagnostic
requires zero overlap against this reference ledger. It does not replace the
public-core release gate. IDs are qualified by query repository; this is not a
claim that semantically similar questions or corpus documents are disjoint.
Overlap alone does not prove model overfitting.

Schema 2 separates the verified `reference` model/ledger from the executed
binary. New binaries expose `reranker_model_sha256` in workspace status and
`--doctor --json`, computed from the embedded model bytes. The matrix checks
that every result reports that checksum, the expected model ID, learned mode,
and the selected binary checksum before marking applicability `verified`.
Missing checksums, same-name/different-byte models, and disabled rerankers stay
`unverified`.

Use `--profile reranker-eval --require-fit-disjoint` to fail the matrix unless
actual repository-qualified query IDs have zero fit-ledger overlap and every
result attests the matching embedded model. This proves the stated ID/byte
relationship, not semantic independence, lack of development-time exposure,
or learned-reranker invocation on every query. Routing can skip the learned
stage even when the model is available. It does not replace public-core.

Existing public artifacts remain readable. The renderer labels public-core as
regression evidence and does not upgrade legacy `verified` flags into
executed-model certification.

## Native reranker training capture

Normal grouped CLI scores can already include learned reranking, backfill and
presentation choices. They are not a faithful reconstruction of the native
pre-learned candidate pool. No-expansion evaluation now preserves native file
`total_score`; multi-query ensembles retain a separate `fusion_score` for their
unchanged reciprocal-rank ordering. Neither form is accepted as a substitute
for native training features.

New training collection requires a capture-capable binary and explicit
`--capture-reranker`. This opt-in uses fresh local processes, canonical C2
evidence and the normal learned candidate budget. Inherited
`IVYGREP_RERANKER_CAPTURE` values are ignored; only the explicit CLI flag enables
capture in the evaluator. The native implementation
emits one versioned `IVYGREP_RERANKER_CAPTURE` record to stderr before learned
score remapping, from the actual accepted pre-backfill pool. Normal stdout is
unchanged. Keep learned mode enabled; deterministic mode is reported as a
skipped native gate, not converted into a guessed training pool.

With pinned assets already cached, a per-dataset collection command is:

```bash
HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 IVYGREP_RERANKER=learned \
  uv run scripts/eval_code_retrieval.py \
    --dataset /tmp/ivygrep-reranker-fit/codetrans-dl \
    --binary target/release/ig --source-commit BUILD_COMMIT \
    --mode blended --capture-reranker \
    --output /tmp/ivygrep-reranker-traces/codetrans-dl.json
```

Replace `BUILD_COMMIT` with the exact binary build commit, not the current
checkout when it differs.

Use `--mode hash` for hash-only correctness fixtures. Native capture requires
`--output` and no query expansion. Each output has a sibling
`.native-captures` directory containing original stdout, stderr, command/PID
and exit receipts. Existing capture directories are not overwritten. Retain
the directory with the result JSON when moving training inputs.

The collector requires exactly one current, valid record for the query and
the spawned process PID. Missing, duplicate, foreign-PID, mismatched,
unsupported or nonfinite records fail collection. Existing daemon responses
cannot silently substitute for local capture. Raw failures remain available.
Query identity follows native outer-whitespace trimming, while original
arguments and receipts remain unchanged. Capture framing uses literal LF;
Unicode line separators inside JSON query or preview strings are payload data.

The trainer validates the original receipts and dataset bytes, then consumes
the exported native feature arrays directly. It never recomputes uncertain
grouped-output features. Explicit skipped routes are retained and counted;
they contribute no model-fit example and are not retrieval-quality failures.
Legacy traces without native provenance fail clearly instead of being upgraded.
Training/evaluation pairs must have disjoint actual repository-qualified IDs.

The trainer fits features that come from the file path (`PATH_FEATURES` in
`train_public_reranker.py`) only on corpora with real paths. The public exporter
stores every document at `documents/<position>.<extension>`, so there these
features describe the export: `primary_source` is the dataset's language tag.
For a corpus whose documents all sit in a flat `documents/` directory, the
trainer reads the path features as zero, in the fit and in every evaluation of
it (validation, which picks the hyperparameters, and `--evaluation-pair`, which
decides the acceptance gate). Their weights stay zero, and the model file names
them under `fixed_zero_features`. The embedded model was fit before this rule
existed. Its three non-zero path weights were set to zero afterwards;
`fixed_zero_features.replaced_fitted_weights` keeps the fitted values, and the
fit ledger is bound to the new model bytes.

Every metrics record in a model file names the weights it was computed for
(`weights_sha256`, a checksum of the feature order and the weights), and
`render_public_reranker.py` refuses a model whose `evaluation` record names other
weights, or matrices whose results do not report the model file's checksum. When
weights change after a fit, evaluate them again:

```bash
python3 scripts/train_public_reranker.py \
  --reevaluate benchmarks/public/reranker_model.json \
  --fit-ledger benchmarks/public/reranker_fit_query_ids.json \
  --evaluation-pair /tmp/ivygrep-reranker-eval/codetrans-dl=/tmp/ivygrep-reranker-traces/codetrans-dl.json \
  --output benchmarks/public/reranker_model.json
```

This fits nothing. It checks that no evaluation query is a fit ID, writes the
`evaluation` record for the weights in the file with its date and capture commit,
moves records computed for other weights to `original_fit`, and binds the ledger
to the new model bytes; pin the printed ledger checksum in `manifest.json`. The
embedded model's `evaluation` was made this way on captures of the `reranker-eval`
and `reranker-train` profiles at main. The traces of its original fit no longer
exist, so `original_fit` keeps that fit's records as history, under the checksum
of the fitted weights.

`train_public_reranker.py --fit-ledger-output PATH` writes the exact used-ID
ledger bound to a newly generated model. Skipped IDs are excluded from fit
counts but remain in source provenance. Updating the embedded model and its
manifest-pinned ledger is a separate, reviewed action; these changes do not
retrain or change existing weights.

Native-capture latency includes local process/model startup and diagnostic
output. It is labeled `native-training-capture`, not the normal warm benchmark
path. Do not use these latency numbers as public performance evidence. Both
the capture result JSON (including embedded native records) and its sibling raw
receipt directory contain queries and canonical source previews. Neither may
be uploaded as an ordinary public benchmark summary.

## Compatible reuse and provenance

Each new result records a versioned execution fingerprint covering actual
dataset bytes and provenance, binary checksum, execution-harness checksums,
explicit query/fusion options, a safe configuration whitelist and runtime
identity. Model/reranker settings, candidate limits, relevant thread/backend
settings and capture mode cannot silently change under `--reuse-results`.
Query-cache disabling follows native presence semantics: even a value of `0`
disables it. Foreground acceleration is fingerprinted as effective `cpu` or
`auto`, matching the runtime's supported values and defaults.
Credentials are never included. Explicit cache-location/device/log settings
that can contain private values are represented only by digests.

Legacy results without this fingerprint can still be rendered, but cannot be
reused as fresh measurements or accepted as training traces. Incompatible or
corrupt fingerprints fail. Exported source revisions, profile sampling and
actual query counts must also match the requested matrix.

Cached execution provenance remains original. New matrix assembly records its
own `aggregation_provenance`; it does not relabel old execution as the current
checkout or machine. Homogeneous execution metadata remains available in the
legacy top-level fields. Mixed source commits are explicitly marked and listed.

`--source-commit` is the caller's build-commit assertion; the binary checksum
identifies its bytes. Supply the actual build commit for external binaries.
Both the evaluator and matrix assembly recheck the full binary checksum before
publishing results, rejecting persistent replacement during a run. These checks
cannot detect a transient replacement restored before the final check; callers
must keep the binary frozen for the entire execution. Dirty local builds also
need a separate full source/patch receipt; a base commit alone does not identify
their effective source.

The checksum-bound embedded model and fit-ledger files are checked out and
written with LF line endings. Their byte identities must not depend on a
platform's default text newline translation.
