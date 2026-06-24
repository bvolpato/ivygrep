# ivygrep vs Semble

Generated: 2026-06-21T06:39:03.132511+00:00

Semble: `30d36ad1bd7467cbe325b59bc9035640ac4e5dca` (0.4.0)
ivygrep: `c21a223cb2dc0b9bbf2f4067f2b73893ee849948`

| Metric | ivygrep | Semble | Winner |
|---|---:|---:|---|
| nDCG@10 | 0.657 | 0.801 | Semble |
| Warm query p50 | 16.49 ms | 7.92 ms | Semble |
| Warm query p95 | 30.22 ms | 23.45 ms | Semble |
| Mean returned tokens | 251 | 1593 | ivygrep |

## Indexing

```json
{
  "axum": {
    "semble": {
      "index_ms": 1770.6415310385637,
      "index_bytes": 2285381,
      "chunks": 1034
    },
    "ivygrep": {
      "lexical_ms": 222.9183299932629,
      "hash_ms": 417.8926380118355,
      "neural_ms": 1167.313362006098,
      "ready_ms": 1808.1243300111964,
      "index_bytes": 3465252,
      "chunks": 1877
    }
  },
  "fastapi": {
    "semble": {
      "index_ms": 1207.6125970343128,
      "index_bytes": 2567125,
      "chunks": 1188
    },
    "ivygrep": {
      "lexical_ms": 107.1693699923344,
      "hash_ms": 122.44707095669582,
      "neural_ms": 244.3774159764871,
      "ready_ms": 473.9938569255173,
      "index_bytes": 1425897,
      "chunks": 509
    }
  },
  "trpc": {
    "semble": {
      "index_ms": 582.7742809779011,
      "index_bytes": 1561937,
      "chunks": 690
    },
    "ivygrep": {
      "lexical_ms": 95.97174497321248,
      "hash_ms": 170.5670109950006,
      "neural_ms": 354.223545989953,
      "ready_ms": 620.7623019581661,
      "index_bytes": 2607090,
      "chunks": 1370
    }
  }
}
```

## One-file refresh

| Metric | ivygrep | Semble |
|---|---:|---:|
| Searchable lexical refresh | 64.07 ms | n/a |
| Full hybrid refresh | 261.77 ms | 826.04 ms |

## Notes

- Same pinned repositories, queries, labels, top-k, and nDCG implementation as Semble.
- Semble runs in-process, matching its official benchmark.
- ivygrep runs through its persistent daemon protocol, excluding CLI process startup.
- Model load is reported separately from per-repository indexing.
