window.BENCHMARK_DATA = {
  "lastUpdate": 1775496136897,
  "repoUrl": "https://github.com/bvolpato/ivygrep",
  "entries": {
    "Rust Benchmark": [
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "989f41deb27bb861a8d9af3ca2923beff4eb653f",
          "message": "docs: test benchmark action (#3)\n\n* docs: formatting trigger\n\n* fix: explicitly convert criterion output into custom json benchmark structure\n\n* build: implement robust cargo dependency caching across workflows\n\n* debug benchmark output",
          "timestamp": "2026-04-05T18:10:50-04:00",
          "tree_id": "8bc32db12fe32e823c2ef7c9e5148e1f139071f2",
          "url": "https://github.com/bvolpato/ivygrep/commit/989f41deb27bb861a8d9af3ca2923beff4eb653f"
        },
        "date": 1775427507802,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 691021171.6,
            "unit": "ns"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "8104515f79cc769ca6c00408894b12e4a77b02a2",
          "message": "chore: remove debug output from benchmarks",
          "timestamp": "2026-04-05T18:21:08-04:00",
          "tree_id": "789e37bed2a0dc975186c27ee27481894d7a8b06",
          "url": "https://github.com/bvolpato/ivygrep/commit/8104515f79cc769ca6c00408894b12e4a77b02a2"
        },
        "date": 1775427757512,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 710680738.8,
            "unit": "ns"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8ba4962207f786f425f9cc25d73d24f788295d0b",
          "message": "chore: fix formatting in benchmarks (#5)",
          "timestamp": "2026-04-05T18:29:35-04:00",
          "tree_id": "7eb2100bb98b3ed2c6f50a117f3aafb325011011",
          "url": "https://github.com/bvolpato/ivygrep/commit/8ba4962207f786f425f9cc25d73d24f788295d0b"
        },
        "date": 1775428255964,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 698828500.2,
            "unit": "ns"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "54e1f5ddb1881b507f3a92661c696f02e1d5652b",
          "message": "fix: resolve Tantivy LockBusy on Linux\n\nClear stale .tantivy-writer.lock before IndexWriter init with retry\nbackoff. The fs2 advisory lock already guarantees exclusive access,\nso any lingering lock file is safe to remove.\n\nMake --rm wait for in-progress indexing by acquiring the fs2 lock\nbefore deleting the index directory, preventing races between the\ndaemon and CLI.",
          "timestamp": "2026-04-05T19:33:09-04:00",
          "tree_id": "41c7a244f0136d2d3d3693a023c006331a7b5e21",
          "url": "https://github.com/bvolpato/ivygrep/commit/54e1f5ddb1881b507f3a92661c696f02e1d5652b"
        },
        "date": 1775432076825,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 584304539.8,
            "unit": "ns"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "425fac461c3976dac4885ebf8abe4e998af86b63",
          "message": "bench: expand suite to 10 benchmarks, show µs in PR comments\n\nAdd chunking (Rust + Python), merkle (scan + diff), hash embedding\n(single + batch), search (hybrid + literal), and incremental reindex\nbenchmarks. Convert Criterion output from nanoseconds to microseconds\nfor readable PR comments.",
          "timestamp": "2026-04-05T19:52:46-04:00",
          "tree_id": "be1971ce6da4834de9e4132fa0c493ce42fc437e",
          "url": "https://github.com/bvolpato/ivygrep/commit/425fac461c3976dac4885ebf8abe4e998af86b63"
        },
        "date": 1775433383140,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 841169320,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6875.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3784,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2712.34,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 8671.17,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 8583.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 538.54,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15343.97,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 6723.22,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "788626fa5db1bd1ae5318cad97289fd84d136aab",
          "message": "chore: re-trigger CI after removing stale app integrations",
          "timestamp": "2026-04-05T22:08:43-04:00",
          "tree_id": "be1971ce6da4834de9e4132fa0c493ce42fc437e",
          "url": "https://github.com/bvolpato/ivygrep/commit/788626fa5db1bd1ae5318cad97289fd84d136aab"
        },
        "date": 1775441544510,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 796344260,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6772.25,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3846.99,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2729.01,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 8913.92,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 8710.35,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.08,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 543.38,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15367.01,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 6839.89,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "8b9e0c134fdaabce74f9516b218684e9c576fc39",
          "message": "chore: bump version to 0.5.8",
          "timestamp": "2026-04-05T22:20:38-04:00",
          "tree_id": "3fb8bab08d2edd64668e8b9373c29b9cdfca73bc",
          "url": "https://github.com/bvolpato/ivygrep/commit/8b9e0c134fdaabce74f9516b218684e9c576fc39"
        },
        "date": 1775442258568,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 825041600,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6834.4,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3903.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2748.88,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9112.58,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 8942.22,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 522.33,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15789.33,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7053.77,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "b4a6160432038aff4fab9fb2c4f78e2d57b571b5",
          "message": "chore: prepare release v0.5.10",
          "timestamp": "2026-04-05T23:43:36-04:00",
          "tree_id": "4280440d243eeb749f74a1fdf781e3eb9bf7b92c",
          "url": "https://github.com/bvolpato/ivygrep/commit/b4a6160432038aff4fab9fb2c4f78e2d57b571b5"
        },
        "date": 1775447235163,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 848420100,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6569.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3813.98,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2705.54,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 8772.62,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 8742.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.05,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 546.27,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15526.57,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 6844.2,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "42d4cf1fe25c126bffa854bba90a3bd678da9b83",
          "message": "chore: restore Cargo.lock to fix benchmark github-action checkout",
          "timestamp": "2026-04-06T11:10:58-04:00",
          "tree_id": "c20c0f5f5e0cdf870c11047d6a45c77944d0d90e",
          "url": "https://github.com/bvolpato/ivygrep/commit/42d4cf1fe25c126bffa854bba90a3bd678da9b83"
        },
        "date": 1775488512572,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 866498470,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6553.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3878.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2706.34,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 8856.6,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 8689.51,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.32,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 527.59,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15542.1,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 6817.07,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "13f7bec4c573c78e7a11c5f6ef0dcbfe4f3721a4",
          "message": "chore: run rustfmt to fix ci",
          "timestamp": "2026-04-06T11:15:36-04:00",
          "tree_id": "8e8f188b78e84ce87598acf1ed0f5f53f4d28e4a",
          "url": "https://github.com/bvolpato/ivygrep/commit/13f7bec4c573c78e7a11c5f6ef0dcbfe4f3721a4"
        },
        "date": 1775488758036,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 846941240,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6810.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3762.34,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2660.41,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9384.38,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9302.42,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 548.63,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15886.68,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7300.36,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "92e75485903d401d4d8da5bc20a319a7e58f6981",
          "message": "perf: optimize initial indexing for large repositories\n\n- Skip redundant remove_file_chunks on fresh index (no data to delete)\n- Use INSERT instead of INSERT OR REPLACE on fresh index (skip conflict check)\n- Switch Merkle snapshot to parallel walker (build_parallel vs build+par_iter)\n- Enable SQLite WAL mode with larger page cache and in-memory temp storage\n- Increase Tantivy writer heap from 50MB to 200MB (fewer forced commits)\n- Lower periodic commit threshold from 100K to 50K chunks\n- Batch SystemTime::now() per file instead of per chunk (1M+ fewer syscalls)\n- Use compact JSON for Merkle snapshot serialization\n- Reduce progress I/O frequency (500/2000 vs 100/500)\n- Fix cargo fmt formatting issues in cli.rs, embedding.rs, workspace.rs",
          "timestamp": "2026-04-06T12:43:26-04:00",
          "tree_id": "8b280a94d67324aef5e43e522572b33aad801e5e",
          "url": "https://github.com/bvolpato/ivygrep/commit/92e75485903d401d4d8da5bc20a319a7e58f6981"
        },
        "date": 1775494191976,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 842796960,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8405.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3814.7,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2725.45,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10801.06,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10598.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.21,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15809.3,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7137.1,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "16de1441ebc45190a35f535bcf3b8d36b64a382f",
          "message": "chore: bump version to 0.5.12",
          "timestamp": "2026-04-06T13:18:43-04:00",
          "tree_id": "e26b5e74019bd12d405a191baa45ebac3f01f61a",
          "url": "https://github.com/bvolpato/ivygrep/commit/16de1441ebc45190a35f535bcf3b8d36b64a382f"
        },
        "date": 1775496136503,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 842968460,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7995.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3818.79,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2693.09,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10870.66,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10497.16,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.79,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 529.7,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15705.69,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7005.36,
            "unit": "µs"
          }
        ]
      }
    ]
  }
}