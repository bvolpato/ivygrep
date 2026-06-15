# Public code-retrieval evaluation

`scripts/eval_code_retrieval.py` accepts the BEIR/CoIR three-file layout:

- `corpus.jsonl`: `_id`, `text`, optional `title`, and optional `metadata.path`
- `queries.jsonl`: `_id` and `text`
- `qrels.tsv`: `query-id`, `corpus-id`, and graded `score`

Run the offline fixture:

```bash
./build.sh
python3 scripts/eval_code_retrieval.py \
  --dataset tests/fixtures/retrieval \
  --binary target/release/ig \
  --mode hash
```

The JSON result reports nDCG@10, MRR@10, precision@5, recall@20,
index time/size, and process-cold versus warm-query latency percentiles.
`warm_query_path` records whether the warm measurement used the daemon;
lexical-only mode remains a local process by design. Neural mode waits for an
explicit daemon model-ready signal before collecting warm results.

Available modes are:

- `lexical`: BM25/path/signature retrieval without vectors
- `hash`: lexical retrieval plus the deterministic 256-dimensional hash tier
- `hybrid`: the normal query path with a completed hash tier
- `neural`: the normal query path after verified neural enhancement

Neural runs fail rather than silently reporting hash fallback results. Set
`IVYGREP_MODEL_PROFILE=code` to evaluate the pinned `code-minilm-l6-v1`
profile. Result JSON is suitable for checked-in history or external baseline
comparison.

## CoIR

Use `coir-team/coir` at commit
`89d0e769c18f0576a766072ba1071d4e04cca3dd`. Export each dataset to the
three-file layout above, preserving graded qrels.

## CodeSearchNet

Use `github/CodeSearchNet` at commit
`106e827405c968597da938f6b373d30183918869`. Treat function documentation as
queries, function bodies as corpus documents, and the paired function as the
positive qrel. Record the source repository and language in `metadata`.

Large public datasets are intentionally downloaded outside this repository.
Store dataset revisions and checksums alongside result artifacts.
