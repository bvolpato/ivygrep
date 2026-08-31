# Public evaluation contracts

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

Schema 2 separates verified `reference` model/ledger checksums from
`executed_binary.applicability`, which remains `unverified`. A supplied binary
can embed different weights, including weights with the same model ID. Neither
that ID nor the binary's own checksum binds its embedded model to the checkout
reference. No executed-model checksum attestation is currently available, so
even zero reference-ID overlap does not certify fit disjointness for the
evaluated binary. Observed binary checksums and model IDs are informational only.

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

Replace `BUILD_COMMIT` with the exact binary build commit, not the current
checkout when it differs.

```bash
HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 IVYGREP_RERANKER=learned \
  uv run scripts/eval_code_retrieval.py \
    --dataset /tmp/ivygrep-reranker-fit/codetrans-dl \
    --binary target/release/ig --source-commit BUILD_COMMIT \
    --mode blended --capture-reranker \
    --output /tmp/ivygrep-reranker-traces/codetrans-dl.json
```

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
