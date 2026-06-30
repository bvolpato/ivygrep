window.BENCHMARK_DATA = {
  "lastUpdate": 1782790884411,
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
          "id": "df1cb852deca3ae416cf0f5a1243c51986ee0009",
          "message": "indexer: optimize initial indexing and handle backward compatibility for is_ignored\n\nThis commit improves the performance of the initial indexing step by running the hashing model synchronously, pushing the neural indexing into the background daemon. Additionally, it implements robust backward compatibility for tantivy field 'is_ignored', and safely limits the cpu affinity for the background fastembed model.",
          "timestamp": "2026-04-06T22:36:34-04:00",
          "tree_id": "b7de51e002b8c8a1cd733d1f6ed0ba64c5590375",
          "url": "https://github.com/bvolpato/ivygrep/commit/df1cb852deca3ae416cf0f5a1243c51986ee0009"
        },
        "date": 1775529619123,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 839020030,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8173.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3859.06,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2748.79,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11319.56,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10683.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 521.48,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16306.96,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7143.94,
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
          "id": "ad27fe46e3b466204aad2fd747abe40d802c4fa6",
          "message": "embedding: fix unused variable on macOS CI\n\nMove budget variable inside #[cfg(target_os = linux)] block so it is not unused on macOS where sched_setaffinity is unavailable.",
          "timestamp": "2026-04-07T08:51:06-04:00",
          "tree_id": "5b283a42ed7853032e1084a15cf90d92b529a795",
          "url": "https://github.com/bvolpato/ivygrep/commit/ad27fe46e3b466204aad2fd747abe40d802c4fa6"
        },
        "date": 1775566493287,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 771388900,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8451.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3832.88,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2722.04,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10701.34,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10496.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 521.05,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16231.12,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7133.24,
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
          "id": "8d1068ccf8d87edfed9a753dedf89633a06553f9",
          "message": "embedding: gate ort_thread_budget to linux only\n\nThe function is only called from the linux-specific sched_setaffinity block. On macOS it was flagged as dead code by -D warnings.",
          "timestamp": "2026-04-07T08:53:27-04:00",
          "tree_id": "9f90134705e8d6840cd73088de263766ff2374be",
          "url": "https://github.com/bvolpato/ivygrep/commit/8d1068ccf8d87edfed9a753dedf89633a06553f9"
        },
        "date": 1775566627929,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 846986270,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7839.18,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3827.76,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2723.58,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11040.3,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10694.59,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.69,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 521.44,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15792.67,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7109.09,
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
          "id": "08b1c3dd617d18a58f365ee7b1e3de6c2ff4e6a4",
          "message": "release: v0.5.13\n\nPerformance: 32x larger enhancement batches, CPU affinity limiting, instant initial indexing. Fixes: is_ignored backward compatibility, honest CUDA detection.",
          "timestamp": "2026-04-07T12:56:59-04:00",
          "tree_id": "2113912a4833dab22640fbdf10c77010b7ae3cf6",
          "url": "https://github.com/bvolpato/ivygrep/commit/08b1c3dd617d18a58f365ee7b1e3de6c2ff4e6a4"
        },
        "date": 1775581235192,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 838620180,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8043.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3928.81,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2744.46,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11049.66,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10708.67,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 512.75,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16080.03,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7173.86,
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
          "id": "75446e9a093e4800079a8f6b1cbdef72dff95ce3",
          "message": "test: comprehensive coverage for walker, embedding, chunking, and benchmarks\n\n- walker: 4 tests for .git exclusion, hidden files, gitignore, skip_gitignore\n- embedding: 10 new tests covering batch embed, normalization, token variants, factory fns\n- chunking: 7 new tests for Go, TypeScript, Java, Python class, JSON, YAML\n- benches: added regex_search and vector_store benchmark groups\n\nUnit tests: 96 → 116",
          "timestamp": "2026-04-07T19:52:40-04:00",
          "tree_id": "eb03c16b806043c33881a04dc794b75ea1072d71",
          "url": "https://github.com/bvolpato/ivygrep/commit/75446e9a093e4800079a8f6b1cbdef72dff95ce3"
        },
        "date": 1775606225522,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 778310610,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8365.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3744.37,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2636.73,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11982.33,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11368.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 520.86,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16659.28,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7707.57,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5141.64,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473991.16,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 598.17,
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
          "id": "6c5744db4156e2ddae3876316ebe5d744ff2a7b0",
          "message": "fix(cli): fallback to regex search when ignoring gitignore overrides index state\n\nWhen an index is built without '--skip-gitignore', it does not contain\nignored files. If a user subsequently searches with '--skip-gitignore',\nthe literal search against the index will fail to find those files.\n\nThis commit detects when a user requests '--skip-gitignore' but the\ntarget workspace(s) index metadata shows it was built with the default\nbehavior (excluding ignored files). In such cases, we automatically\nfallback to a regex search (which crawls the filesystem) to ensure\nthe search results correctly include ignored files.\n\nAlso adds an integration test to validate '--skip-gitignore' correctly\noverrides '.gitignore' exclusions during search operations.",
          "timestamp": "2026-04-08T11:36:45-04:00",
          "tree_id": "fa0be0ead034176ec0691d98a43cd68a4a3b6f33",
          "url": "https://github.com/bvolpato/ivygrep/commit/6c5744db4156e2ddae3876316ebe5d744ff2a7b0"
        },
        "date": 1775662970499,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 833475970,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8546.24,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3837.53,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2691.94,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11697.16,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11166,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.79,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.32,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16368.34,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7418.53,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 4957.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450507.55,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 620.61,
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
          "id": "a60f452706ed2da79048697254f6b9456c25d650",
          "message": "fix(cli): rely on SQLite filtering and trigger re-index for skip-gitignore",
          "timestamp": "2026-04-08T11:48:37-04:00",
          "tree_id": "95a5f95f3371667f3fdf0c44d3747ba0e4f344f3",
          "url": "https://github.com/bvolpato/ivygrep/commit/a60f452706ed2da79048697254f6b9456c25d650"
        },
        "date": 1775663896863,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 793987980,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8348.15,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3725.54,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2639.41,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11513.77,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11167.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.54,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 518.86,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16321.67,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7672.77,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5179.22,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472980.16,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 573.07,
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
          "id": "c1cdb0d3158a3819a57fa910f0c39a0010a162ec",
          "message": "fix(indexer): drastically reduce batch sizes to prevent memory ballooning and respect skip-gitignore on first run",
          "timestamp": "2026-04-08T12:02:35-04:00",
          "tree_id": "ab1fe69883064861ad4541c716fbfbd1dcb66de7",
          "url": "https://github.com/bvolpato/ivygrep/commit/c1cdb0d3158a3819a57fa910f0c39a0010a162ec"
        },
        "date": 1775664425357,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 721921590,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8056.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3830.79,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2706.77,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10886.27,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10609.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.73,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 522.48,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15803.41,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7067.94,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 4881.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 447495.2,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 474.45,
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
          "id": "c1be7950c7edbe895f0316f692de7120e4955187",
          "message": "fix: correctly initialize workspace metadata on first run to persist gitignore logic early",
          "timestamp": "2026-04-08T12:06:26-04:00",
          "tree_id": "e8beb29cd69e6a1e8478b6e08204067424653938",
          "url": "https://github.com/bvolpato/ivygrep/commit/c1be7950c7edbe895f0316f692de7120e4955187"
        },
        "date": 1775664657249,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 749040050,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8155.56,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3800.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2640.71,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11466.38,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11188.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 509.66,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16352.52,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7626.22,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5138.62,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472820.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 569.67,
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
          "id": "0acde5444af04ca6938f13a727cfbf5973d93179",
          "message": "chore: release 0.5.14",
          "timestamp": "2026-04-08T12:23:22-04:00",
          "tree_id": "f443732fa4baa30231139fe7a783746b0771b1b1",
          "url": "https://github.com/bvolpato/ivygrep/commit/0acde5444af04ca6938f13a727cfbf5973d93179"
        },
        "date": 1775665668737,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 777879080,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8144.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3888.46,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2718.25,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11311.12,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10629.29,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.64,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 514.9,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16320.23,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7174.71,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 4878.19,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450296.28,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 529.57,
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
          "id": "409921913aa547c4b743ec79e6f98a7ce701ee9d",
          "message": "fix(search): enforce case-insensitive matching and fix gitignore filter pipeline",
          "timestamp": "2026-04-08T15:01:44-04:00",
          "tree_id": "f8a79bf7f2a0f088d64b3ba90784af3d7539d4e6",
          "url": "https://github.com/bvolpato/ivygrep/commit/409921913aa547c4b743ec79e6f98a7ce701ee9d"
        },
        "date": 1775675182934,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 758261660,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8478.16,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3877.7,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2755.6,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11245.77,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10927.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 527.4,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15991.69,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8562.35,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5323.3,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450950.45,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 642.16,
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
          "id": "5251af637260a4689154e7ca4459c339747cef4c",
          "message": "perf(search): massive memory and speed improvement via lazy parallel zstd decompression",
          "timestamp": "2026-04-08T16:01:29-04:00",
          "tree_id": "1f0f8273a79c0c91905e1de1a85512f3336f3542",
          "url": "https://github.com/bvolpato/ivygrep/commit/5251af637260a4689154e7ca4459c339747cef4c"
        },
        "date": 1775678752867,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 703050430,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8294.91,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3841.64,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2721.57,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10935.31,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10743.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.79,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 524.51,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15745.35,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7796.49,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5200.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449673.17,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 523.97,
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
          "id": "36ef083f48bda960da8c0e7a1706e8822bd30947",
          "message": "chore: version 0.5.16",
          "timestamp": "2026-04-08T16:21:05-04:00",
          "tree_id": "9b42144b4a4f183ffad149780bc3ea4e95d59b04",
          "url": "https://github.com/bvolpato/ivygrep/commit/36ef083f48bda960da8c0e7a1706e8822bd30947"
        },
        "date": 1775679937083,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 811061140,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8179.42,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3815.11,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2693.79,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11241.75,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10835.67,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 553.97,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15799.06,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7853.16,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5302.77,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 461994.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 531.76,
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
          "id": "2f07b47cc65cd935d5db67b9488073fe42eca892",
          "message": "perf(search): index-backed literal search + hybrid literal fusion\n\nTwo major improvements:\n\n1. literal_search now uses the Tantivy inverted index as a pre-filter\n   instead of scanning every chunk from SQLite. This drops literal\n   search from O(all_chunks) to O(index_lookup + verified_candidates),\n   making it 20-40x faster on large repos.\n\n2. hybrid_search now always includes a literal pass that feeds verified\n   exact substring matches into the RRF fusion with a strong weight.\n   This ensures 'ig openai' surfaces files containing 'OpenAI' even\n   when tokenization splits the term differently.",
          "timestamp": "2026-04-08T20:16:16-04:00",
          "tree_id": "263154d4226b0ae6419a915558a458b26fc17d99",
          "url": "https://github.com/bvolpato/ivygrep/commit/2f07b47cc65cd935d5db67b9488073fe42eca892"
        },
        "date": 1775694047293,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 721541450,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8671.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3847.36,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2720.36,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10933.85,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10592.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 538.62,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19100.87,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10456.58,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5184.71,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449665.26,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 503.81,
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
          "id": "80ebed5d7c0b9a7bcdeaef4c55a7637febe0b172",
          "message": "chore: version 0.5.17",
          "timestamp": "2026-04-08T20:16:36-04:00",
          "tree_id": "a2aa27d0fc66123e704d64cdd2613f3277c5156c",
          "url": "https://github.com/bvolpato/ivygrep/commit/80ebed5d7c0b9a7bcdeaef4c55a7637febe0b172"
        },
        "date": 1775694060995,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 705613530,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8205.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3747.86,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2652.94,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11670.54,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11382.04,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 512.01,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19939.31,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11163.41,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5523.18,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472165.72,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 571.47,
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
          "id": "b6f3e3e4a6e1f1964f8ed15aa587a0af0e5c43e1",
          "message": "feat: rename --all to --all-indices, support absolute paths, implement --no-limit",
          "timestamp": "2026-04-08T21:00:17-04:00",
          "tree_id": "8a1dee133fa7c0b96fd1fced74c312e27fd50762",
          "url": "https://github.com/bvolpato/ivygrep/commit/b6f3e3e4a6e1f1964f8ed15aa587a0af0e5c43e1"
        },
        "date": 1775696686211,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 695176520,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8384.31,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3834.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2710.84,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10872.72,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10466.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.59,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 518.56,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19375.46,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10620.64,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5180.87,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450143.39,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 515.98,
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
          "id": "82c2ad4d94f84fab3531cff7c75e8270ac9f7914",
          "message": "feat(indexing): prevent indexing nested repositories",
          "timestamp": "2026-04-08T21:11:16-04:00",
          "tree_id": "ecf4f3818c8b950c6acba5ba0420609977dc3b5e",
          "url": "https://github.com/bvolpato/ivygrep/commit/82c2ad4d94f84fab3531cff7c75e8270ac9f7914"
        },
        "date": 1775697350731,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 751818550,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8619.75,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3772.61,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2649.3,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 12159.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11701.29,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 514.76,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20563.98,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11480.07,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5594.2,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472906.16,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 606.19,
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
          "id": "44f084645798ab815164c1b8e6e5f373187fa181",
          "message": "fix: compile release binaries with default neural feature enabled",
          "timestamp": "2026-04-08T21:27:01-04:00",
          "tree_id": "2e7d047c0e3bd6de2854d9756bc284a4f4595352",
          "url": "https://github.com/bvolpato/ivygrep/commit/44f084645798ab815164c1b8e6e5f373187fa181"
        },
        "date": 1775698283208,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 804146490,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8256.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3861.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2734.95,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10879.11,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10534.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.6,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 520.95,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19445.94,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10700.16,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5192.29,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449938.93,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 535.65,
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
          "id": "a0c3f09c04f9b8f6d8cc6deea8eeb7e46521f46f",
          "message": "fix(cli): prevent capacity overflow panic for file-name-only unbounded search and default it to no limit; add alias for all-indices",
          "timestamp": "2026-04-08T21:33:01-04:00",
          "tree_id": "d4356aa7848969189a21580af60cfc930e130241",
          "url": "https://github.com/bvolpato/ivygrep/commit/a0c3f09c04f9b8f6d8cc6deea8eeb7e46521f46f"
        },
        "date": 1775698650336,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 776995440,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8360.83,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3762.77,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2642.91,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11590.99,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11300.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 513.47,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15348.23,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11249.62,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5567.53,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472380.89,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 604.11,
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
          "id": "ceb7cf238a3e7c90d5f025940ac77f05f0c152e0",
          "message": "fix(release): add dual linux builds to resolve glibc cross-compilation errors",
          "timestamp": "2026-04-08T21:40:03-04:00",
          "tree_id": "520f44cc2ebbf9272695dd3af6b42b131bcc5433",
          "url": "https://github.com/bvolpato/ivygrep/commit/ceb7cf238a3e7c90d5f025940ac77f05f0c152e0"
        },
        "date": 1775699066001,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 788303570,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8088.79,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3757.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2644.66,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11590.03,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11424.42,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.53,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 508.93,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15384.75,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11216.64,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5583.6,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471610.68,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 575.09,
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
          "id": "72a6835688d9e7eeed272b836e29c593caeaa958",
          "message": "chore: enable native aarch64-linux-gnu build to restore neural features",
          "timestamp": "2026-04-08T21:43:22-04:00",
          "tree_id": "593fd9880baae69461d89caab00c35254ecb21c7",
          "url": "https://github.com/bvolpato/ivygrep/commit/72a6835688d9e7eeed272b836e29c593caeaa958"
        },
        "date": 1775699273250,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 755768650,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8254.95,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3918.82,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2718.27,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11069.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10892.94,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.64,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 524.24,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14762.75,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10565.08,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5195.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450136.45,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 535.81,
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
          "id": "2bbac10804091bbc6afec6b2f1d6d6694dd8690a",
          "message": "fix(search): prevent literal match discard in hybrid search\n\nLiteral exact matches were being incorrectly filtered out by the\nadaptive threshold scoring system during hybrid search if their\nbase BM25 scores were low relative to other semantic matches, leading\nto inconsistent result counts between case-sensitive and case-insensitive\ninvocations that expanded search scopes. We now explicitly bypass\nthe threshold for any hits carrying the 'literal' provenance tag.",
          "timestamp": "2026-04-08T21:54:08-04:00",
          "tree_id": "d69c3bb1a0f0112fa29fdcc29d2364ee70ade673",
          "url": "https://github.com/bvolpato/ivygrep/commit/2bbac10804091bbc6afec6b2f1d6d6694dd8690a"
        },
        "date": 1775699917785,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 721611310,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8175.98,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3792.33,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2706.18,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10907.92,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10622.88,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.66,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 520.42,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14425.24,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10398.96,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5147.99,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450042.74,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 548.51,
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
          "id": "b2837a883da93c331bc4a1f63dbf972103c9f936",
          "message": "fix(tests): resolve macos tmpdir path mismatch in nested index test",
          "timestamp": "2026-04-08T21:58:57-04:00",
          "tree_id": "32a7fd85575f7e3f4cf9a68a970fcbf011507231",
          "url": "https://github.com/bvolpato/ivygrep/commit/b2837a883da93c331bc4a1f63dbf972103c9f936"
        },
        "date": 1775700205958,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 804876130,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8232.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3851.42,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2736.9,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11130.13,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11017.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.66,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.61,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15056.08,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10776.7,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5302.68,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450950.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 602.82,
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
          "id": "198f842d8f4fb1db6e9c20e767af2ae1fe1dc94a",
          "message": "chore: fmt",
          "timestamp": "2026-04-08T22:10:33-04:00",
          "tree_id": "558990f1a88f57b88e92b0dc54f0ae1da75bebd8",
          "url": "https://github.com/bvolpato/ivygrep/commit/198f842d8f4fb1db6e9c20e767af2ae1fe1dc94a"
        },
        "date": 1775700902106,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 711393610,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8482.77,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3839.91,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2708.81,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11082.25,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10740.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.86,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 518.42,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14834.93,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10529.11,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5231.68,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449983.24,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 535.99,
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
          "id": "05402f60e88c81dbfe76de9b5b6c011697f13138",
          "message": "chore: bump version to 0.5.26",
          "timestamp": "2026-04-08T22:11:25-04:00",
          "tree_id": "a7805cf6af42fb916eec8e0e37b30b3d4c021ac9",
          "url": "https://github.com/bvolpato/ivygrep/commit/05402f60e88c81dbfe76de9b5b6c011697f13138"
        },
        "date": 1775700955961,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 819045540,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8425.02,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3903.38,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2723.48,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 12102.9,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11712.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.74,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 519.9,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15632.48,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11508.24,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6232.48,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472727.49,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 608.3,
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
          "id": "9c434eef139e7af7feaf7cfa6a9a4c9b064df0b7",
          "message": "fix(ci): fix release pipeline for arm and macos",
          "timestamp": "2026-04-08T22:20:59-04:00",
          "tree_id": "b1a06fb13431504de9bcbbcb90376ace947f5ea4",
          "url": "https://github.com/bvolpato/ivygrep/commit/9c434eef139e7af7feaf7cfa6a9a4c9b064df0b7"
        },
        "date": 1775701523911,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 804613250,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8160.38,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3835.21,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2685.39,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10897.57,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10645.89,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.72,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 525.46,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14735.11,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10533.45,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5185.58,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449274.92,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 491.41,
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
          "id": "2a1bc4e7754050fce38a190fbefbb20b7e2b58ee",
          "message": "fix(ci): update macos intel runner to macos-15-intel",
          "timestamp": "2026-04-08T22:22:52-04:00",
          "tree_id": "35903a3064f70b9d30d606ee940d6261a1da9854",
          "url": "https://github.com/bvolpato/ivygrep/commit/2a1bc4e7754050fce38a190fbefbb20b7e2b58ee"
        },
        "date": 1775701634817,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 797919470,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8050.47,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3755.83,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2647.95,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11643.24,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11326.31,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.88,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.43,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15287.52,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11182,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5501.38,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472635.84,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 630.44,
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
          "id": "eff6a4fe3cf5f505e78715c14e7823118e95a5da",
          "message": "fix(ci): disable default features for macos-x86_64 to drop onnx dependency",
          "timestamp": "2026-04-08T23:03:40-04:00",
          "tree_id": "e9b5e310b47770e3179c90aabb2a1a6e66446c3d",
          "url": "https://github.com/bvolpato/ivygrep/commit/eff6a4fe3cf5f505e78715c14e7823118e95a5da"
        },
        "date": 1775704086596,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 783325130,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7796.24,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3823.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2692.1,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10884.88,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10611.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.75,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.62,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14571.31,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10481.1,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5177.41,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450536.01,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 549.46,
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
          "id": "10712ad1840fbf5334cb4adadbbd129f354269ef",
          "message": "chore: bump linux-x86_64-gnu runner to ubuntu-22.04",
          "timestamp": "2026-04-08T23:56:21-04:00",
          "tree_id": "43acfbcd7f8e4c32bbb012b89b6e1dd2cb8cd9ca",
          "url": "https://github.com/bvolpato/ivygrep/commit/10712ad1840fbf5334cb4adadbbd129f354269ef"
        },
        "date": 1775707252074,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 682088390,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8292.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3810.16,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2683.07,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10753.27,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10502.08,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.97,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14589.66,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10493.13,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5207.94,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449325.09,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 516.25,
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
          "id": "074873e4897439d435994baabcf536f53457875b",
          "message": "chore: bump linux-x86_64-gnu runner to ubuntu-latest",
          "timestamp": "2026-04-09T00:15:32-04:00",
          "tree_id": "839554718cae2ca9b9087b7c6d8667922ebd0d39",
          "url": "https://github.com/bvolpato/ivygrep/commit/074873e4897439d435994baabcf536f53457875b"
        },
        "date": 1775708413503,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 853435540,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7074.7,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2900.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2032.46,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9309.74,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9260.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 5.12,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 396.07,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 11860.62,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8712.97,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 4350.3,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 376428.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 442.03,
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
          "id": "1f165f3e72834a826be18bfb4ab6635161f5d4cf",
          "message": "fix: homebrew linux archive names + add ivygrep-static musl formula",
          "timestamp": "2026-04-09T10:07:49-04:00",
          "tree_id": "f5f44058a37309d9ee43570adf626ee719c0b76a",
          "url": "https://github.com/bvolpato/ivygrep/commit/1f165f3e72834a826be18bfb4ab6635161f5d4cf"
        },
        "date": 1775743939041,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 781639900,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8100.7,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3823.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2694.71,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11020.33,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10693.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.13,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 548.24,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14795.41,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10586.53,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5447.8,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450415.19,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 581.7,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ab9e59cd20b5399009ce566192ff000e141e2d1c",
          "message": "feat(neural): replace fastembed with candle_embed for universal static binaries (#6)\n\n* feat(neural): replace fastembed with candle_embed for universal static binaries\n\nReplaces fastembed/ort with pure-Rust candle_embed, making the neural feature available on static (musl) Linux builds without requiring dynamic glibc ONNX Runtime linkages. Downgrades half to 2.3.1 to avoid a rand_distr dependency mismatch with candle-core.\n\n* ci: prune linux-gnu targets from release matrix\n\nSince candle_embed produces exactly the same feature-rich neural binaries natively for musl and completely independently of any glibc ONNX installations, there's no need to build or distribute the legacy x86_64/aarch64 GNU fallback targets. The Linux static musl binaries provide complete platform-agnostic distribution.",
          "timestamp": "2026-04-09T11:03:53-04:00",
          "tree_id": "fafaccd6dac4b5020c5d77b7619787fb25a7f9de",
          "url": "https://github.com/bvolpato/ivygrep/commit/ab9e59cd20b5399009ce566192ff000e141e2d1c"
        },
        "date": 1775747483323,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 761225620,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8129.75,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3829.34,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2716.65,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10769.17,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10508.2,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 522.1,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14654.61,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10503.89,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5177.43,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450308.88,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 515.16,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "81aa15334e06bb9167a84845f808cc5fc405364b",
          "message": "chore: release v0.5.33 (#7)",
          "timestamp": "2026-04-09T11:04:58-04:00",
          "tree_id": "8003dbfb6005a1989513a655dc0043f74e1a42dd",
          "url": "https://github.com/bvolpato/ivygrep/commit/81aa15334e06bb9167a84845f808cc5fc405364b"
        },
        "date": 1775747573663,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 797007310,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8184.91,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3948.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2695.5,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11104.37,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10513.81,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 521.97,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14708.36,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10649.61,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5208.14,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451238.49,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 523.95,
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
          "id": "95936c276c2f879f89b02278da21dc3de922654d",
          "message": "fix(ci): remove --no-default-features to allow neural in release binaries",
          "timestamp": "2026-04-09T11:29:52-04:00",
          "tree_id": "a135313a29d772cedebb4d3182138a3afad65bb4",
          "url": "https://github.com/bvolpato/ivygrep/commit/95936c276c2f879f89b02278da21dc3de922654d"
        },
        "date": 1775748864580,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 853948620,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 5042.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3680.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2587.41,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 4851.37,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4849.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.88,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 733.45,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 10393.49,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 6841.54,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 2756.84,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472279,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 395.11,
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
          "id": "dab3b83c5dc0d73f1fcca5da8b18e00edd0db0f9",
          "message": "chore: release v0.5.35",
          "timestamp": "2026-04-09T11:57:44-04:00",
          "tree_id": "896d744e366b91f32f684d1af4d4410d374198c4",
          "url": "https://github.com/bvolpato/ivygrep/commit/dab3b83c5dc0d73f1fcca5da8b18e00edd0db0f9"
        },
        "date": 1775750636687,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 750216900,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8193.03,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 4009.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2788.16,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11667.48,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11222.83,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.57,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 522.69,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14941.57,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10673.15,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5203.52,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451457.47,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 537.08,
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
          "id": "be24f1a568491e1ddde6eef46e8b216ffa6e293e",
          "message": "fix(neural): prepend endpoint to relative redirects in hf-hub to resolve RelativeUrlWithoutBase",
          "timestamp": "2026-04-09T13:48:31-04:00",
          "tree_id": "1ff47e5167d20366712df2db39f9791d241215cc",
          "url": "https://github.com/bvolpato/ivygrep/commit/be24f1a568491e1ddde6eef46e8b216ffa6e293e"
        },
        "date": 1775757188750,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 783167210,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8246.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3839.51,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2690.95,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10818.68,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10635.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 568.51,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14849.58,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10594.48,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5189.6,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451210.59,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 577,
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
          "id": "15b9b48e52cc6e4e737878703e9e841f5ee60136",
          "message": "fix(neural): prepend endpoint to relative redirects in hf-hub to resolve RelativeUrlWithoutBase",
          "timestamp": "2026-04-09T13:52:43-04:00",
          "tree_id": "36f579694fc590a871b210996f5a31fc157c6aa3",
          "url": "https://github.com/bvolpato/ivygrep/commit/15b9b48e52cc6e4e737878703e9e841f5ee60136"
        },
        "date": 1775757434752,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 783342600,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8054.23,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3830.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2700.2,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11058.65,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10859.44,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.66,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 525.47,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14820.78,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10628.92,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5215.02,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451988.09,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 466.16,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7c0f46f630ffa58c38a8a3a859412e7c837ef129",
          "message": "perf(merkle): replace per-entry mutex with FlushGuard + benchmark warmup (#8)\n\nEliminate parallel walker lock contention by collecting entries into\nper-thread buffers that flush once on drop via FlushGuard, reducing\nMutex acquisitions from N to T (~4-8 threads).\n\nAdd explicit warm_up_time (3-5s) and measurement_time (10-15s) to all\nbenchmark groups to reduce cold-start noise in CI.",
          "timestamp": "2026-04-09T15:05:33-04:00",
          "tree_id": "a1f86d91e629d9f21b1d7a56d044f5ae3a526f7e",
          "url": "https://github.com/bvolpato/ivygrep/commit/7c0f46f630ffa58c38a8a3a859412e7c837ef129"
        },
        "date": 1775761966160,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 746122330,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7982.47,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3822.79,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2703.47,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11101.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10711.93,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.61,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 521.52,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14975.54,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10686.28,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5221.92,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449875.15,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 534.77,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "9f305bb51951afb9f5f0f4a6685ae2826c687959",
          "message": "fix(cli): handle SIGPIPE gracefully and silence candle_embed CUDA warnings",
          "timestamp": "2026-04-09T15:47:01-04:00",
          "tree_id": "cbd10934a123903a9f00287077b3f419f32be552",
          "url": "https://github.com/bvolpato/ivygrep/commit/9f305bb51951afb9f5f0f4a6685ae2826c687959"
        },
        "date": 1775764458303,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 700291540,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8195.3,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3724.95,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2636.86,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11511.14,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11286.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 513.07,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15160.71,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11186.3,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5490.06,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472337.69,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 554.43,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "36a2cdb1aeef5f56681f8c774d9c32da8c049f4c",
          "message": "chore: release v0.5.37",
          "timestamp": "2026-04-09T15:54:47-04:00",
          "tree_id": "faeff055eb8a2e16485901f5ee1953fa9700c662",
          "url": "https://github.com/bvolpato/ivygrep/commit/36a2cdb1aeef5f56681f8c774d9c32da8c049f4c"
        },
        "date": 1775764920302,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 742488730,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8385.86,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3825.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2698.44,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10962.78,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10518.85,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 517.31,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14911.19,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10571.82,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5195.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449374.16,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 561.57,
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
          "id": "9155ce832b90d82079a355097161a833e532ee1a",
          "message": "docs: polish README and add --scope user to gemini command",
          "timestamp": "2026-04-09T16:21:31-04:00",
          "tree_id": "aaa1bfc3084bfbd74f7df4677e622721cb615af8",
          "url": "https://github.com/bvolpato/ivygrep/commit/9155ce832b90d82079a355097161a833e532ee1a"
        },
        "date": 1775766539997,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 716418870,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8106.07,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3741.2,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2651.69,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11571.82,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11365.09,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.69,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 511.25,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15184.9,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11177.3,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5486.55,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473877.44,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 568.55,
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
          "id": "8b85c82fd7e771b6c583cd5c3608224cf162d2b0",
          "message": "feat: add ig_status tool to MCP server",
          "timestamp": "2026-04-09T16:28:27-04:00",
          "tree_id": "a9f5be333aee20a55470c52909428bebf8dc8186",
          "url": "https://github.com/bvolpato/ivygrep/commit/8b85c82fd7e771b6c583cd5c3608224cf162d2b0"
        },
        "date": 1775767104276,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 746070380,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8491.1,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3852.16,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2721.8,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11192.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10793.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 513.67,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14889.91,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10739.18,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5294.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449839.86,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 526.21,
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
          "id": "5167c87179f9ba2b1c73f8bb028131b32359ade4",
          "message": "chore: bump version to 0.5.38",
          "timestamp": "2026-04-09T16:31:34-04:00",
          "tree_id": "79f81c5f697d87484e02cca3924b8e1bbaf780ec",
          "url": "https://github.com/bvolpato/ivygrep/commit/5167c87179f9ba2b1c73f8bb028131b32359ade4"
        },
        "date": 1775767135489,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 769252830,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8211.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3820.2,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2691.01,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10740.1,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10599.39,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 514.11,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14638.73,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10639.85,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5191.39,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449062.82,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 530.95,
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
          "id": "257b751d98f08c38dc4b93ddaa4798a211bf69a1",
          "message": "chore: release 0.5.39 and update changelog",
          "timestamp": "2026-04-09T16:32:26-04:00",
          "tree_id": "e56a44847ea90c7152f1f3e9be0c43547d35a2f6",
          "url": "https://github.com/bvolpato/ivygrep/commit/257b751d98f08c38dc4b93ddaa4798a211bf69a1"
        },
        "date": 1775767194144,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 774886980,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8277.23,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3726.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2655.14,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11729.49,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11523.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 510.18,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15351.24,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11221.37,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5527.97,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 475128.69,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 581.86,
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
          "id": "c5ba5668c05125342c98a9bfadc6ddf6c44051fb",
          "message": "test: add end-to-end integration test for MCP interface",
          "timestamp": "2026-04-09T18:57:42-04:00",
          "tree_id": "f1e64a13242c582dc8184ea4ebe466252bacec6c",
          "url": "https://github.com/bvolpato/ivygrep/commit/c5ba5668c05125342c98a9bfadc6ddf6c44051fb"
        },
        "date": 1775775889364,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 759312060,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8070.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3821.75,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2705.99,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10997.1,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10652.88,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 517.87,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14755.29,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10605.33,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5172.08,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449178.83,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 577.09,
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
          "id": "0c17361a0b0111af7cac2aa69833f06c27c258f6",
          "message": "style: fix cargo fmt formatting in mcp tests",
          "timestamp": "2026-04-09T19:16:31-04:00",
          "tree_id": "6d69ec02d662ed19870fef7043a7a51287218b3c",
          "url": "https://github.com/bvolpato/ivygrep/commit/0c17361a0b0111af7cac2aa69833f06c27c258f6"
        },
        "date": 1775777011436,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 785645520,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8105.67,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3801.27,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2697.56,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10932.89,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10652.05,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 521.2,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14685.56,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10545.59,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5186.75,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449007.41,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 484.67,
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
          "id": "d37f4402b327f4aaacbffb5d3bc00621d5151da9",
          "message": "feat: hardware acceleration for local endpoints (AVX2 and Accelerate)",
          "timestamp": "2026-04-09T20:48:47-04:00",
          "tree_id": "c68f146a8478f9c333c715c4227bf9a194c98ccb",
          "url": "https://github.com/bvolpato/ivygrep/commit/d37f4402b327f4aaacbffb5d3bc00621d5151da9"
        },
        "date": 1775782567672,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 833473990,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 5115.19,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3644.57,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2555.38,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 6599.85,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5067.31,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.86,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 700.92,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 10467.71,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 6746.21,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 2714.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 476490.22,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 400.58,
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
          "id": "19de7e85539c56c29da86f17cea83d34ea818f40",
          "message": "chore: release v0.5.40",
          "timestamp": "2026-04-09T21:27:14-04:00",
          "tree_id": "4d78f1615ec679c14716d29fc39473a9db5369aa",
          "url": "https://github.com/bvolpato/ivygrep/commit/19de7e85539c56c29da86f17cea83d34ea818f40"
        },
        "date": 1775784870311,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 741279160,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8328.21,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3805.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2692.56,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10808.94,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10540.59,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.74,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 515.38,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14613.37,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10529.84,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5215.21,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449539.75,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 500.74,
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
          "id": "44b9ff9f4e53ce5024a1f252aab4a6e9e542d155",
          "message": "test: expand ci coverage to all modes and add e2e smoke tests",
          "timestamp": "2026-04-10T00:12:06-04:00",
          "tree_id": "fd2b969130de0313acc805126498fc8c640b153b",
          "url": "https://github.com/bvolpato/ivygrep/commit/44b9ff9f4e53ce5024a1f252aab4a6e9e542d155"
        },
        "date": 1775794761107,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 781571970,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8221.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3851.7,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2707.39,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11152.23,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10787.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 515.12,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15097.08,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10768.28,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5337.82,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450938.35,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 563.02,
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
          "id": "85c7aabf15f8fb3495d9ddcd779c35948785e992",
          "message": "fix(ci): resolve all CI failures — clippy lint, bench test-threads, smoke test flags",
          "timestamp": "2026-04-10T16:10:36-04:00",
          "tree_id": "0ae048c59b29b3b91899820cbcd92328c5b32b7e",
          "url": "https://github.com/bvolpato/ivygrep/commit/85c7aabf15f8fb3495d9ddcd779c35948785e992"
        },
        "date": 1775852275257,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 726575330,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8517.27,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3743.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2637.16,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11977.29,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11712.38,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 504.68,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15751.07,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11544.24,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5598.85,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472285.59,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 652.2,
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
          "id": "2714d846f2033b57f34ebe57775076febf4330e1",
          "message": "fix(ci): replace retired macos-13 with macos-15-intel runner",
          "timestamp": "2026-04-10T16:13:32-04:00",
          "tree_id": "701e78ac1697eed639380bd65ef0d6807de6ec19",
          "url": "https://github.com/bvolpato/ivygrep/commit/2714d846f2033b57f34ebe57775076febf4330e1"
        },
        "date": 1775852435584,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 778276420,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8312.65,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3971.56,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2775.06,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10742.04,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10651.82,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 516.72,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14807.86,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10571.9,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5191.06,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 448977.35,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 460.16,
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
          "id": "2908ca0a70d7c56daca91a52d6589b39bee192b3",
          "message": "feat: search relevance overhaul — definition ranking, query expansion, scoring rebalance\n\n- Rebalance RRF scoring: term_coverage 0.12→0.35, path_segment 0.08→0.20\n- Add definition_name_boost: prefer fn/class definitions over usage sites\n- Harden semantic-only penalty: 0.82→0.60, require both lexical+literal miss\n- Add zero-coverage noise filter for chunks with no query term overlap\n- Query expansion: generate snake_case and camelCase variants automatically\n- Density-aware literal scoring: count occurrences instead of flat 1.0\n- Add 5 targeted relevance integration tests\n- Create AGENTS_TESTING.md, AGENTS_DEPLOY.md, AGENTS_MONITOR.md",
          "timestamp": "2026-04-10T17:34:26-04:00",
          "tree_id": "beb8a13fcc9379f03cc463ff27aa8e43a0ba7029",
          "url": "https://github.com/bvolpato/ivygrep/commit/2908ca0a70d7c56daca91a52d6589b39bee192b3"
        },
        "date": 1775857320348,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 774152210,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8087.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3834.03,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2703.85,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11437.07,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10840.31,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.59,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 513.76,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15724.4,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10708.35,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5271.33,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450270.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 533.06,
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
          "id": "68234672a75cb86ea7b37ccc5a7b4449e03d6107",
          "message": "feat: search relevance overhaul — definition ranking, query expansion, scoring rebalance\n\n- Rebalance RRF scoring: term_coverage 0.12→0.35, path_segment 0.08→0.20\n- Add definition_name_boost: prefer fn/class definitions over usage sites\n- Harden semantic-only penalty: 0.82→0.60, require both lexical+literal miss\n- Add zero-coverage noise filter for chunks with no query term overlap\n- Query expansion: generate snake_case and camelCase variants automatically\n- Density-aware literal scoring: count occurrences instead of flat 1.0\n- Add 5 targeted relevance integration tests\n- Create AGENTS_TESTING.md, AGENTS_DEPLOY.md, AGENTS_MONITOR.md",
          "timestamp": "2026-04-10T17:38:35-04:00",
          "tree_id": "0bfd97130008194fa096635923140685402c09dc",
          "url": "https://github.com/bvolpato/ivygrep/commit/68234672a75cb86ea7b37ccc5a7b4449e03d6107"
        },
        "date": 1775857555556,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 742421230,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8301.26,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3849.53,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2699,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10954.68,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10650.72,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.59,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 516.7,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15413.81,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10795.8,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5238.7,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 454042.18,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 592.27,
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
          "id": "48822d0d3cf9591e50d9fe9d39189692bdc0c6ba",
          "message": "fix: literal search recall for top-level code\n\nTree-sitter chunker was silently dropping source lines not covered by\nfunction/class AST nodes (imports, constants, type aliases). This caused\nliteral and hybrid search to miss terms like 'gquota' that only appeared\nin top-level const declarations.\n\nChanges:\n- chunking: emit Module-kind gap chunks for uncovered source lines\n- search: clean up collect_literal_candidates (Tantivy index only, no\n  SQLite full-scan fallback needed now that all text is indexed)\n- tests: add CLI e2e + unit tests for the exact gquota regression\n\nCloses the literal search recall bug reported for v0.5.41.",
          "timestamp": "2026-04-11T00:57:54-04:00",
          "tree_id": "5f8357d8b64deeec1a32ad498f40bedec0ba99b5",
          "url": "https://github.com/bvolpato/ivygrep/commit/48822d0d3cf9591e50d9fe9d39189692bdc0c6ba"
        },
        "date": 1775883909055,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 925136550,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8541.14,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3823.12,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2753.21,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11531.23,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11338.21,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.94,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 531.25,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 21548.52,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14451.09,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5578.44,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473965.99,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 586.62,
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
          "id": "a3eff112ce7d35d4fc17d11c972bc503f2e1dde0",
          "message": "feat: BM25F code-aware tokenizer and multi-field relevance scoring\n\nImplement a custom Tantivy tokenizer that splits camelCase, snake_case,\ndots, colons, and path separators so BM25 natively matches natural-\nlanguage queries against code identifiers without post-hoc expansion.\n\nAdd two BM25F fields to the Tantivy schema:\n- file_path_text: tokenized path components (5× boost)\n- signature: first line of function/class definitions (5× boost)\n\nThis brings Sourcegraph/Zoekt-style field-level relevance: queries like\n'handle error' rank the handleError() definition above call sites, and\npath matches (e.g., 'auth' matching auth.rs) score 5× higher than body\ntext matches.\n\n- 7 new tokenizer unit tests (camelCase, snake_case, paths, etc.)\n- BM25F integration test proving definition-site ranking\n- All 132+ existing tests pass with no regressions",
          "timestamp": "2026-04-11T11:46:48-04:00",
          "tree_id": "6f4471748477d91c0e2377e68c39495690ad4ce7",
          "url": "https://github.com/bvolpato/ivygrep/commit/a3eff112ce7d35d4fc17d11c972bc503f2e1dde0"
        },
        "date": 1775922833069,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 951882000,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8206.1,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3818.56,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2809.6,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11327.69,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10928.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.59,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 522.71,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 25856.56,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15079.51,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5320.21,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450887.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 504.47,
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
          "id": "c80f18416951f0005057c5985746ea4839602020",
          "message": "feat: BM25F code-aware tokenizer and multi-field relevance scoring\n\nImplement a custom Tantivy tokenizer that splits camelCase, snake_case,\ndots, colons, and path separators so BM25 natively matches natural-\nlanguage queries against code identifiers without post-hoc expansion.\n\nAdd two BM25F fields to the Tantivy schema:\n- file_path_text: tokenized path components (5× boost)\n- signature: first line of function/class definitions (5× boost)\n\nThis brings Sourcegraph/Zoekt-style field-level relevance: queries like\n'handle error' rank the handleError() definition above call sites, and\npath matches (e.g., 'auth' matching auth.rs) score 5× higher than body\ntext matches.\n\n- 7 new tokenizer unit tests (camelCase, snake_case, paths, etc.)\n- BM25F integration test proving definition-site ranking\n- All 132+ existing tests pass with no regressions",
          "timestamp": "2026-04-11T11:52:39-04:00",
          "tree_id": "6906b49c465265aa9af06671a6d1d56b90debb97",
          "url": "https://github.com/bvolpato/ivygrep/commit/c80f18416951f0005057c5985746ea4839602020"
        },
        "date": 1775923188069,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 949140800,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8077.29,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3825.7,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2828.54,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10975.97,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10616.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.67,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 516.11,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 25266.09,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15005.71,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5262.68,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450366.44,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 500.73,
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
          "id": "464de7a6bec8556b5d37d3b6b96e7dd51469d38c",
          "message": "Improve relevance and add index doctor",
          "timestamp": "2026-04-11T17:51:15-04:00",
          "tree_id": "3ec38f985ec85efb863a1d57f020cc62e64b9b36",
          "url": "https://github.com/bvolpato/ivygrep/commit/464de7a6bec8556b5d37d3b6b96e7dd51469d38c"
        },
        "date": 1775946223661,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 945039230,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 9720.28,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3742.91,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2758.38,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11615.69,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11466.53,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 514.9,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 57780.18,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15352.91,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5602.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471349.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 564.21,
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
          "id": "63db75990ccd1e0585e7e6e54bdfb44c54b75638",
          "message": "Stabilize background jobs and expand parser support",
          "timestamp": "2026-04-12T10:24:05-04:00",
          "tree_id": "8a0433306bff4387cffda06e2f7d79968f2971c2",
          "url": "https://github.com/bvolpato/ivygrep/commit/63db75990ccd1e0585e7e6e54bdfb44c54b75638"
        },
        "date": 1776004302905,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 952954780,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 22391.77,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3875.15,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2835.6,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11578.59,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11141.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.04,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 550.86,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 57676.09,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15132.55,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5355.33,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 452955.7,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 552.52,
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
          "id": "432c695d52470459107f4b15b7c96780494b3a86",
          "message": "Serialize env-sensitive unit tests",
          "timestamp": "2026-04-12T10:36:17-04:00",
          "tree_id": "b2ad5fd496a328e6ccd1bcf26c0e71038d9730c6",
          "url": "https://github.com/bvolpato/ivygrep/commit/432c695d52470459107f4b15b7c96780494b3a86"
        },
        "date": 1776005012550,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 956984960,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 17963.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3848.54,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2851.75,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11427.96,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11025.22,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 518.92,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 59367.49,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15317.13,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5404.12,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 453343.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 546.4,
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
          "id": "7c3c9856c94b3d0c6dc18346609eae4177e7d883",
          "message": "Protect no-op reindex performance",
          "timestamp": "2026-04-12T14:05:55-04:00",
          "tree_id": "de72b2f8cecc3a23fad907fcd1fdf5f859912d48",
          "url": "https://github.com/bvolpato/ivygrep/commit/7c3c9856c94b3d0c6dc18346609eae4177e7d883"
        },
        "date": 1776017688291,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 1017110020,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4543.35,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3670.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2673.72,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 6895.93,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5503.92,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 710.5,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 43441.47,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10094.86,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 2773.19,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 474871.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 408.56,
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
          "id": "72a1ae04b1d8f8b34ce189bd3027a301bca3347e",
          "message": "Prepare v0.5.47 release",
          "timestamp": "2026-04-12T19:07:10-04:00",
          "tree_id": "24da96946bb0b34cb2209a28f9caf881a3f8ff2d",
          "url": "https://github.com/bvolpato/ivygrep/commit/72a1ae04b1d8f8b34ce189bd3027a301bca3347e"
        },
        "date": 1776035790802,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 985637630,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 5298.19,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3666.51,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2684.35,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 4779.41,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4843.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 709.27,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 43195.6,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 9972.49,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 2769.53,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471561.9,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 409.81,
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
          "id": "47d85d230a4820812372656accda7fe28198d8d0",
          "message": "Prepare v0.5.48 release",
          "timestamp": "2026-04-12T21:20:31-04:00",
          "tree_id": "af79ce7a7ca47905374edfd4a807faf64e27bec7",
          "url": "https://github.com/bvolpato/ivygrep/commit/47d85d230a4820812372656accda7fe28198d8d0"
        },
        "date": 1776043778360,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 1004262040,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4562.71,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3673.61,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2676.72,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 4813.26,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4710.04,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.86,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 695.67,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 43017.45,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10001.62,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 2769.81,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473539.2,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 411.53,
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
          "id": "19b8c433eac92776e10c08f2e6414a5a560df3d1",
          "message": "Prepare v0.5.49 release",
          "timestamp": "2026-04-12T21:36:26-04:00",
          "tree_id": "1b4e9294d67e79fa479d1a53372b9dd063dc4590",
          "url": "https://github.com/bvolpato/ivygrep/commit/19b8c433eac92776e10c08f2e6414a5a560df3d1"
        },
        "date": 1776044706834,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 850993600,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7941.87,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3803.12,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2811.76,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11359.36,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10810.34,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 518.96,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56819.87,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15037,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5356.54,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450423.89,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 598,
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
          "id": "b69c04cf9939309b4267272da2b453382072a05a",
          "message": "fix: prevent silent data loss on crash and improve test-path precision\n\nCritical fix: Merkle snapshot was saved BEFORE index stores were committed.\nA crash (SIGKILL/OOM/power loss) between the snapshot save and the final\ntx.commit()/writer.commit()/vector_index.save() left the snapshot claiming\nfiles were indexed while stores were empty/partial. On next run, the diff\nwas empty and missing files were silently never re-indexed.\n\nFix 1 (indexer.rs): Defer snapshot save to after all store commits and\nwrite_metadata(). The snapshot is now a high-water mark of actually-persisted\nstate. Crash mid-indexing → stale snapshot → non-empty diff → re-index.\n\nFix 2 (workspace.rs): Detect crashed indexing in index_health_with_options.\nIf .indexing.pid exists but the PID is dead (SIGKILL bypasses Drop), mark\nindex as Unhealthy → forces rebuild_index_storage on next run.\n\nFix 3 (merkle.rs): Atomic snapshot write via write-to-tmp + fs::rename().\nCrash during save can no longer leave truncated JSON.\n\nFix 4 (search.rs): is_test_path() used bare .contains(\"test\") which\npenalized files like attestation.rs, contest.rs, inspect.py as test files.\nReplaced with boundary-aware matching: directory segments (tests/, __tests__/)\nand filename conventions (_test., .test., test_).\n\nAdded 2 test functions with 27 assertions for is_test_path coverage.",
          "timestamp": "2026-04-13T15:18:26-04:00",
          "tree_id": "4945e0ee1df6b861c28c9896651b739032ad4144",
          "url": "https://github.com/bvolpato/ivygrep/commit/b69c04cf9939309b4267272da2b453382072a05a"
        },
        "date": 1776108448242,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 899399960,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8263.71,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3813.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2844.14,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11319.33,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10799.3,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.75,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 519.81,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 58373.5,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15152.89,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5365.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449763.72,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 618.18,
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
          "id": "b9dfa6a9922b765246dd87fdc3edd8b83253ff31",
          "message": "release: v0.5.50",
          "timestamp": "2026-04-13T15:19:32-04:00",
          "tree_id": "be35bbd00523cd4b86a9bcd341d271a32bdb6bf8",
          "url": "https://github.com/bvolpato/ivygrep/commit/b9dfa6a9922b765246dd87fdc3edd8b83253ff31"
        },
        "date": 1776108486483,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 839738950,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8008.91,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3797.2,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2814.09,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10945.71,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10667.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 516.36,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 57823.97,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14925.15,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5275.82,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449099.45,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 456.17,
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
          "id": "d3959466ab077876e4786052e1ace6510abcb68e",
          "message": "Fix worktree overlay staleness bug by tracking base index generation",
          "timestamp": "2026-04-13T16:03:37-04:00",
          "tree_id": "c05806e4ab7bf043ae273f6dd476dff0d4d64f83",
          "url": "https://github.com/bvolpato/ivygrep/commit/d3959466ab077876e4786052e1ace6510abcb68e"
        },
        "date": 1776111156477,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 925486450,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8114.55,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3798.99,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2817.57,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10877.67,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10586.61,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 548.11,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 57001.69,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14847.9,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5377.51,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451363.88,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 608.6,
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
          "id": "0e078f3f8d0fcd81c97d2d4afe0ac80965b53f17",
          "message": "chore: bump version to 0.5.51",
          "timestamp": "2026-04-13T16:04:38-04:00",
          "tree_id": "3b3bb16b816c4d89a434c8d27e2a3664dffc9bee",
          "url": "https://github.com/bvolpato/ivygrep/commit/0e078f3f8d0fcd81c97d2d4afe0ac80965b53f17"
        },
        "date": 1776111193822,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 821975810,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7820.76,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3705.55,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2759.58,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11554.28,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11268.38,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 522.83,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56885.18,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15214.04,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5600.81,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471373.3,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 565.64,
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
          "id": "c6967c0fb66f240b603690b13fa39d848b2eb6c0",
          "message": "test: add cli e2e test for worktree overlay auto-reindex",
          "timestamp": "2026-04-13T16:51:57-04:00",
          "tree_id": "2260b6b24f061122f07070bef6b60473fa5ddb84",
          "url": "https://github.com/bvolpato/ivygrep/commit/c6967c0fb66f240b603690b13fa39d848b2eb6c0"
        },
        "date": 1776113995881,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 840696180,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8074.61,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3714.26,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2782.37,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11503.24,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11346.75,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.6,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 509.11,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56942.56,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15099.58,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5613.94,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472947.17,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 567.71,
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
          "id": "36eec9756dc3827f9864dac7ec562ccf3857788f",
          "message": "chore: bump version to 0.5.52",
          "timestamp": "2026-04-13T17:25:39-04:00",
          "tree_id": "3513f2a2d5f7498b571e5b27e03ca0156fd73fcc",
          "url": "https://github.com/bvolpato/ivygrep/commit/36eec9756dc3827f9864dac7ec562ccf3857788f"
        },
        "date": 1776116069216,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 841750840,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7946.84,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3716.82,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2773.99,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11637.76,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11382.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.61,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 514.7,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56526.34,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15233.81,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5600.26,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 474409.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 590.55,
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
          "id": "d5815458e087e6d51e9d5512652362dc52cb53d1",
          "message": "fix: eliminate TOCTOU race in register_watcher\n\nHold the watchers mutex across check+build+insert so two concurrent\nrequests can no longer both pass contains_key and create duplicate\nwatchers.  The second insert previously overwrote the first, dropping\nthe WatchRegistration (stopping its notify fd) while leaving the\nspawned tokio task parked forever on a dead Notify — a silent fd/task\nleak.",
          "timestamp": "2026-04-13T20:42:36-04:00",
          "tree_id": "edce2730d0ff10147ac2941d74443cfa89985f96",
          "url": "https://github.com/bvolpato/ivygrep/commit/d5815458e087e6d51e9d5512652362dc52cb53d1"
        },
        "date": 1776127902683,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 951617050,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7805.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3716.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2742.76,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11551.51,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11351.34,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 514.06,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56717.9,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15348.86,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5613.95,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471700.98,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 566.96,
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
          "id": "f5c7e0f0a35a43013676c341466955ba5fa5b6f5",
          "message": "chore: bump version to 0.5.53",
          "timestamp": "2026-04-13T21:16:43-04:00",
          "tree_id": "09b0f06b9ab956cefcd08aff3181dba08400d465",
          "url": "https://github.com/bvolpato/ivygrep/commit/f5c7e0f0a35a43013676c341466955ba5fa5b6f5"
        },
        "date": 1776129945133,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 986428210,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6696.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2881.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2127.88,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9589.35,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9239.98,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 5.6,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 418.77,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 44235.7,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 12123.74,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 4489.85,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 375574.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 484.42,
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
          "id": "8f03431f8fd4ea872d8320004578591829328c65",
          "message": "fix: TOCTOU race in register_watcher + regression test\n\nHold the watchers mutex across check+build+insert to prevent concurrent\nrequests from creating duplicate watchers.  Adds a concurrent registration\nregression test that fires 8 tokio tasks at register_watcher simultaneously\nand asserts exactly one watcher survives.\n\nBump version to 0.5.54.",
          "timestamp": "2026-04-13T22:13:00-04:00",
          "tree_id": "d20cce58d82cd07ebd19c2f4b0a99ccd4f82181f",
          "url": "https://github.com/bvolpato/ivygrep/commit/8f03431f8fd4ea872d8320004578591829328c65"
        },
        "date": 1776133334864,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 950893930,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7977.18,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3909.02,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2867.22,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 12006.52,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11580.98,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.89,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.52,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 58959.52,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15676.22,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5452.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451752.94,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 544.8,
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
          "id": "8bb8c1bf683201e4e77da8e1a76cf72c1c9c86f6",
          "message": "fix: prevent flock race in pre-lock rebuild_index_storage\n\nMove the needs_rebuild() health check and rebuild_index_storage() call\ninside the flock critical section. Previously, rebuild ran before the\nlock was acquired, calling remove_dir_all on index_dir which destroyed\nthe index.lock inode. Any concurrent process that already held the\nflock on the old inode lost mutual exclusion — both believed they had\nexclusive access, causing silent index corruption.\n\nChanges:\n- Add remove_workspace_index_contents() that removes everything in\n  index_dir except index.lock, preserving the flock inode\n- Move health check + rebuild after lock_exclusive()\n- rebuild_index_storage now uses lock-preserving removal so the\n  secondary call (storage verification fallback) also can't break\n  a held lock\n\nBump: 0.5.54 → 0.5.55",
          "timestamp": "2026-04-14T01:25:27-04:00",
          "tree_id": "897d2230630d50091a076dfe21962c629e3d94c2",
          "url": "https://github.com/bvolpato/ivygrep/commit/8bb8c1bf683201e4e77da8e1a76cf72c1c9c86f6"
        },
        "date": 1776144873760,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 913656800,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8971.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3779.17,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2815.51,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10952.58,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10715.9,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 517.56,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 57829.32,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14859.39,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5237.65,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449223.26,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 534.76,
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
          "id": "ab85f8e0284ecef1dc1647de535a50b218bae6ff",
          "message": "fix: overlay merkle snapshot mismatch + VectorStore F32 silent data loss\n\n1. Overlay merkle snapshot format mismatch (perf bug):\n   Overlay creation saved a content-based snapshot (xxh3 of path+bytes)\n   but subsequent watcher runs called MerkleSnapshot::build (mtime-based).\n   Every file's hash differed, triggering a full re-index on every watcher\n   event. Fix: save mtime-based snapshot after overlay creation so\n   incremental diffs use the same hash format.\n\n2. VectorStore::open() F32 silent empty fallback (data loss):\n   When quantization was F32, a failed load() fell through to returning\n   an empty index (the !F32 guard skipped the fallback branch). Next\n   save() overwrites the corrupt file, wiping all neural vectors.\n   Fix: propagate the error for F32 loads instead of silently returning\n   empty. Applied to both open() and open_readonly().\n\nBump: 0.5.55 → 0.5.56",
          "timestamp": "2026-04-14T09:22:57-04:00",
          "tree_id": "209dc037707d6e3ba8e278779c713f3894980c89",
          "url": "https://github.com/bvolpato/ivygrep/commit/ab85f8e0284ecef1dc1647de535a50b218bae6ff"
        },
        "date": 1776173534801,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 953711840,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7942.79,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3841.88,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2837.98,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11031.26,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10925.22,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.77,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 518.54,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56834.37,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14852.07,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5355.79,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450368.66,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 592.7,
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
          "id": "b813f81fda6f0d729c505c7292ada952d2611848",
          "message": "test: improve E2E MCP tests and daemon stale socket recovery\\n\\n- Expanded mcp_e2e to test tools/list and tools/call\\n- Verified graceful recovery when encountering a stale IPC socket file\\n- Normalized actions/checkout minor version skew in CI",
          "timestamp": "2026-04-20T15:53:06-04:00",
          "tree_id": "80cf9c7752c88790e76b78e10fb8fb516531f455",
          "url": "https://github.com/bvolpato/ivygrep/commit/b813f81fda6f0d729c505c7292ada952d2611848"
        },
        "date": 1776715706804,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 859445920,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7639.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3794.31,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2813.36,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11053.25,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10747.74,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 523.08,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56466.45,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14767.34,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5276.64,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 452218.9,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 507.48,
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
          "id": "dd452b49a03c3e4074dcd335f18cffb89f273d7e",
          "message": "chore(release): bump version to 0.6.1",
          "timestamp": "2026-04-20T15:55:31-04:00",
          "tree_id": "9c2ada60fce882f10baee970c790521c78ce30fe",
          "url": "https://github.com/bvolpato/ivygrep/commit/dd452b49a03c3e4074dcd335f18cffb89f273d7e"
        },
        "date": 1776715930060,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 835672680,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7907.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3714.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2722.9,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11566.23,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11334.88,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.55,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 504.11,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 56633.47,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14975.92,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5607.55,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472611.23,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 568.58,
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
          "id": "e51dc00ce314fcc0a6f09dde9399dfbc7eb6d7bc",
          "message": "chore(release): bump version to 0.6.2",
          "timestamp": "2026-04-20T16:07:32-04:00",
          "tree_id": "243c2a0e519a1435d56ad1847fb7d184af7f11ad",
          "url": "https://github.com/bvolpato/ivygrep/commit/e51dc00ce314fcc0a6f09dde9399dfbc7eb6d7bc"
        },
        "date": 1776716651841,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 839980580,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7942.05,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3846.34,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2830.98,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11344.8,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10988.11,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 559.69,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 57038.07,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15001.06,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5373.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451303.25,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 609.57,
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
          "id": "7a20efb5d37b446c8503d8420b83884a92ee07be",
          "message": "tui: harden interactive search UX",
          "timestamp": "2026-04-23T19:53:24-04:00",
          "tree_id": "27d1e4a58c97e8d8557f950c5fc2b80e41c8954b",
          "url": "https://github.com/bvolpato/ivygrep/commit/7a20efb5d37b446c8503d8420b83884a92ee07be"
        },
        "date": 1776988962857,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 968427290,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 9454.4,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3804.06,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2811.19,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11216.69,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10869.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.02,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 529.03,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 71157.17,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14859.35,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5367.63,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 452584.12,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 603.98,
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
          "id": "5b950c1dcd207f9b0400211e8a1b300909220a3f",
          "message": "test: relax flaky stress timing guard",
          "timestamp": "2026-04-23T20:00:56-04:00",
          "tree_id": "ace8904be2d2fc9a5571a1952ec6a34ad88a3f19",
          "url": "https://github.com/bvolpato/ivygrep/commit/5b950c1dcd207f9b0400211e8a1b300909220a3f"
        },
        "date": 1776989350522,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 840441250,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7641.64,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3793.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2803.46,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10960.43,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10683.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.6,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 516.74,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 71481.13,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14763.82,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5215.27,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449761,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 575.93,
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
          "id": "f504bfb19ca23306070e242e05371c00f002bcb9",
          "message": "cli: remove interactive short alias",
          "timestamp": "2026-04-23T20:48:51-04:00",
          "tree_id": "017654796c4812cf07943feedc18f48443f70ef4",
          "url": "https://github.com/bvolpato/ivygrep/commit/f504bfb19ca23306070e242e05371c00f002bcb9"
        },
        "date": 1776992288410,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 931048630,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7866.67,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3692.36,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2756.15,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11722.44,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11464.5,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 506.89,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 69645.12,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15112.68,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5647.16,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472559.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 633.73,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "5e3f5c6bf4153cf2a7318152f1eed2f8a473615c",
          "message": "tui: redesign interactive mode with hierarchical code browser\n\n- Four-mode navigation: Search → FileList → SnippetList → FileView\n- Deduplicated file list with hit counts in left panel\n- Snippet browsing with syntax-highlighted previews\n- Full file expansion with line numbers and gutter highlighting\n- Editor integration via 'e' key, clipboard copy via 'y'\n- Esc/Ctrl+C clear-then-quit behavior in search box\n- Status bar with mode-dependent key hints\n- Visual polish: divider lines, higher-contrast colors\n- Update README: remove TUI from roadmap, add --interactive to CLI ref",
          "timestamp": "2026-04-24T23:20:48-04:00",
          "tree_id": "48155f4323bafb1c7fb0ff90c14bec836856ae0b",
          "url": "https://github.com/bvolpato/ivygrep/commit/5e3f5c6bf4153cf2a7318152f1eed2f8a473615c"
        },
        "date": 1777087804799,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 956377840,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7937.22,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3716.57,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2765.32,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11495.3,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11336.54,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 518.19,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 71595.92,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14950.68,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5585.24,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472296.61,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 570.7,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "ee821ec498c6f25bb4e604666d8623d0e92ef8fd",
          "message": "style: fix cargo fmt",
          "timestamp": "2026-04-25T00:10:34-04:00",
          "tree_id": "7c5e79f2cc2984213797a086e1facec785c6a8af",
          "url": "https://github.com/bvolpato/ivygrep/commit/ee821ec498c6f25bb4e604666d8623d0e92ef8fd"
        },
        "date": 1777090802190,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 961931420,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7858.11,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3831.4,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2802.85,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10984.21,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10665.36,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.67,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 527.47,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 70549.83,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14748.59,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5279.69,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449742.87,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 530.09,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "20edf25427c8a3f6bbca28177f273f3ab5b35781",
          "message": "tui: show aggregated score in file list",
          "timestamp": "2026-04-25T01:03:56-04:00",
          "tree_id": "6656d20ff0851d9f366b863e740465539fa0899f",
          "url": "https://github.com/bvolpato/ivygrep/commit/20edf25427c8a3f6bbca28177f273f3ab5b35781"
        },
        "date": 1777094006740,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 975345780,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 10842.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3858.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2848.66,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11576.79,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11330.8,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.66,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 520.35,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 73512.39,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15413.66,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5388.42,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451089.19,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 551.88,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "fa0e388fefefe7b07eb05e37bd2893e72eeaef0a",
          "message": "tui: cache highlighted lines in FileView to prevent large-file render lag",
          "timestamp": "2026-04-25T01:31:51-04:00",
          "tree_id": "f7dc66a095d0cef26e58d06f5d7f14b5c44f4d94",
          "url": "https://github.com/bvolpato/ivygrep/commit/fa0e388fefefe7b07eb05e37bd2893e72eeaef0a"
        },
        "date": 1777095660132,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 923179020,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7785.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3808.22,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2812.98,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11426.63,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11003.6,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.18,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 559.16,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 72700.42,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14902.52,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5285.99,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449978.21,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 538.3,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "124ee2a15f120af7fa65e84f3df844ce8e292295",
          "message": "tui: fix clippy lints and add comprehensive unit tests\n\n- Fix type_complexity: add FileViewCache type alias\n- Fix collapsible_match: fold if-guards into match arms\n- Add 27 new unit tests covering:\n  - File navigation wrapping (next/prev, empty list)\n  - Snippet navigation wrapping (next/prev, empty)\n  - Mode transitions and equality\n  - current_snippets / selected_snippet behavior\n  - Flash message lifecycle\n  - reset_results state clearing\n  - Snippet rendering (dividers, scores, empty, selection)\n  - File view rendering (with/without highlight)\n  - Path resolution (relative/absolute)\n  - Hint line generation\n  - FileViewCache type alias\n  - group_hits_by_file dedup and score aggregation\n\nTotal test count: 200 (up from 173)",
          "timestamp": "2026-04-25T14:37:32-04:00",
          "tree_id": "8bf0049b2249e73f8be93818c37a16dd35d5f828",
          "url": "https://github.com/bvolpato/ivygrep/commit/124ee2a15f120af7fa65e84f3df844ce8e292295"
        },
        "date": 1777142814468,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 959273630,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 9223.19,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3816.53,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2793.47,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11032.09,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10746.86,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 533.81,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 71362.04,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14730.88,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5277.97,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450323.98,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 550.6,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "52797e990dcb8cfd487900f8299ce077efd5fb38",
          "message": "release: v0.6.5",
          "timestamp": "2026-04-25T14:38:20-04:00",
          "tree_id": "5d95ab59bb242e8904240cbd632e34585a3e3a1a",
          "url": "https://github.com/bvolpato/ivygrep/commit/52797e990dcb8cfd487900f8299ce077efd5fb38"
        },
        "date": 1777142814647,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 849776530,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7634.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3717.45,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2738.74,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11536.19,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11295.14,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 516.66,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 67750.15,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14782.75,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5404.05,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472525.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 581.34,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "894c27cf857bd86636dd18a66d198a3ed0f33ac0",
          "message": "tui: add mouse support, draggable separator, and Tab/Shift+Tab cycling\n\n- Mouse click: focus search input, file list, or snippet panel\n- Mouse scroll: navigate lists or scroll file view (3 lines per tick)\n- Draggable separator: click+drag the border to resize panels (15-70%)\n- Tab cycles forward: Search → FileList → SnippetList → Search\n- Shift+Tab cycles backward\n- Status bar hints updated with Tab and mouse indicators\n- 11 new unit tests (211 total)",
          "timestamp": "2026-04-25T15:22:15-04:00",
          "tree_id": "76b5b9388c90bc2af69698f0b80052e98ccc3d1a",
          "url": "https://github.com/bvolpato/ivygrep/commit/894c27cf857bd86636dd18a66d198a3ed0f33ac0"
        },
        "date": 1777145484779,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 934442780,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7982.87,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3826.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2815.51,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11121.09,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10799.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 522.87,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 71662.93,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14974.8,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5364.67,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450824.06,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 535.89,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "b6b00ddc85989931284647dae84f01e9065c8ec2",
          "message": "fix: collapse Tab guard to satisfy clippy collapsible_match",
          "timestamp": "2026-04-25T15:32:53-04:00",
          "tree_id": "97ec97bdc17dd20e24c84dc0b88cb662891af9e0",
          "url": "https://github.com/bvolpato/ivygrep/commit/b6b00ddc85989931284647dae84f01e9065c8ec2"
        },
        "date": 1777146132541,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 936332580,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7646.53,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3790.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2780.12,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11074.21,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10735.5,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.64,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 512.69,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 72048.75,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14872.17,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5288.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449302.95,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 501.74,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "4e7f374265ef4ee3c4c3d5b871339019f27d28bf",
          "message": "tui: never block on indexing — launch TUI immediately\n\nprepare_workspace_for_tui is now fully non-blocking. If the workspace\nis not yet indexed, indexing is kicked off in a background thread (via\ndaemon or locally) and the TUI launches immediately with a status\nmessage. The search pipeline already falls back gracefully when the\nindex is unavailable, so users can search with whatever is available\nwhile indexing completes in the background.",
          "timestamp": "2026-04-25T15:46:05-04:00",
          "tree_id": "ecbc75a55cb7e2ed8147e2265bc3bfce90212647",
          "url": "https://github.com/bvolpato/ivygrep/commit/4e7f374265ef4ee3c4c3d5b871339019f27d28bf"
        },
        "date": 1777146921697,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 1033852550,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4591.11,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3704.2,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2678.12,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 5050.3,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5019.17,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.98,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 676.84,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 54517.35,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10008.87,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 2815.2,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471917.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 423.64,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "e12452d828d102251e8b668d2e126e03adae3314",
          "message": "tui: fix pre-filled query hanging the UI before initial draw",
          "timestamp": "2026-04-25T16:07:11-04:00",
          "tree_id": "b2a557b1e3c8c88b7fd2c3115a253ea0ed150caa",
          "url": "https://github.com/bvolpato/ivygrep/commit/e12452d828d102251e8b668d2e126e03adae3314"
        },
        "date": 1777148208956,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 967751070,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7976.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3842.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2823.54,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11379.33,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10879.98,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.72,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 529.52,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 72408.15,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 15149.38,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5364.24,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450931.01,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 556.14,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "3a08f679547f38709c1009c8047fa5a9b55c42cc",
          "message": "bump version to v0.6.8",
          "timestamp": "2026-04-25T16:52:57-04:00",
          "tree_id": "a0da3fbb97f8a2e548be334b9d9c09e72d5b9de8",
          "url": "https://github.com/bvolpato/ivygrep/commit/3a08f679547f38709c1009c8047fa5a9b55c42cc"
        },
        "date": 1777150902922,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 855124190,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7776.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3845.1,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2905.28,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11301.84,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10809.73,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.88,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 531.82,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 71250.04,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 14760.56,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5266.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450406.29,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 529.84,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "e48ce5df29cf76e24a51863838de76078282db88",
          "message": "tui: display live background indexing progress and fix phantom snippets",
          "timestamp": "2026-04-25T16:52:27-04:00",
          "tree_id": "61ae9c0ffb770b6ead2deaecca71c9269556b506",
          "url": "https://github.com/bvolpato/ivygrep/commit/e48ce5df29cf76e24a51863838de76078282db88"
        },
        "date": 1777150929525,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 912965210,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 6623.28,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2895.64,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2126.99,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9405.68,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9206.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 5.13,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 403.56,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 54166.79,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 11862.09,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 4448.83,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 375003.03,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 455.38,
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
          "id": "59ecff171ad32a07b7a52fe5c8e4d3e9b6192fab",
          "message": "perf: 17x faster search on large repos (20s → 1.2s on dd-source)\n\nRoot cause: candidate limits were set to 1,000,000 by default for\nnon-skip-gitignore searches, causing:\n- Tantivy BM25: scoring/sorting millions of docs per query variant\n- USearch ANN: k=1M vector search (15s+ on 3.8M vectors)\n- SQLite: thousands of individual text lookups (text not in Tantivy)\n\nFixes:\n- Tantivy candidate_limit: 1M → 5K (10x output limit, capped)\n- Literal pass limit: 5K → 500 (text requires SQLite fetch per hit)\n- Semantic/vector limit: 1M → 200 (ANN with k=200 is ~30ms vs 15s)\n- Truncate BM25 results before SQLite text-population phase\n- Early termination in literal/variant loops when enough hits found\n\nBenchmarks on dd-source (289K files, 3.8M chunks, 7GB index):\n  'ddsqlizer client': 20s → 1.2s\n  'error handling':   → 0.5s\n  'kafka producer':   → 0.5s",
          "timestamp": "2026-04-26T02:13:52-04:00",
          "tree_id": "025c373b1f6be961a2825e4f4233d66cdfe595d3",
          "url": "https://github.com/bvolpato/ivygrep/commit/59ecff171ad32a07b7a52fe5c8e4d3e9b6192fab"
        },
        "date": 1777184594835,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 947018350,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7910.27,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3697.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2761.83,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11585.18,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11330.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.91,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 507.63,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 37356.78,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8913.36,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5598.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 474571.79,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 594.75,
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
          "id": "8232e8ef1693491457605224d30f7239346527f9",
          "message": "perf: candidate limits scale proportionally with --limit\n\nDefault (no --limit) remains fast (~1s on dd-source/3.8M chunks).\nWith --limit N, candidates grow proportionally:\n\n  Lexical (Tantivy BM25): 10×N, capped at 50K\n  Literal (substring):     5×N, capped at 25K\n  Semantic (vector ANN):   1×N, capped at 2K\n\nBenchmarks on dd-source (289K files, 3.8M chunks):\n  default:    1.1s\n  --limit 10: 183ms\n  --limit 100: 608ms\n  --limit 500: 2.7s",
          "timestamp": "2026-04-26T11:40:23-04:00",
          "tree_id": "1af80a8fcb96ceb998ec557f8d69e8d52792a770",
          "url": "https://github.com/bvolpato/ivygrep/commit/8232e8ef1693491457605224d30f7239346527f9"
        },
        "date": 1777218581101,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 960671880,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8606.15,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3719.46,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2757.43,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11692.75,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11364.87,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.09,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 539.24,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29937.75,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 9021.78,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5603.3,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471481.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 579.38,
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
          "id": "177d2344dff261c45fe40534c5c2710fe6e14e34",
          "message": "release: v0.6.10 — 17× faster search on large repos",
          "timestamp": "2026-04-26T19:51:02-04:00",
          "tree_id": "b5714ba5eb483a7bd0464337ae31c569e2fbca62",
          "url": "https://github.com/bvolpato/ivygrep/commit/177d2344dff261c45fe40534c5c2710fe6e14e34"
        },
        "date": 1777248684974,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 846578600,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7653.28,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3801.1,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2836.13,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11121.28,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10803.74,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 540.32,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29402.77,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8574.66,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5330.38,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450220.41,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 557.48,
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
          "id": "4d78d211bbafe623915e81a52a265e1a401d4913",
          "message": "chore: remove internal repo references from public code",
          "timestamp": "2026-04-26T19:59:37-04:00",
          "tree_id": "ff088a74ca43fd0122a6133ff5ed5b604014741f",
          "url": "https://github.com/bvolpato/ivygrep/commit/4d78d211bbafe623915e81a52a265e1a401d4913"
        },
        "date": 1777250286471,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 1022216260,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4738.86,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3678.1,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2685.94,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 6830.4,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5416.07,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 707.99,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 22476.51,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 5363.02,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 2794.94,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473565.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 423.18,
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
          "id": "1c54997e194b44a215a4d82302ab9a059bc97830",
          "message": "perf: remove unnecessary 10ms sleep in neural enhancer\n\ncheck_system_constraints() already throttles on battery, thermal\nlimits, and high load. The extra sleep was pure throughput loss.",
          "timestamp": "2026-04-26T20:56:23-04:00",
          "tree_id": "baded5c7daf9e3ccfee3e6ddf012fe0e6e97d8e5",
          "url": "https://github.com/bvolpato/ivygrep/commit/1c54997e194b44a215a4d82302ab9a059bc97830"
        },
        "date": 1777251952283,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 947179600,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7680.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3810.57,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2821.5,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11044.54,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10669.94,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.77,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 512.06,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29307.56,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8496.44,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5259.12,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450106.32,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 537.73,
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
          "id": "67eb88f99aea6fb90b6b126bc3f8e71f4faafd00",
          "message": "feat: boost files whose path matches the query\n\nAdd a path-match pass that queries the file_path_text index for\nchunks whose directory or file name contains all query tokens.\nResults are injected into the lexical candidate set with high\npriority so they dominate after RRF fusion.\n\nAlso add path_exact_match_boost (weight 3.0) in the scoring\nfunction: when the full query appears verbatim as a path segment,\nthat chunk gets a massive score boost.\n\nCombined effect: searching for a hyphenated service name like\n\"my-service\" now correctly ranks files under apps/my-service/\nat the very top instead of burying them under random code matches\nfor individual tokens like \"service\" or \"controller\".\n\nAlso increased PATH_SEGMENT_WEIGHT (0.20 → 0.40) and\nFILE_STEM_WEIGHT (0.30 → 0.50) for stronger general path ranking.",
          "timestamp": "2026-04-26T21:18:24-04:00",
          "tree_id": "6f98a3db58e3d81dc47396fcaade27577b8f316d",
          "url": "https://github.com/bvolpato/ivygrep/commit/67eb88f99aea6fb90b6b126bc3f8e71f4faafd00"
        },
        "date": 1777253577910,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 930833280,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7797.75,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3863.01,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2760.01,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11539.9,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11356.45,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 547.33,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29928.43,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8888.65,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5582.92,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471835.08,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 597.53,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4ca8d0907a0a95219ce7025ce814a6841bb2022d",
          "message": "chore(deps): bump rustls-webpki from 0.103.10 to 0.103.13 (#9)\n\nBumps [rustls-webpki](https://github.com/rustls/webpki) from 0.103.10 to 0.103.13.\n- [Release notes](https://github.com/rustls/webpki/releases)\n- [Commits](https://github.com/rustls/webpki/compare/v/0.103.10...v/0.103.13)\n\n---\nupdated-dependencies:\n- dependency-name: rustls-webpki\n  dependency-version: 0.103.13\n  dependency-type: indirect\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-04-26T21:44:18-04:00",
          "tree_id": "bcc9eeda3399e308fa9298c513f47dd1bd446ff5",
          "url": "https://github.com/bvolpato/ivygrep/commit/4ca8d0907a0a95219ce7025ce814a6841bb2022d"
        },
        "date": 1777255098145,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 834640610,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7883.23,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3820.03,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2815.9,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10986.89,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10635.34,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 513.72,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29661.4,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8556.85,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5304.59,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450101.51,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 529.26,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7cb7a1e864f96c39fa7cf440a0220451a49e5151",
          "message": "chore(deps): bump rand from 0.8.5 to 0.8.6 (#11)\n\nBumps [rand](https://github.com/rust-random/rand) from 0.8.5 to 0.8.6.\n- [Release notes](https://github.com/rust-random/rand/releases)\n- [Changelog](https://github.com/rust-random/rand/blob/0.8.6/CHANGELOG.md)\n- [Commits](https://github.com/rust-random/rand/compare/0.8.5...0.8.6)\n\n---\nupdated-dependencies:\n- dependency-name: rand\n  dependency-version: 0.8.6\n  dependency-type: indirect\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-04-26T21:58:29-04:00",
          "tree_id": "d2eee4e4453075b40deea8d767152d45cb9431b5",
          "url": "https://github.com/bvolpato/ivygrep/commit/7cb7a1e864f96c39fa7cf440a0220451a49e5151"
        },
        "date": 1777255888628,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 851650710,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7787.33,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3879.27,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2840.56,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11174.6,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10835.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.29,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 562.55,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29224.8,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8514.36,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5278.67,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450183.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 525.59,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e346eb80d8c5aaac95aea3a78f1c13cbbef36962",
          "message": "chore(deps): bump openssl from 0.10.76 to 0.10.78 (#10)\n\nBumps [openssl](https://github.com/rust-openssl/rust-openssl) from 0.10.76 to 0.10.78.\n- [Release notes](https://github.com/rust-openssl/rust-openssl/releases)\n- [Commits](https://github.com/rust-openssl/rust-openssl/compare/openssl-v0.10.76...openssl-v0.10.78)\n\n---\nupdated-dependencies:\n- dependency-name: openssl\n  dependency-version: 0.10.78\n  dependency-type: direct:production\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-04-26T21:58:35-04:00",
          "tree_id": "9295945f605943007a5a43a0898a0200a4856fff",
          "url": "https://github.com/bvolpato/ivygrep/commit/e346eb80d8c5aaac95aea3a78f1c13cbbef36962"
        },
        "date": 1777256026078,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 837218240,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7845.92,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3825.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2801.43,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11124,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10681.05,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.27,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 553.58,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29434.43,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8522.2,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5281.43,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450294.32,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 527.11,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "03583f50293ca38e9d321669abb6908a55ac7692",
          "message": "chore(deps): bump openssl from 0.10.76 to 0.10.78 (#10)\n\nBumps [openssl](https://github.com/rust-openssl/rust-openssl) from 0.10.76 to 0.10.78.\n- [Release notes](https://github.com/rust-openssl/rust-openssl/releases)\n- [Commits](https://github.com/rust-openssl/rust-openssl/compare/openssl-v0.10.76...openssl-v0.10.78)\n\n---\nupdated-dependencies:\n- dependency-name: openssl\n  dependency-version: 0.10.78\n  dependency-type: direct:production\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-04-26T21:59:17-04:00",
          "tree_id": "fc49c1067b0c7b4f6a7dc4712a99306a16cc6127",
          "url": "https://github.com/bvolpato/ivygrep/commit/03583f50293ca38e9d321669abb6908a55ac7692"
        },
        "date": 1777256109237,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 965489430,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7816.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3847.86,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2794.05,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11871.95,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11518.69,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.91,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 555.12,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 30550.44,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 9134.23,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5701.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472835.41,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 668.53,
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
          "id": "cdca1109ece96ac063cdda9739b467824a03b910",
          "message": "perf: index-backed regex search with Tantivy pre-filtering and rayon parallelism\n\nRegex search on large repos was scanning every file on disk (289K files\nfor a 2GB+ monorepo), taking 12+ seconds even for simple patterns.\n\nNow extracts literal fragments from regex patterns (e.g. 'DDSQLizer' from\n'func.*DDSQLizer') and uses the Tantivy inverted index to pre-filter to\nonly files that could possibly match. Those candidates are then regex-scanned\nin parallel using rayon.\n\nResults on a 289K-file, 3.8M-chunk monorepo:\n- 'func.*DDSQLizer':    12s → 0.2s  (60× faster)\n- 'SELECT.*FROM.*WHERE': ~12s → 0.8s (15× faster)\n\nFalls back gracefully to filesystem walk when no index exists or no\nliteral fragments can be extracted from the pattern.",
          "timestamp": "2026-04-27T01:11:31-04:00",
          "tree_id": "1882205043ba887f61effe78d5e6f36a3a13a024",
          "url": "https://github.com/bvolpato/ivygrep/commit/cdca1109ece96ac063cdda9739b467824a03b910"
        },
        "date": 1777267250496,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 965756470,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7851.76,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3745.03,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2762.89,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11656.68,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11343.47,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.72,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 508.23,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29948.79,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8874.79,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 10768.09,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472649.64,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 564.93,
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
          "id": "e95279b5e6ff7c6cf0a5afeb2f7d380ad63858cb",
          "message": "chore: prepare v0.6.11 release\n\n- Bump version to 0.6.11\n- Add changelog entry: 60× regex speedup, path boosting, dep bumps\n- Update docs/gh-pages: version badge, regex benchmark row, monorepo scale\n- Update README: regex benchmark, monorepo performance note",
          "timestamp": "2026-04-27T10:09:32-04:00",
          "tree_id": "c380e05634f77414f8be15aff40598464c1c48e4",
          "url": "https://github.com/bvolpato/ivygrep/commit/e95279b5e6ff7c6cf0a5afeb2f7d380ad63858cb"
        },
        "date": 1777299493397,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 857113700,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7774.4,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3826,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2833.8,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11048.8,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10808.07,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.74,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 520.19,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29916.36,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8616.09,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 10466.96,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449929.96,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 547,
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
          "id": "f3595da9a4da466113b44b75a37b342389988446",
          "message": "feat: --type accepts extensions and aliases (rs, py, md, c++, bash, js, ...)\n\nThe --type flag now resolves user-friendly inputs to canonical language\nnames automatically:\n\n  ig --type rs    →  filters to 'rust' files\n  ig --type py    →  filters to 'python' files\n  ig --type md    →  filters to 'markdown' files\n  ig --type c++   →  filters to 'cpp' files\n  ig --type bash  →  filters to 'shell' files\n  ig --type .rs   →  filters to 'rust' files (dot prefix stripped)\n\nResolution happens at three levels:\n1. Canonical name match (rust, python, javascript, ...)\n2. Extension match (rs→rust, py→python, go→go, cs→csharp, ...)\n3. Common aliases (c++→cpp, c#→csharp, js→javascript, bash→shell, ...)\n\nWorks in CLI, MCP server, and daemon paths. Includes 6 new unit tests\ncovering canonical names, extensions, aliases, case-insensitivity,\ndot-prefix handling, and unknown inputs.",
          "timestamp": "2026-04-27T10:56:07-04:00",
          "tree_id": "e12599763d390016785bbf9e6a130287beb0305e",
          "url": "https://github.com/bvolpato/ivygrep/commit/f3595da9a4da466113b44b75a37b342389988446"
        },
        "date": 1777302334669,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 963610410,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7856.96,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3816.11,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2798.43,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11214.24,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11592.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.19,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 554.32,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 30683.77,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8941.62,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 11483.29,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 455756.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 603.97,
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
          "id": "8c299876797774fa0dc89c08c8a4b53d290ac95d",
          "message": "fix: update CLI help snapshot for --type help text",
          "timestamp": "2026-04-27T11:36:23-04:00",
          "tree_id": "e97bf2c3c2b9c0ba589467e6709b0b032b48c8b1",
          "url": "https://github.com/bvolpato/ivygrep/commit/8c299876797774fa0dc89c08c8a4b53d290ac95d"
        },
        "date": 1777304685872,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 829860030,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8634.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3841.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2795.6,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11222.82,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10874.34,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.77,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 519.13,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 29548.15,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8687.28,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 10572.15,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451119.38,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 579.34,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "3462216036f6aff3e2d5a7e72d75d07ad4f9a712",
          "message": "perf: batch SQLite lookups and add read-path PRAGMAs for large repos\n\n- Add mmap_size, cache_size, temp_store PRAGMAs to open_sqlite_readonly\n  for dramatically better cold-start I/O on multi-GB databases\n- Add fetch_chunks_by_vector_keys_batch() using WHERE vector_key IN (...)\n  to replace O(N) individual SQLite round-trips with 1-2 batched queries\n- Use prepare_cached() for fetch_chunk_by_vector_key to reuse compiled SQL\n- Wire batch fetch into lexical text population and semantic candidate paths\n\nOn a large repo (~290K files, ~3.8M chunks, ~11GB index):\n  Neural hybrid (warm): 0.73s -> 0.57s (22% faster)\n  Hash hybrid (warm):   4.1s  -> 0.51s (87% faster / 8x)\n  Cold start:           5.4s  -> 3.5s  (35% faster)",
          "timestamp": "2026-04-27T16:59:37-04:00",
          "tree_id": "8e928630aea9ce26998ab9593f10dae52711dda4",
          "url": "https://github.com/bvolpato/ivygrep/commit/3462216036f6aff3e2d5a7e72d75d07ad4f9a712"
        },
        "date": 1777324157672,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 940528330,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7991.49,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3721.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2739.77,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11754.61,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11592.14,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.6,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 515.83,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 22343.02,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8363.32,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 10748.9,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472195.51,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 584.67,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "5e0403c2089d57a98f98f5fa5e1d8f915382fb40",
          "message": "release: v0.6.12",
          "timestamp": "2026-04-27T23:26:58-04:00",
          "tree_id": "b34e5bf13fe5219f01df31711b5d0ecb0b370382",
          "url": "https://github.com/bvolpato/ivygrep/commit/5e0403c2089d57a98f98f5fa5e1d8f915382fb40"
        },
        "date": 1777347361284,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 850459420,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7685.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3866.33,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2845.98,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11006.06,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10621.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 519.46,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20908.46,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7703.82,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 10422.32,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449736.42,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 543.27,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "fd2801f7052a93a214087785cacf725a6d4f43d5",
          "message": "release: v0.6.13 — doctor subcommand → --doctor flag\n\nBreaking: `ig doctor` is now `ig --doctor`. The word 'doctor' was\nsilently intercepted before clap parsing, making it impossible to search\nfor the word 'doctor'. Now uses a proper --doctor flag with --fix as\na dependent flag (requires --doctor).\n\n- Convert `doctor` positional subcommand to `--doctor` clap flag\n- Add `--fix` flag with `requires = \"doctor\"` constraint\n- Remove raw env::args() interception in maybe_run_doctor_command()\n- Update all user-facing strings, docs, tests, and snapshots\n- Suppress pre-existing dead_code warnings for filtered chunk helpers",
          "timestamp": "2026-04-28T11:54:02-04:00",
          "tree_id": "f2fd2d283109653dce2f15887d8b18c67cf0133e",
          "url": "https://github.com/bvolpato/ivygrep/commit/fd2801f7052a93a214087785cacf725a6d4f43d5"
        },
        "date": 1777392228901,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 949506960,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7697.32,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3917.98,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2819.03,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11153.11,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10856.27,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.42,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 564.43,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19915.46,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8205.51,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 10362.92,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449556.66,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 513.57,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "f2c22e1f6a9d434584fe94a7c6203fde41e34f36",
          "message": "release: v0.6.14 — regex perf fix + filter score tests\n\nPerformance:\n- Hoist regex matcher out of par_iter hot loop in regex_search_parallel,\n  eliminating per-file regex compilation overhead\n\nTests:\n- Add 8 unit tests for filter_meaningful_scores covering adaptive\n  threshold behavior, literal source bypass, and edge cases",
          "timestamp": "2026-04-29T10:22:19-04:00",
          "tree_id": "bf3c284beb5090f61cbe2c32836a66b9008fedb4",
          "url": "https://github.com/bvolpato/ivygrep/commit/f2c22e1f6a9d434584fe94a7c6203fde41e34f36"
        },
        "date": 1777473116441,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 894666580,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 11971.29,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2899.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2145.04,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9432.22,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9161.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 5.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 393.69,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16465.22,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 10800.41,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5261.89,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 375244.29,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 471.98,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "e2af3262379f95f25ad577308662e414c4cb38fd",
          "message": "fix: rustfmt formatting in filter_meaningful_scores tests",
          "timestamp": "2026-04-30T01:20:08-04:00",
          "tree_id": "290eddd6be550614a8f9e43df4e730330579a0ce",
          "url": "https://github.com/bvolpato/ivygrep/commit/e2af3262379f95f25ad577308662e414c4cb38fd"
        },
        "date": 1777526990274,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 1021500940,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4609.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3634.51,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2649.05,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 6694.67,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5321.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.94,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 684.88,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15745.74,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 5281.11,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 3626.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473763.12,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 411.11,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "67be267c15f384d719e3ecf83058dca92f8b86b5",
          "message": "perf: cap background CPU — wire thread limits and nice(10)\n\nThe background neural enhancement subprocess was running at full CPU\n(400%+ on multi-core machines) because the _is_background flag in\nCandleEmbeddingModel::new_internal was dead code (prefixed with _).\n\nChanges:\n- Wire is_background to set rayon global pool to cores/4 (min 1)\n- Add nice(10) via pre_exec on the --enhance-internal subprocess\n- Use a dedicated rayon ThreadPool (cores/2) for HashEmbeddingModel\n  embed_batch instead of the unbounded global pool",
          "timestamp": "2026-05-02T00:40:17-04:00",
          "tree_id": "9e7999b56b3f2d2bec8080a65a7e8347287ab4a2",
          "url": "https://github.com/bvolpato/ivygrep/commit/67be267c15f384d719e3ecf83058dca92f8b86b5"
        },
        "date": 1777697389167,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 888241350,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 9358.81,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3860.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2824.8,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11523.78,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11164.35,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.92,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 734.57,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20158.91,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8368.13,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6265.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450376.86,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 585.01,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "82e6c2363adfcf3a71cf0d888f75840a394425f1",
          "message": "perf: eliminate redundant lowercasing in search scoring loop\n\nIntroduce ChunkBoostContext that precomputes text_lower, path_lower,\npath_segments, file_stem, first_line, text_compact, and path_compact\nonce per candidate chunk. All 7 boost functions and file_authority_score\nnow use the precomputed context instead of independently lowercasing\nthe same strings (~10 redundant allocations per candidate eliminated).\n\nAlso: cache the hash embedding thread pool in a OnceLock so it's built\nonce instead of per embed_batch call.\n\nAdds 26 new tests:\n- 8 ChunkBoostContext field correctness tests\n- 10 individual boost function tests with precomputed context\n- 3 embed_batch thread pool consistency tests (200-item batch)\n- 2 E2E hybrid search integration tests validating the full pipeline\n- 3 file_authority_score tests",
          "timestamp": "2026-05-02T00:50:59-04:00",
          "tree_id": "dd3682d04c82c7a46fb7981181dc91848beafa2b",
          "url": "https://github.com/bvolpato/ivygrep/commit/82e6c2363adfcf3a71cf0d888f75840a394425f1"
        },
        "date": 1777698046121,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 907185000,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8113.1,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3914.07,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2827.63,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11018.66,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10741.19,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.28,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 732.8,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20346.16,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8346.69,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6222.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450254.63,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 545.75,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "9206ba93dd69f40e04ef1fb95d3fd50a8b1db5f2",
          "message": "perf: deduplicate hot-path calls across search pipeline\n\nSecond pass of the optimization audit:\n\n1. build_lexical_queries: was called 3x with the same input per hybrid\n   search (literal pass, lexical pass, path-match pass). Now computed\n   once and shared across all three passes.\n\n2. HashEmbeddingModel::new(256): was recreated on every search query\n   (rebuilding the alias hash map). Now cached in a static OnceLock so\n   it's built once for the process lifetime.\n\n3. RRF accumulation: consolidated 3 separate HashMaps (scores, chunks,\n   sources) into a single RrfEntry struct map, eliminating 6 redundant\n   chunk_id.clone() calls per candidate across accumulation passes.\n\n4. summarize_reason: focus.to_ascii_lowercase() was called once for\n   the contains-check and again per token in the loop — now hoisted\n   to a single allocation.\n\n5. to_hit: used c.to_string() to clone pre-read file content even\n   when passed by reference. Now uses Cow<str> for zero-copy borrows.",
          "timestamp": "2026-05-02T11:38:45-04:00",
          "tree_id": "6cc0fc022455fc2a1ff5837c86ba6ff52284d069",
          "url": "https://github.com/bvolpato/ivygrep/commit/9206ba93dd69f40e04ef1fb95d3fd50a8b1db5f2"
        },
        "date": 1777736909780,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 904842770,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7905.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3834.06,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2832.59,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11056.99,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10789.26,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.09,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 728.42,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20050.67,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8331.92,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6289.6,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449897.93,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 532.08,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "b7cf4c451e0e292a810d427fe46ff42db05b9a46",
          "message": "perf: cache hash model in daemon + add watcher debounce\n\nTwo high-impact daemon optimizations:\n\n1. HashEmbeddingModel cached process-wide: The daemon was calling\n   create_model(true) on every watcher filesystem event, every Index\n   request, and every fallback search (when ONNX is still loading).\n   Now uses a static OnceLock<Arc<dyn EmbeddingModel>> so the alias\n   hash map is built exactly once for the daemon lifetime.\n\n2. Watcher debounce: The watch worker was waking and starting\n   re-indexing immediately on the first notify event. Added a 300ms\n   debounce sleep so burst saves (e.g., cargo fmt touching 50 files)\n   coalesce into a single indexing pass.\n\nAlso fixes escaped quotes in README CLI examples.",
          "timestamp": "2026-05-02T16:33:51-04:00",
          "tree_id": "811a4adf9a041117a46a0e0d2b4e6ce89af7d6e8",
          "url": "https://github.com/bvolpato/ivygrep/commit/b7cf4c451e0e292a810d427fe46ff42db05b9a46"
        },
        "date": 1777754591311,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 915355700,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7864.81,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3826.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2836.48,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11060.93,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10734.25,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 711.65,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19871.53,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8267.97,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6176.82,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449650.85,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 526.42,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "a491fae33a5d01ca76c7de0e92d47964ebe60695",
          "message": "release: v0.6.16 — daemon model cache + watcher debounce",
          "timestamp": "2026-05-03T00:38:15-04:00",
          "tree_id": "91b410a3586515e63d9d00d80f7350ec4df5d6db",
          "url": "https://github.com/bvolpato/ivygrep/commit/a491fae33a5d01ca76c7de0e92d47964ebe60695"
        },
        "date": 1777783630014,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 790310270,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8946.65,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3746.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2747.94,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11525.68,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11269.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 714.01,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20531.27,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8750.79,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6561.83,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471513.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 563.04,
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
          "id": "54ea816d7b581c18f4fca61efb3d8d521e29a017",
          "message": "[daemon] tame Linux watcher storms",
          "timestamp": "2026-05-04T08:02:24-04:00",
          "tree_id": "d6688d0cbe8f6c7651672f52ab54c4db1377ece1",
          "url": "https://github.com/bvolpato/ivygrep/commit/54ea816d7b581c18f4fca61efb3d8d521e29a017"
        },
        "date": 1777896720386,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 905547570,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7767.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3857.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2832.86,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11329.43,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11097.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.2,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 726.4,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20212.33,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8288.99,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6141.75,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450689.41,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 527.24,
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
          "id": "5e2462a10559c13c3c92254f70078d95816684a1",
          "message": "[daemon] fix watcher gitignore matching",
          "timestamp": "2026-05-04T09:13:16-04:00",
          "tree_id": "03d724793d6b977d493c0571947daa4037f64d0c",
          "url": "https://github.com/bvolpato/ivygrep/commit/5e2462a10559c13c3c92254f70078d95816684a1"
        },
        "date": 1777900978672,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 895118800,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8646.39,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3724.64,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2768.98,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11856.46,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11517.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.17,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 722.1,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20873.51,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8827.13,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6581.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471710.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 578.11,
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
          "id": "38a8d4a94e79264b11c117729bd753c0ea63ca31",
          "message": "[daemon] handle canonical watcher paths",
          "timestamp": "2026-05-04T09:20:58-04:00",
          "tree_id": "b31af1a5cac9db29e789117cfcd65b3e3b7f2f11",
          "url": "https://github.com/bvolpato/ivygrep/commit/38a8d4a94e79264b11c117729bd753c0ea63ca31"
        },
        "date": 1777901431991,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 915697040,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7843.12,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3858.21,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2812.84,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11208.67,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10829.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.21,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 741.07,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20056.11,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8343.85,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6223.37,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 452310.62,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 533.39,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "feb8c9c86d2f1b251dd5acf803f157944be4d3a4",
          "message": "fix: Linux stability — cap memory, guard OOM, detect inotify limits\n\nCritical fixes for Linux crashes (OOM killer):\n\n1. Vector store capacity growth capped at 256K entries per reallocation\n   (was unbounded 2x doubling — could allocate 512MB in one shot)\n\n2. Tantivy writer heap: 200MB → 50MB\n\n3. SQLite WAL cache: 64MB → 16MB\n\n4. Pre-indexing memory guard: checks /proc/meminfo and refuses to\n   start when MemAvailable < 512 MiB\n\n5. WAL checkpoint(TRUNCATE) after indexing to reclaim disk\n\n6. inotify ENOSPC detection with actionable sysctl guidance",
          "timestamp": "2026-05-04T10:20:34-04:00",
          "tree_id": "406cb3079c3bde0b84eb0abc29e3327f9420cbd8",
          "url": "https://github.com/bvolpato/ivygrep/commit/feb8c9c86d2f1b251dd5acf803f157944be4d3a4"
        },
        "date": 1777905016157,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 888118530,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 9857.74,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3831.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2846.44,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10988.93,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10820.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 717.48,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19437.69,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7949.01,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5906.3,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451904.49,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 545.13,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "e57bb94cfcbd4eb055a8657854e5215e747368ab",
          "message": "ci: add cross-platform E2E test workflow\n\n- New e2e-cross-platform.yml workflow with 5 test jobs:\n  * Linux x86_64 (native) — full test suite + E2E smoke\n  * macOS ARM64 (native) — full test suite + E2E smoke\n  * macOS x86_64 (native) — full test suite + E2E smoke\n  * Linux aarch64 (QEMU) — cross test + Docker smoke test\n  * Linux x86_64 (hash-only) — no-neural feature gate test\n\n- Triggers: manual (workflow_dispatch with architecture picker\n  and test scope selector), release tags, weekly schedule\n\n- Upgraded release.yml smoke test from bare --help to full\n  index+search cycle on native runners\n\n- Summary job produces a markdown table of results",
          "timestamp": "2026-05-04T10:40:06-04:00",
          "tree_id": "acebd6e6b7b910abfe9e02d2f91a4878ede4f9d7",
          "url": "https://github.com/bvolpato/ivygrep/commit/e57bb94cfcbd4eb055a8657854e5215e747368ab"
        },
        "date": 1777906104053,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 774642560,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8965.65,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3834.3,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2827.29,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11165.59,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10894.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.05,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 719.03,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19560.02,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8079.47,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6021.3,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450915.84,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 531.72,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "49ab3268016ee310d37e711b34f2ee24addc3dc6",
          "message": "fix(ci): use --rm instead of --remove in E2E workflows",
          "timestamp": "2026-05-04T10:52:11-04:00",
          "tree_id": "7bc03078ecc8415e4b554d2ee9403e6c3182d707",
          "url": "https://github.com/bvolpato/ivygrep/commit/49ab3268016ee310d37e711b34f2ee24addc3dc6"
        },
        "date": 1777906843744,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 779059540,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8190.49,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3833.68,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2812.03,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11166.25,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10775.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 738.33,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19540.02,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8042.69,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5947.06,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450946.7,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 539.9,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "35a8fd4726992a06be5edf9291f838c41dfd4591",
          "message": "fix(ci): isolate E2E smoke tests with mktemp to avoid workspace conflicts\n\nThe E2E smoke tests were running inside the checkout directory, which\ncaused ig to auto-index the repo itself. When the test then tried to\n--add a /tmp/ sub-path, it conflicted with the already-indexed workspace.\n\nFix: use mktemp -d for fully isolated temp dirs.\nAlso mark QEMU cross-tests as continue-on-error since they flake\nunder emulation.",
          "timestamp": "2026-05-04T11:10:10-04:00",
          "tree_id": "61bf5db344adda6c47ed7a884ad78c190a013e76",
          "url": "https://github.com/bvolpato/ivygrep/commit/35a8fd4726992a06be5edf9291f838c41dfd4591"
        },
        "date": 1777907907116,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 771998290,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8120.19,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3810.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2815.66,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11107.48,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10812.44,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 732.13,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19734.86,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8073.51,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5921.23,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451850.37,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 493.89,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "d335ca2b9dc42e047f0b57233f0741f5116d75eb",
          "message": "fix(ci): E2E smoke tests — index before search to avoid auto-index conflicts\n\nLiteral file searches trigger first-run auto-indexing of sub-paths,\nwhich then conflicts with the explicit --add. Fix: always --add first,\nthen search in the already-indexed workspace.",
          "timestamp": "2026-05-04T11:27:37-04:00",
          "tree_id": "505e8fab73c3349a69d3c750e5a166141e2fd091",
          "url": "https://github.com/bvolpato/ivygrep/commit/d335ca2b9dc42e047f0b57233f0741f5116d75eb"
        },
        "date": 1777908978750,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 777084030,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7904.39,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3848.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2833.75,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10941.78,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10533.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.11,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 712.24,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19254.68,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7892.03,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5797.61,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449779.81,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 493.6,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2394d5326c2b4ef08f5d86a169284e6867f27f0d",
          "message": "chore(deps): bump openssl in the cargo group across 1 directory (#12)\n\nBumps the cargo group with 1 update in the / directory: [openssl](https://github.com/rust-openssl/rust-openssl).\n\n\nUpdates `openssl` from 0.10.78 to 0.10.79\n- [Release notes](https://github.com/rust-openssl/rust-openssl/releases)\n- [Commits](https://github.com/rust-openssl/rust-openssl/compare/openssl-v0.10.78...openssl-v0.10.79)\n\n---\nupdated-dependencies:\n- dependency-name: openssl\n  dependency-version: 0.10.79\n  dependency-type: direct:production\n  dependency-group: cargo\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-05-10T16:27:14-04:00",
          "tree_id": "27aea713764e0875de1da6daab98dd2bc11c77ca",
          "url": "https://github.com/bvolpato/ivygrep/commit/2394d5326c2b4ef08f5d86a169284e6867f27f0d"
        },
        "date": 1778445584813,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 785150710,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8322.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3732.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2768.31,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11528.79,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11172.57,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.11,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 743.08,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20426.65,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8470.08,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6187.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473015.09,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 571.35,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "109e9beaa6c0408c554570c8a80aeb00d17c5d20",
          "message": "perf(index): speed up Linux kernel cold indexing\n\nOptimize fresh indexing on large repositories, add Linux-kernel benchmark/report artifacts, guard benchmark temp-home deletion under /tmp, and add a CI Criterion benchmark for 30k-chunk fresh indexes.",
          "timestamp": "2026-05-13T08:34:39-04:00",
          "tree_id": "2ba5baa0b9f873c88c964f9441a1b48874c595d2",
          "url": "https://github.com/bvolpato/ivygrep/commit/109e9beaa6c0408c554570c8a80aeb00d17c5d20"
        },
        "date": 1778676463034,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 883041170,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 10231.51,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 18263469.86,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3703.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2742.64,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 12549.51,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11990.41,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.5,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 845.03,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 21125.14,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8948.48,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6587.06,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 475318.7,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 622.74,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4efd4052a4963886699204f180cffa58918b311c",
          "message": "perf(search): speed up base query paths (#14)",
          "timestamp": "2026-05-13T09:48:07-04:00",
          "tree_id": "77fd73f8f9b88fe7ea56fd6a05168f1fbc1f4bec",
          "url": "https://github.com/bvolpato/ivygrep/commit/4efd4052a4963886699204f180cffa58918b311c"
        },
        "date": 1778680949954,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 850739000,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 10554.52,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17967363.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3757.79,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2751.8,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11585.05,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11369.72,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 734.2,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20694.58,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8432.55,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6294.49,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17940.56,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11092.53,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3222.68,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2467.9,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473313.69,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 587.94,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a99f5fb6eafef9c41d656a42dccaf44e27972d32",
          "message": "perf(search): cache daemon hot queries (#15)\n\n* bench: add daemon hot query benchmark\n\n* experiment: cache daemon search contexts\n\n* experiment: cache daemon query results\n\n* experiment: use quick query index health\n\n* fix: satisfy clippy for search context helpers\n\n* bench: emit numeric daemon benchmark flags\n\n* test: add daemon equivalence harness\n\n* experiment: skip static daemon status in bench mode\n\n* docs: add daemon hot query benchmark report\n\n* fix: separate all-index query cache keys\n\n* fix: verify index health before repair skip\n\n* fix: compare daemon checks against local baselines\n\n* fix: fall back from stale daemon sockets",
          "timestamp": "2026-05-13T23:20:34-04:00",
          "tree_id": "4d0dee994983d5d96b5c53d01d0de2d85fdcd589",
          "url": "https://github.com/bvolpato/ivygrep/commit/a99f5fb6eafef9c41d656a42dccaf44e27972d32"
        },
        "date": 1778729673428,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 832174260,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8513.38,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 16828708.87,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3792.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2790.6,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10991.17,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10638.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.72,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 694.24,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19240.4,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7796.27,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5888.65,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 16999.45,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 10677.2,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2965.64,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2195.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449642.63,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 482.92,
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
          "id": "0538fc371058df98e4be375c6122b8ff6931485d",
          "message": "fix(ci): avoid cached cargo binary restores",
          "timestamp": "2026-05-14T00:07:22-04:00",
          "tree_id": "2c42a5bd71405b58738583b3614262512003b033",
          "url": "https://github.com/bvolpato/ivygrep/commit/0538fc371058df98e4be375c6122b8ff6931485d"
        },
        "date": 1778732924974,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 740938540,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8421.26,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17910725.32,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3714.24,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2761.84,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11570.15,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11343.83,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 694.93,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20241.13,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8467.15,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6241.53,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17802.08,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11394.01,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3197.2,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2443.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471358.69,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 565,
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
          "id": "b3eaadec6d0bd5d1feee7ede07afdaa544ae3588",
          "message": "ci: avoid docs publish cancellation",
          "timestamp": "2026-05-14T17:43:17-04:00",
          "tree_id": "b6daee3bba1e9d61ac3039d37ac24e8b41d4acb6",
          "url": "https://github.com/bvolpato/ivygrep/commit/b3eaadec6d0bd5d1feee7ede07afdaa544ae3588"
        },
        "date": 1778795788060,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 737043140,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 7989.9,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 16954764.13,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3798.32,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2810.8,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11129.06,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10915.86,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 733.84,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19579.96,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8010.73,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5944.43,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17344.97,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11236.36,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3039.79,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2236.44,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449956.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 520.3,
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
          "id": "84d023630c9c59cf654d8aa94b1a5f7f9deac082",
          "message": "search: tighten relevance quality gates",
          "timestamp": "2026-05-14T21:08:06-04:00",
          "tree_id": "95eff8d7c4d622e4ff7ba79474803953d18eaad1",
          "url": "https://github.com/bvolpato/ivygrep/commit/84d023630c9c59cf654d8aa94b1a5f7f9deac082"
        },
        "date": 1778808157594,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 840691670,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8212.43,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17042829.55,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3820.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2829.3,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10973.49,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10820.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.22,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 712.49,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 19759.2,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7820,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5907.19,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17510.13,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11215.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2947.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2194.09,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449621.75,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 516.25,
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
          "id": "96b5a1debe832a355767d9047254f56396f2d83e",
          "message": "Add Linux relevance benchmark baseline",
          "timestamp": "2026-05-15T17:05:00-04:00",
          "tree_id": "5cd42ebf26c08120dd180c3e43194d8fe203af0e",
          "url": "https://github.com/bvolpato/ivygrep/commit/96b5a1debe832a355767d9047254f56396f2d83e"
        },
        "date": 1778879995573,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 860611420,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8483.61,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17963375.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3726.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2748.1,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11800.47,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11334.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 686.02,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 21036.15,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8561.57,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6344.67,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18376.44,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11700.23,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3174.69,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2450.9,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472695,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 577.92,
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
          "id": "8a460772a0cb83784df301a63de14ea15ce3945b",
          "message": "experiment: apply driver demotion to authority filtering",
          "timestamp": "2026-05-15T17:25:46-04:00",
          "tree_id": "c988829b11ea94e4dd0588b204245938bdba72bd",
          "url": "https://github.com/bvolpato/ivygrep/commit/8a460772a0cb83784df301a63de14ea15ce3945b"
        },
        "date": 1778881367745,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 883047510,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8538.82,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 18442302.84,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3742.67,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2775.29,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 12017.59,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11659.66,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.48,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 719.35,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 21295.44,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8436.97,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6282.87,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19306.63,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12254.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3175.63,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2459.05,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470963.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 568.37,
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
          "id": "96b5a1debe832a355767d9047254f56396f2d83e",
          "message": "Add Linux relevance benchmark baseline",
          "timestamp": "2026-05-15T17:05:00-04:00",
          "tree_id": "5cd42ebf26c08120dd180c3e43194d8fe203af0e",
          "url": "https://github.com/bvolpato/ivygrep/commit/96b5a1debe832a355767d9047254f56396f2d83e"
        },
        "date": 1778887827324,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 883331600,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 9950.43,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17968620.68,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3944.18,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2884.34,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 12107.89,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11705.43,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 704.36,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 21335,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8370.53,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6260.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19326.21,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13104,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3290.76,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2274.7,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451341.23,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 575.79,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8e575e4a6dbd22e886e1538b042446370a576022",
          "message": "search: add generic relevance scoring (#16)",
          "timestamp": "2026-05-15T19:47:12-04:00",
          "tree_id": "6aa1d44c2a672e8abc0df5be6c4bbda2439ea10b",
          "url": "https://github.com/bvolpato/ivygrep/commit/8e575e4a6dbd22e886e1538b042446370a576022"
        },
        "date": 1778889697629,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 855141550,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8139.88,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 16769048.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3795.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2812.29,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10874.97,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10714.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 692.74,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 20813.22,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7786.86,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5777.39,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18795.09,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12645.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2953.39,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2197.41,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 448369.4,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 471.41,
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
          "id": "0dd329416c094a442c001cbdcf329702a59f4b3a",
          "message": "search: move aliases into generated dictionary",
          "timestamp": "2026-05-15T20:39:56-04:00",
          "tree_id": "9aff6b6c5c73ed551bb7d392663f82606eccd048",
          "url": "https://github.com/bvolpato/ivygrep/commit/0dd329416c094a442c001cbdcf329702a59f4b3a"
        },
        "date": 1778892901260,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 839196400,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8493.36,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 18065448.42,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3708.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2748.78,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11562.4,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11377.51,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.93,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 708.94,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 22108.94,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8560.59,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6361.75,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19968.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13593.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3127.32,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2475.4,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472369.25,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 570.32,
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
          "id": "9de97e1677cc4f8c8690748a29d003dd4fefd83c",
          "message": "scripts: expand help output",
          "timestamp": "2026-05-15T20:55:48-04:00",
          "tree_id": "c5ccbcb4b0cf0561a0ec00ddbbdb6b0e72d5838a",
          "url": "https://github.com/bvolpato/ivygrep/commit/9de97e1677cc4f8c8690748a29d003dd4fefd83c"
        },
        "date": 1778893766727,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 734996290,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8420.13,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 18100870.21,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3751.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2739.06,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11763.33,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11574.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 704.83,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 22169.69,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 8452.58,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 6389.52,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 20380.71,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13785.6,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3188.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2487.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 474434.91,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 570.7,
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
          "id": "1bd029690027c70679aad636c2ced23c15fd82e0",
          "message": "release: harden advertising readiness",
          "timestamp": "2026-05-18T12:43:28-04:00",
          "tree_id": "19d01a31a5ee432a7eef85b3b2846972c8d939f6",
          "url": "https://github.com/bvolpato/ivygrep/commit/1bd029690027c70679aad636c2ced23c15fd82e0"
        },
        "date": 1779123441725,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 731111100,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 9498.51,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17044677.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3813.67,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2806.77,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11141.48,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10809.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.04,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 726.7,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 21234.32,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7947.95,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5980.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19161.64,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13134.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2999.28,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2241.96,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450535.05,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 583.35,
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
          "id": "69f379f70f4731c05864c78803a50c8a056ef731",
          "message": "ci: normalize e2e temp paths",
          "timestamp": "2026-05-18T12:52:09-04:00",
          "tree_id": "a62d6f3bc051170c9148581f42c2cc1c0abe3dd9",
          "url": "https://github.com/bvolpato/ivygrep/commit/69f379f70f4731c05864c78803a50c8a056ef731"
        },
        "date": 1779123944073,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 739908270,
            "unit": "ns"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8280.15,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 16973562.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3799.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2804.91,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10902.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10671.36,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.12,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 715.83,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 21267.95,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 7905.13,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 5935.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19256.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13084.44,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2975.77,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2196.93,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451843.04,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 538.93,
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
          "id": "24146ecceab2d80b66faa933aea19da4cc40c868",
          "message": "bench: stabilize noisy benchmark reporting",
          "timestamp": "2026-05-18T13:36:44-04:00",
          "tree_id": "f655a58bd333c33d6d0e29d81dbdf446d25105f5",
          "url": "https://github.com/bvolpato/ivygrep/commit/24146ecceab2d80b66faa933aea19da4cc40c868"
        },
        "date": 1779127126036,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8349.85,
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
          "id": "4476f37b8cee36921c921f60e6d8c5dd11444d0e",
          "message": "bench: avoid stale local baseline noise",
          "timestamp": "2026-05-18T14:40:27-04:00",
          "tree_id": "d8341ee72c84521f3e302149b58663aece2b5484",
          "url": "https://github.com/bvolpato/ivygrep/commit/4476f37b8cee36921c921f60e6d8c5dd11444d0e"
        },
        "date": 1779130508306,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8346.88,
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
          "id": "55b19cf7b9fff68b2a32f943983e8e1a7de9f279",
          "message": "bench: lengthen timed samples",
          "timestamp": "2026-05-18T15:22:50-04:00",
          "tree_id": "f5f7bb8ffd91a3491fd8d95fd1452668ede79a61",
          "url": "https://github.com/bvolpato/ivygrep/commit/55b19cf7b9fff68b2a32f943983e8e1a7de9f279"
        },
        "date": 1779133035527,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 8018.4,
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
          "id": "4dc1f62568216b8a33edb5d639e1a7400f6b470c",
          "message": "release: v0.6.20",
          "timestamp": "2026-05-18T15:45:48-04:00",
          "tree_id": "126e7ebc793ad0475c8793c7d8122efc4f201e0f",
          "url": "https://github.com/bvolpato/ivygrep/commit/4dc1f62568216b8a33edb5d639e1a7400f6b470c"
        },
        "date": 1779134483916,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3620.53,
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
          "id": "0f1fdc56a42d6b70172a7c2230dde818ec1f8c5f",
          "message": "docs: fix Homebrew monitoring setup",
          "timestamp": "2026-05-18T16:10:51-04:00",
          "tree_id": "5d9fdf48253fc6d8c6abe56dc841519be5f80518",
          "url": "https://github.com/bvolpato/ivygrep/commit/0f1fdc56a42d6b70172a7c2230dde818ec1f8c5f"
        },
        "date": 1779136340634,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3758.85,
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
          "id": "8bd30cbc3cbf9c08033ca3f48a30d7a16cfbba69",
          "message": "release: v0.7.0",
          "timestamp": "2026-05-23T01:12:26-04:00",
          "tree_id": "d166e0cac0fc47c894286dc113175d9c50ad86eb",
          "url": "https://github.com/bvolpato/ivygrep/commit/8bd30cbc3cbf9c08033ca3f48a30d7a16cfbba69"
        },
        "date": 1779513997994,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3562.31,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6485ddcd1dd7f6ca25d389e3fb9d8ef20b5d23b1",
          "message": "Merge pull request #36: relevance + stability fixes (v0.7.0)\n\nFix noisy search relevance + two stability bugs (P0/P1)",
          "timestamp": "2026-05-23T01:11:22-04:00",
          "tree_id": "9c9c03d05192d6563120f3dcf9f1a8bcacefcc56",
          "url": "https://github.com/bvolpato/ivygrep/commit/6485ddcd1dd7f6ca25d389e3fb9d8ef20b5d23b1"
        },
        "date": 1779514002670,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3538.26,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d0b18ff8eee764f96e3b921256851e6c3b23fc7d",
          "message": "Merge pull request #45: harden indexing/search correctness + benchmarks\n\nHarden indexing/search: fix stale-chunk, literal panic, regex alternation, MCP --all paths",
          "timestamp": "2026-05-23T11:30:53-04:00",
          "tree_id": "11a884865807837484a28ef4689428d50bcd7a7d",
          "url": "https://github.com/bvolpato/ivygrep/commit/d0b18ff8eee764f96e3b921256851e6c3b23fc7d"
        },
        "date": 1779551687224,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3632.5,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b327eb128b4e28ec56f81c11a59d60ece9f232f4",
          "message": "Merge pull request #46: remove MCP --all + overlay-aware index health\n\nRemove MCP --all (sandbox risk) + overlay-aware index health",
          "timestamp": "2026-05-23T13:04:55-04:00",
          "tree_id": "629dd9d15230ab222a41c496922b5c4be5daa3fe",
          "url": "https://github.com/bvolpato/ivygrep/commit/b327eb128b4e28ec56f81c11a59d60ece9f232f4"
        },
        "date": 1779556917077,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3617.27,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f6ad2efc541ebf6725d62625aa6efea4dddbaa48",
          "message": "Merge pull request #55: harden daemon/IPC/MCP security + robustness\n\nHarden daemon/IPC/MCP: socket auth + permissions, robustness, DoS caps",
          "timestamp": "2026-05-23T22:43:34-04:00",
          "tree_id": "457c1f9b2a972fea54972d9cac9e5cd420d4ba72",
          "url": "https://github.com/bvolpato/ivygrep/commit/f6ad2efc541ebf6725d62625aa6efea4dddbaa48"
        },
        "date": 1779591675363,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3522.67,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6d706ae980c2cc4bc5350d998273250e9383ddd0",
          "message": "Merge pull request #37 from bvolpato/dependabot/cargo/cargo-b5bfc02d2b\n\nchore(deps): bump openssl from 0.10.79 to 0.10.80 in the cargo group across 1 directory",
          "timestamp": "2026-05-24T01:03:53-04:00",
          "tree_id": "9345dcdbbad6f4f9ac4b4c158ed6c966939b5fdd",
          "url": "https://github.com/bvolpato/ivygrep/commit/6d706ae980c2cc4bc5350d998273250e9383ddd0"
        },
        "date": 1779600148323,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3744.18,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3717622a908d2510f8de8fe8ca4586991a027773",
          "message": "Merge pull request #60 from bvolpato/fix/parallel-mcp-meltdown\n\nHarden MCP & daemon for large repos and parallel use (P0 inline-neural index + 3 fixes)",
          "timestamp": "2026-05-25T09:59:38-04:00",
          "tree_id": "d727e4266569f62cf8462fa9d96111d2168b17e1",
          "url": "https://github.com/bvolpato/ivygrep/commit/3717622a908d2510f8de8fe8ca4586991a027773"
        },
        "date": 1779718664878,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3813.07,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b8238eead73877b4800d98a36f5db4cb3fb2a9ca",
          "message": "Merge pull request #61 from bvolpato/fix/large-repo-file-size-cap\n\nSkip minified bundles / single-line blobs when indexing",
          "timestamp": "2026-05-25T12:36:22-04:00",
          "tree_id": "2f2a9149b4aa4ab49a858be6e22471f686a6b291",
          "url": "https://github.com/bvolpato/ivygrep/commit/b8238eead73877b4800d98a36f5db4cb3fb2a9ca"
        },
        "date": 1779728046601,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3713.99,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ec753a0e5b93ac6a0c625315a559985322a29aac",
          "message": "Merge pull request #64 from bvolpato/relevance/eval-gate-hardening\n\nRelevance eval gate in CI + reliable neural measurement (#20)",
          "timestamp": "2026-05-25T13:38:51-04:00",
          "tree_id": "aa35a7847d3db4b2415f8cf21386d6bedd1af5eb",
          "url": "https://github.com/bvolpato/ivygrep/commit/ec753a0e5b93ac6a0c625315a559985322a29aac"
        },
        "date": 1779731736407,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 598250.74,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3656.05,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 16926234.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3825.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2810.73,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10978.62,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10733.17,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.49,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 686.53,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16207.76,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3884.87,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1393.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18787.33,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12818.18,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2987.27,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2195.23,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451199.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 174.11,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 43562.68,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1484.31,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bd1fd5201edf19a207987d8b477a08627f7953c3",
          "message": "Merge pull request #65 from bvolpato/fix/enhance-throttle-load\n\nRelax + configure the neural enhancement load-throttle (#62)",
          "timestamp": "2026-05-25T14:35:08-04:00",
          "tree_id": "21e20fe9bbaa92a8c12cf1ee37eebf08c4ed2077",
          "url": "https://github.com/bvolpato/ivygrep/commit/bd1fd5201edf19a207987d8b477a08627f7953c3"
        },
        "date": 1779735155302,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 718394.78,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3756.6,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 16891742.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3814.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2810.48,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11129.77,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10748.31,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 711.74,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16453.38,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3950.03,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1418.57,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18791.17,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13190.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3005.56,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2225.71,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449825.72,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 176.49,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 44107.18,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1551.98,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5086dd16b9bfa7c1aae4fc21750d25b6dc46a3fd",
          "message": "Merge pull request #68 from bvolpato/fix/site-version-and-noscript\n\nfix(site): version 0.7.0 + reveal content without JS / reduced-motion",
          "timestamp": "2026-05-25T17:57:35-04:00",
          "tree_id": "b60b81db6ee0fa7860977c6e11eef80c65e79231",
          "url": "https://github.com/bvolpato/ivygrep/commit/5086dd16b9bfa7c1aae4fc21750d25b6dc46a3fd"
        },
        "date": 1779747254912,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 596644.73,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3702.71,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 16867415.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3804.76,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2819.09,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11132.13,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10758.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.4,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 692.48,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16380.49,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3938.56,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1413.51,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18870.81,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13152.12,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2985.98,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2215.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449193.2,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 174.38,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 43193,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1498.19,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0eef82d7048739f9fcb8886e03908c085da08800",
          "message": "Merge pull request #67 from bvolpato/perf/neural-embed-accel\n\nperf: parallelize background neural embedding (no coverage cap) (#66)",
          "timestamp": "2026-05-25T17:57:38-04:00",
          "tree_id": "409bbadcfb676a44b5d3065e405ce0e79b7370da",
          "url": "https://github.com/bvolpato/ivygrep/commit/0eef82d7048739f9fcb8886e03908c085da08800"
        },
        "date": 1779747361613,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 690977.01,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3925.29,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 18304273.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3725.15,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2756.15,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11898.49,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11473.64,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.09,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 676.61,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17112.24,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4208.16,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1494.09,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19776.01,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13944.9,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3188.07,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2468,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471545.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 182.82,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 45148.41,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1666.27,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "184b4bc710338610befa9864de012b8c618f532d",
          "message": "Merge pull request #70 from bvolpato/fix/enhance-rayon-deadlock\n\nfix(enhance): fan out neural embed_batch on std::thread, not rayon (deadlock #69)",
          "timestamp": "2026-05-25T22:11:19-04:00",
          "tree_id": "fe822187f7be21ae5690cc12dfbf41c28a3dfcd9",
          "url": "https://github.com/bvolpato/ivygrep/commit/184b4bc710338610befa9864de012b8c618f532d"
        },
        "date": 1779762563124,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 663348.71,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3807.25,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17005445.37,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3848.4,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2842.73,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11122.27,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10788.4,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.69,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 700.28,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16643.52,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3960.63,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1430.62,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19046.66,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13024.82,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3016.7,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2223.7,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449620.58,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 175.93,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 45221.37,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1511.42,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e9a5a4c55670d6fa66824d0dfe05e8d2f6a085a7",
          "message": "Merge pull request #73 from bvolpato/fix/watcher-cpu-permit\n\nfix(daemon): gate watcher-triggered indexing behind the CPU semaphore (#72 HIGH-2)",
          "timestamp": "2026-05-25T23:54:21-04:00",
          "tree_id": "f9a2359db7da4a42952108b3e7347c60aad133b7",
          "url": "https://github.com/bvolpato/ivygrep/commit/e9a5a4c55670d6fa66824d0dfe05e8d2f6a085a7"
        },
        "date": 1779768734294,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 713027.63,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3985.89,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17303700.32,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3836.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2836.14,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11305.05,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10998.21,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.73,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 689.88,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16745.23,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4024.47,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1419.31,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19280.51,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13646.06,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3060.36,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2236.98,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450546.69,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 176.08,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 48046.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1516.49,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a58d0245819556597bacb40896bd71324f700ddb",
          "message": "Merge pull request #71 from bvolpato/test/stress-large-repo\n\ntest: large-repo stress harness (scripts/stress_large_repo.sh)",
          "timestamp": "2026-05-26T00:21:17-04:00",
          "tree_id": "9bd943ff3f8ecdbbb7e641c4cbca3392ed3221e7",
          "url": "https://github.com/bvolpato/ivygrep/commit/a58d0245819556597bacb40896bd71324f700ddb"
        },
        "date": 1779770305319,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 597791.69,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4015.05,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 18494211.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3756.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2777.67,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 12292.26,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11934.27,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.81,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 677.44,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17863.37,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4279.32,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1504.57,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 20125.27,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 14260.26,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3172.84,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2495.35,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473768.13,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.35,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 45122.24,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1646.48,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e47763fd81f3564717065a7b7f7c75f38dc1cebd",
          "message": "Merge pull request #74 from bvolpato/fix/harness-timeout-validation\n\ntest: validate stress-harness timeout flags are numeric (#71 follow-up)",
          "timestamp": "2026-05-26T00:52:37-04:00",
          "tree_id": "cdf9bba7e004436787dc338cbe27efed20b320c0",
          "url": "https://github.com/bvolpato/ivygrep/commit/e47763fd81f3564717065a7b7f7c75f38dc1cebd"
        },
        "date": 1779772173351,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 603783.5,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3749.66,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 17009662.23,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 3834.69,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 2833.53,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11126.76,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10773.48,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.55,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 691.04,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16432.92,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3955.98,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1422.46,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18934.57,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13172.19,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3019.78,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2215.7,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451098.98,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 172.12,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 43705.88,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1512.84,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "209e9ea3d5380d8e4dc8c47212133b3f42aa187a",
          "message": "Merge pull request #77 from bvolpato/fix/stat-only-stale-index-detection\n\nAccelerate initial indexing and fix restored-mtime stale indexes",
          "timestamp": "2026-05-27T02:37:01-04:00",
          "tree_id": "a42c809c632c144b81ac64b371da0c37960b1c16",
          "url": "https://github.com/bvolpato/ivygrep/commit/209e9ea3d5380d8e4dc8c47212133b3f42aa187a"
        },
        "date": 1779864673894,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 142925.54,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4592.81,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 4351375.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 10.87,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1567.23,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1316.41,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9493.47,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9250.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.89,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 516.02,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 13408.85,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3290.81,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1170.14,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 15039.56,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 9925.71,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2433.52,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1899.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 375014.46,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 142.27,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 382719.39,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 68582.37,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1281.93,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dcc5e4460e255e76564be43467544a89374ad8da",
          "message": "Merge pull request #80 from bvolpato/bvolpato/local-accelerator-backends\n\nfeat(embed): add verified opt-in Metal execution",
          "timestamp": "2026-05-27T03:52:46-04:00",
          "tree_id": "14e16cb2c8324ea3e3948b9e36d0ab5c0fb6d5e0",
          "url": "https://github.com/bvolpato/ivygrep/commit/dcc5e4460e255e76564be43467544a89374ad8da"
        },
        "date": 1779869254402,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 161754.74,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3761.07,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5225744.11,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.37,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2093.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1749.47,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11149.57,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10818.5,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.31,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 733.73,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16363.98,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3909.02,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1429.68,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18839.95,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12743.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3041,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2199.03,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449634.05,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 175.72,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 465223.82,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 39599.78,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1535.18,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f54b796986dcf82d7368fbb4ab7ac184d8fe5b62",
          "message": "Merge pull request #81 from bvolpato/bvolpato/fix-index-producer-lifecycle\n\nfix(indexer): join batch producer on every exit",
          "timestamp": "2026-05-27T04:40:34-04:00",
          "tree_id": "492b678f70fcc5dca706ae3153212da41e35e459",
          "url": "https://github.com/bvolpato/ivygrep/commit/f54b796986dcf82d7368fbb4ab7ac184d8fe5b62"
        },
        "date": 1779872116043,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 164276.19,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3797.77,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5323394.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.18,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2077.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1724.21,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11602.94,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11047.61,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.38,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 725.15,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16312.26,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3875.73,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1434.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18976.66,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12700.05,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3001.86,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2206.67,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449411.25,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 177.08,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 466477.47,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 40089.09,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1549.21,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2affa512882a01c261506e5819ff4929a2a1dfa3",
          "message": "Merge pull request #82 from bvolpato/bvolpato/fix-daemon-search-context-pool\n\nfix(daemon): pool contexts for concurrent searches",
          "timestamp": "2026-05-27T05:13:32-04:00",
          "tree_id": "f5bff4b21ed90ddfa148b3264979afbde1e0c94f",
          "url": "https://github.com/bvolpato/ivygrep/commit/2affa512882a01c261506e5819ff4929a2a1dfa3"
        },
        "date": 1779874106934,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 163689.39,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3835.41,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5389570.15,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.16,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2090.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1745.86,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11572.71,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11254.36,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 703.98,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16823.25,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3965.65,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1437.43,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18725,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12954.24,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2989.27,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2223.43,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450486.23,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 177.71,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 465843.52,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 40585.37,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1596.71,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fd7e0d396fe322b42488cbbb5f8dc43358b1cdf7",
          "message": "Merge pull request #83 from bvolpato/bvolpato/fix-hash-first-neural-upgrade\n\nfix(search): keep first queries on hash-first path",
          "timestamp": "2026-05-27T05:43:18-04:00",
          "tree_id": "58f6e88d25805857e810860753b45b7099f4c694",
          "url": "https://github.com/bvolpato/ivygrep/commit/fd7e0d396fe322b42488cbbb5f8dc43358b1cdf7"
        },
        "date": 1779875871782,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 150410.07,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2790.19,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5362991.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1897.83,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1562.01,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 5460.79,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4876.57,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 865.54,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14012.47,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3139.98,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1192.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 16889.07,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11412.34,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2511.04,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1941.75,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 477806.46,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 119.87,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 500120.34,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 32872.72,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1359.8,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7bb6ddc5cba4ebf501cea369073edc13c499a425",
          "message": "Merge pull request #84 from bvolpato/bvolpato/fix-status-ledger-snapshot\n\nfix(status): reuse job ledger snapshots",
          "timestamp": "2026-05-27T06:05:40-04:00",
          "tree_id": "640138508152f6841378bce019fb171505e47a23",
          "url": "https://github.com/bvolpato/ivygrep/commit/7bb6ddc5cba4ebf501cea369073edc13c499a425"
        },
        "date": 1779877219683,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 166014.84,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3745.98,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5240670.47,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.42,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2068.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1736.01,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10997.23,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10703.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.18,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 716.92,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16207.75,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3880.78,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1416.31,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18526.19,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12780.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2966.46,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2206.12,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449898.53,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 175.07,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 465049.25,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 39599.73,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1521.33,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4284d36f27757840670239d9a02986394f73088b",
          "message": "Merge pull request #85 from bvolpato/bvolpato/optimize-large-hash-tier\n\nperf(index): reduce provisional hash graph build expansion",
          "timestamp": "2026-05-27T07:36:42-04:00",
          "tree_id": "d27d944975710a4c4267cd511be7d3c67693871f",
          "url": "https://github.com/bvolpato/ivygrep/commit/4284d36f27757840670239d9a02986394f73088b"
        },
        "date": 1779882687674,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 151704.56,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3773.69,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5286704.64,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2096.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1742.43,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11660.11,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11151.28,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.41,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 744.42,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16868.39,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3949.91,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1440.77,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19207.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13347.56,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2999.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2216.9,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450178.39,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178.46,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390455,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 41647.21,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1573.47,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "82f8f4f159083b210e396ac0442f2be9c7395b93",
          "message": "Merge pull request #86 from bvolpato/bvolpato/fix-deterministic-literal-large-index\n\nfix(search): make literal limit selection deterministic",
          "timestamp": "2026-05-27T13:15:36-04:00",
          "tree_id": "00f4aed98b7537f3b2ac568cec2d1ea166dc1e00",
          "url": "https://github.com/bvolpato/ivygrep/commit/82f8f4f159083b210e396ac0442f2be9c7395b93"
        },
        "date": 1779903024249,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 139376.82,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3903.96,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5160202.12,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2052.75,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1712.12,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10842.53,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10695.48,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 702.2,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16501.16,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3991.42,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1414.71,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18654.77,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12604.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3020.44,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2206.66,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449169.36,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 175.83,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390654.75,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 41576.18,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1505.3,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "1f6893396e90e2ea7bb656d0bf555568cb3a98d9",
          "message": "Merge pull request #87 from bvolpato/bvolpato/fix-worktree-overlay-transitions\n\nfix(indexer): keep worktree overlays base-relative",
          "timestamp": "2026-05-27T14:25:03-04:00",
          "tree_id": "6210328ddace3d583a7ca36d21467ae225b5c2f7",
          "url": "https://github.com/bvolpato/ivygrep/commit/1f6893396e90e2ea7bb656d0bf555568cb3a98d9"
        },
        "date": 1779907218332,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 138016.51,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3881.48,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5327116.36,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.05,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2024.3,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1713.08,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11887.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11617.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.77,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 702.67,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17762.27,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4317.08,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1503.62,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 20500.55,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 14471.89,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3220.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2458.68,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472728.72,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 184.03,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410017.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 39450.71,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1668.22,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e0a088d79714003430befdb1ea79594de474a816",
          "message": "Merge pull request #88 from bvolpato/bvolpato/fix-worktree-stale-base-delegation\n\nfix(indexer): preserve overlays against stale base index",
          "timestamp": "2026-05-27T15:08:31-04:00",
          "tree_id": "d6337266514e3ba59c3b6b24d439240bded58395",
          "url": "https://github.com/bvolpato/ivygrep/commit/e0a088d79714003430befdb1ea79594de474a816"
        },
        "date": 1779909808623,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 154957.74,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3909.3,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5392043.54,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.23,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2064.56,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1758.89,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11712.13,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11256.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.5,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 701.45,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17235.37,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4152.59,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1458.98,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 20093.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 14173.66,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3108.7,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2270.26,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 452091.93,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 177.33,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 392460.15,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 46267.78,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1674.58,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c7ec3f600202e51fae252fcda8ef5cc1daa5a85c",
          "message": "Merge pull request #89 from bvolpato/bvolpato/add-starlark-tsx-ast-support\n\nfeat(chunking): add Starlark coverage and TSX parsing",
          "timestamp": "2026-05-27T23:03:23-04:00",
          "tree_id": "2c2a660ec43708fc3d1b444b5060e11b56dbc3d0",
          "url": "https://github.com/bvolpato/ivygrep/commit/c7ec3f600202e51fae252fcda8ef5cc1daa5a85c"
        },
        "date": 1779938294016,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 148071.34,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3582.86,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5175241.82,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.2,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2094.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1762.94,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11133.84,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10848.34,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.48,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 726.5,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16394.15,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3961.63,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1430.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18571.31,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12598.24,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2991.5,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2222.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450551.9,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 176.32,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390887.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 39597.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1515.09,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "067a292e9bb093f25ec9a82e9993ea714990405b",
          "message": "feat(chunking): split targets in very large Starlark sources (#90)",
          "timestamp": "2026-05-29T02:44:26-04:00",
          "tree_id": "61604414501f29e18215ac98235799958f4851a4",
          "url": "https://github.com/bvolpato/ivygrep/commit/067a292e9bb093f25ec9a82e9993ea714990405b"
        },
        "date": 1780038391577,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 151811.94,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3778.51,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5233458.75,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.61,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2163.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1774.52,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11152.21,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10840.32,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.29,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 723.71,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16537.12,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4002.34,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1423.74,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18856.64,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13053.9,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3039.68,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2213.87,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450541.37,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 389752.85,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 41584.23,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1550.03,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "937128e26f6fa40ffcf32329332aaf6cd586d639",
          "message": "release: v0.8.0 (#91)",
          "timestamp": "2026-05-30T00:01:51-04:00",
          "tree_id": "5aabae7bbb327cd7211a005843649eaa46fe12da",
          "url": "https://github.com/bvolpato/ivygrep/commit/937128e26f6fa40ffcf32329332aaf6cd586d639"
        },
        "date": 1780114582190,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 140380.59,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3821.52,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5267552.22,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2093.92,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1750.14,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11095.03,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10821.57,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.24,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 728.98,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16584.96,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3979.06,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1432.8,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18772.11,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13086.23,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3011.31,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2216.34,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451374.5,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 176.38,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 389889.17,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 40261.65,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 2125.65,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5a24dfcd4df365be267e85b57305ee08be217471",
          "message": "ci: update release actions to Node 24 (#92)",
          "timestamp": "2026-05-30T00:50:26-04:00",
          "tree_id": "38f1406d0b13efa7e0beb02d64941f46c8cb9d20",
          "url": "https://github.com/bvolpato/ivygrep/commit/5a24dfcd4df365be267e85b57305ee08be217471"
        },
        "date": 1780117897084,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 140353.02,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3932.93,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5248562.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2120.87,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1782.3,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11294.83,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10949.67,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.28,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 734.96,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16778.61,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4005.9,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1437.54,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19026.18,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13211.81,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2996.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2225.25,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450866.67,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 177.86,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390110.5,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 41283.55,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1571.88,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bf55e33037d52d75163f6d565b8e029fa6424d2f",
          "message": "fix(deps): clear rand and lru security advisories (#93)",
          "timestamp": "2026-05-30T01:42:26-04:00",
          "tree_id": "5abd2aeaaadc55dae8eed1909267351ad3ad5735",
          "url": "https://github.com/bvolpato/ivygrep/commit/bf55e33037d52d75163f6d565b8e029fa6424d2f"
        },
        "date": 1780120693054,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 137432.07,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3856.57,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5229733.05,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.3,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2042.71,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1742.56,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11099.77,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10768.89,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.24,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 728.52,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16421.59,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3960.86,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1413.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18944.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13029.82,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3024.44,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2213.77,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449307.85,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 174.27,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 389746.78,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 39603.56,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1517.49,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f2941aa442263bdbdccc19f283e97197f7443530",
          "message": "release: v0.8.1 (#94)",
          "timestamp": "2026-05-30T02:17:14-04:00",
          "tree_id": "1306cfdb8a06f5ddf5727c710df048b9b728acc9",
          "url": "https://github.com/bvolpato/ivygrep/commit/f2941aa442263bdbdccc19f283e97197f7443530"
        },
        "date": 1780122702456,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 135197.81,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3855.75,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5267814.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.03,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2014.16,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1709.47,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11609.35,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11371.92,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 699.98,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17500.74,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4268.03,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1497.03,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19548.03,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13402.12,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3136.26,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2470.06,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471820.35,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 183.39,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409834.18,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 38998.99,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1647.14,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "56045d2cc9a2a4ff10966733ab42ec7045a62468",
          "message": "Merge pull request #95 from bvolpato/bvolpato/fix-aarch64-e2e-tests\n\nci: make aarch64 E2E tests authoritative",
          "timestamp": "2026-05-30T04:27:01-04:00",
          "tree_id": "989ac238e54fbf1e092195f9bd29eaddb94d8255",
          "url": "https://github.com/bvolpato/ivygrep/commit/56045d2cc9a2a4ff10966733ab42ec7045a62468"
        },
        "date": 1780130889379,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 136086.67,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3867.55,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 5300699.25,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2017.03,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1700.11,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11714.55,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11500.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 9.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 674.1,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17050.72,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4265.29,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1499.26,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19588.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13463.07,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3184.94,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2455.3,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472535.01,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 182.34,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410056.44,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 40062.32,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1656.19,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "836bb0683766f843fa6e574882a84901f7e75ae6",
          "message": "perf(indexer): make lexical search ready before ANN enrichment",
          "timestamp": "2026-05-31T09:44:12-04:00",
          "tree_id": "9facdecf85260611f3525a1664ec973830e81efc",
          "url": "https://github.com/bvolpato/ivygrep/commit/836bb0683766f843fa6e574882a84901f7e75ae6"
        },
        "date": 1780235921851,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 76846.81,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3850.67,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 1030283.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.34,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2086.14,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1731.16,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11369.92,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11007.16,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 8.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 597.67,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15028.29,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4057.74,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1433.25,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17952.91,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12202.58,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3016.47,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2221.74,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450812.45,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 177.58,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391542,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 21398.12,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1556.72,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "e533c4a2cb182637c83934f258a16734b77e4c14",
          "message": "release: v0.9.0",
          "timestamp": "2026-05-31T10:00:23-04:00",
          "tree_id": "60e988e6e40ecfa7bc549d59315bf397c9398ea4",
          "url": "https://github.com/bvolpato/ivygrep/commit/e533c4a2cb182637c83934f258a16734b77e4c14"
        },
        "date": 1780236839957,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 65943.28,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3747.38,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 980269.47,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.3,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2059.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1717.87,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10957.28,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10780.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 8.44,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 614.07,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14848.95,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3979.35,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1398.5,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17365.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11504.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2969.87,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2196.83,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 448733.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 175.41,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390250.43,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20065.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1532.29,
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
          "id": "28101a131a2d0b6e1da94dcc22cba79ea553bacb",
          "message": "ci: allow accelerator fallback smoke",
          "timestamp": "2026-05-31T19:05:25-04:00",
          "tree_id": "1ffdd1c2ee1d4d2d7316c467f589abd6629373b2",
          "url": "https://github.com/bvolpato/ivygrep/commit/28101a131a2d0b6e1da94dcc22cba79ea553bacb"
        },
        "date": 1780269678307,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 85338.31,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4528.88,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 1116100,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 11.01,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1612.8,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1334.78,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9368.77,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9201.94,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 6.33,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 437.18,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 11836.8,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3349.51,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1163.47,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 13723.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 9051.36,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2446.86,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1900.87,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 374424.78,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 140.24,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 321693.88,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 41163.22,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1258.19,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "40899b20e093a3526a9b52858f9c913eea824700",
          "message": "ci: make cross-platform stress coverage runnable\n\n- bootstrap required stress fixtures before ignored tests\n- run stress coverage on scheduled and tagged E2E runs",
          "timestamp": "2026-06-04T00:40:13-04:00",
          "tree_id": "2bb5ced2bc6166dddd4850bc606414f5005ffe47",
          "url": "https://github.com/bvolpato/ivygrep/commit/40899b20e093a3526a9b52858f9c913eea824700"
        },
        "date": 1780548828581,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 54507.93,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2742.9,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 871227.45,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1917.53,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1583.71,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 6772.29,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5220.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.45,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 708.87,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 12568.9,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3182.48,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1169.12,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 15217.84,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 10126.16,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2481.91,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1933.1,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473635.48,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 117.87,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 420206.76,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 16765.69,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1380.42,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "4704d385cc36aaac65dc64f65df9faddbf5a38d0",
          "message": "ci: fetch benchmark baseline commit",
          "timestamp": "2026-06-04T00:54:48-04:00",
          "tree_id": "38c1f28dc45fbd8389b60d82c2cb6bbf64e0bd1a",
          "url": "https://github.com/bvolpato/ivygrep/commit/4704d385cc36aaac65dc64f65df9faddbf5a38d0"
        },
        "date": 1780549673378,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 61991.29,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3899.11,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 883427.54,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.06,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2051.14,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1710.96,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11942.01,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11573.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 8.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 565.31,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15480.14,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4338.75,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1505.17,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17968.62,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11801.26,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3136.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2455.49,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472542.35,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.3,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410092.4,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18706.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1640.71,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "a94b5dd1c26c3906a5335a7b9028bf79c74434f3",
          "message": "release: v0.9.2\n\n- document CI reliability fixes\n- bump package and lockfile version",
          "timestamp": "2026-06-04T02:18:06-04:00",
          "tree_id": "7fd3cf58ca9ee890202aaacf19c6c9832cce9b5d",
          "url": "https://github.com/bvolpato/ivygrep/commit/a94b5dd1c26c3906a5335a7b9028bf79c74434f3"
        },
        "date": 1780554714782,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 61913.1,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3963.61,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 904110.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.01,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2043.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1734.01,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11549.64,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11311.59,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 8.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 609.83,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15522.21,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4266.67,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1495.64,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 17811.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11587.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3115.88,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2444.24,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471944.51,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 185.72,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409833.45,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18112.93,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1642.63,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "5d3aac32b3e9aa30a70aab9f1371fae7bc227b4a",
          "message": "fix: harden index health and release gates",
          "timestamp": "2026-06-04T22:12:29-04:00",
          "tree_id": "e64e39ed9086b67105b6dfebf86e6cbfa690bc77",
          "url": "https://github.com/bvolpato/ivygrep/commit/5d3aac32b3e9aa30a70aab9f1371fae7bc227b4a"
        },
        "date": 1780626934855,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 73824.09,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3772.64,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 1004154.76,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.61,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2110.87,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1796.23,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11397.33,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11061.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.41,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 730.18,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16685.15,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4011.32,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1416.2,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19065.78,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13219.05,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3020.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2216.17,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449631.69,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 184.25,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390397.22,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20801.64,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1572.67,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.74,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "c72e9487a716fc7a35daf06af8bdb84d907cc835",
          "message": "fix: harden search hot paths and daemon IPC",
          "timestamp": "2026-06-05T01:18:28-04:00",
          "tree_id": "98c8bef082374198d55e7b27ed0a8dcb580a4e1d",
          "url": "https://github.com/bvolpato/ivygrep/commit/c72e9487a716fc7a35daf06af8bdb84d907cc835"
        },
        "date": 1780637598652,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 75832.24,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3883.57,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 979671.64,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2107.67,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1822.07,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10969.78,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10762.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.28,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 674.92,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16231.6,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3949.18,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1397.8,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 18983.18,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12622.98,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 2956.56,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2218.05,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449987.23,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 183.17,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390637.67,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 22604.84,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1613.48,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.27,
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
          "id": "cb7cb9c5138f02db8984a025564ff029bdb2ba37",
          "message": "Speed up local query preflight",
          "timestamp": "2026-06-07T01:27:13-04:00",
          "tree_id": "78e6e3046b122c133d53ad20bfefcfc1f575e312",
          "url": "https://github.com/bvolpato/ivygrep/commit/cb7cb9c5138f02db8984a025564ff029bdb2ba37"
        },
        "date": 1780811444824,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 69708.24,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3858.35,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 878789.81,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.04,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2038.15,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1735.53,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11555.63,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11473.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 727.1,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17022.57,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4257.62,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1483.67,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19219.59,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12912.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3122.69,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2441.63,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471386.68,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 184.37,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409688.32,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18893.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1642.14,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.46,
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
          "id": "4e0044c98363f60055737b2836af4228e7d24419",
          "message": "docs: refresh site release and deploy guidance",
          "timestamp": "2026-06-11T01:29:22-04:00",
          "tree_id": "d831b5fcaa32bab6ad20ef80a7e738ac36c6c671",
          "url": "https://github.com/bvolpato/ivygrep/commit/4e0044c98363f60055737b2836af4228e7d24419"
        },
        "date": 1781156567845,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 67349.58,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3917.39,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 1018780.99,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.65,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2101.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1781.76,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11566.48,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11209.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.53,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 680.72,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16503.2,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4163.63,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1409.07,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19529.78,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13617.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3031.01,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2208.06,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451499.16,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.52,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391448.86,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 21314.33,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1579.27,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.14,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "3791addea21e8dc314a3119c8147fb845eb2fbe4",
          "message": "ci: fix security runner tooling",
          "timestamp": "2026-06-12T22:26:23-04:00",
          "tree_id": "92523a0654cd7f4af55e968613b766821fc2bd6e",
          "url": "https://github.com/bvolpato/ivygrep/commit/3791addea21e8dc314a3119c8147fb845eb2fbe4"
        },
        "date": 1781318549963,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 60729.6,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3854.45,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 864843.05,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2004.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1708.21,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11486.07,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11389.64,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 698.6,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16812.26,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4224.22,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1477.61,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 19365,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13276.31,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 3133.77,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2445.4,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472549.8,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 188.5,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409568.23,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18512.67,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1648.44,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.72,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "e3e1882a3accc9f4d872543e9f0f037f7b4cbb82",
          "message": "docs: align performance claims with fresh results",
          "timestamp": "2026-06-12T23:47:05-04:00",
          "tree_id": "ff62d574ed9c93bfee5de56dc7bbb1546d24aed4",
          "url": "https://github.com/bvolpato/ivygrep/commit/e3e1882a3accc9f4d872543e9f0f037f7b4cbb82"
        },
        "date": 1781323242790,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 60183.1,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3969.75,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 852852.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.98,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2029.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1704.4,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11593.74,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11362.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 649.79,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17100.77,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4245.98,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1473.23,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 14290.72,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13447.43,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1755.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2429.35,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472600.4,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 187.38,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409814.86,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18262.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1642.93,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.55,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "6e545f834657116ab0f94acd35960680e0d18f5f",
          "message": "ci: extend macOS Intel E2E timeout",
          "timestamp": "2026-06-13T00:44:36-04:00",
          "tree_id": "6ef6c3b4c1fb7cbb717368913df9c8fcecfc9f3b",
          "url": "https://github.com/bvolpato/ivygrep/commit/6e545f834657116ab0f94acd35960680e0d18f5f"
        },
        "date": 1781326676615,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 65165.82,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3730.07,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 966276.26,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.41,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2094.3,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1737.05,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11099.68,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10767.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.46,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 713,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16715.28,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3992.51,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1398.32,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 13917.12,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 13037.25,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1673.26,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2191.16,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450791.81,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.2,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390787.27,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20515.26,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1520.4,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.29,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "bruno.volpato@datadoghq.com",
            "name": "bruno.volpato",
            "username": "bvolpato-dd"
          },
          "committer": {
            "email": "bruno.volpato@datadoghq.com",
            "name": "bruno.volpato",
            "username": "bvolpato-dd"
          },
          "distinct": true,
          "id": "d0a43adacbec67f843c9bd61894267120887346d",
          "message": "[indexer] harden large-repo vector enhancement",
          "timestamp": "2026-06-14T15:58:06Z",
          "tree_id": "074653e76e1e13abbd5ceaeb3e7ee7d20be172f3",
          "url": "https://github.com/bvolpato/ivygrep/commit/d0a43adacbec67f843c9bd61894267120887346d"
        },
        "date": 1781454048574,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 75353.14,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3926.19,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 976599.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.45,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2128.34,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1768.62,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11050.73,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10804.73,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.3,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 719.36,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 16799.29,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4077.52,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1398.33,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 13954.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12749.07,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1723.89,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2201.2,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449818.15,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 184.03,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390084.5,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20473.5,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1562.72,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.78,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "bruno.volpato@datadoghq.com",
            "name": "bruno.volpato",
            "username": "bvolpato-dd"
          },
          "committer": {
            "email": "bruno.volpato@datadoghq.com",
            "name": "bruno.volpato",
            "username": "bvolpato-dd"
          },
          "distinct": true,
          "id": "60c69560d841e031fa1f53e84fe4af61b423b991",
          "message": "release: v0.9.7",
          "timestamp": "2026-06-14T22:38:59Z",
          "tree_id": "2893ea7d061f53bfa13d9fbeadd3747cfdb1dea6",
          "url": "https://github.com/bvolpato/ivygrep/commit/60c69560d841e031fa1f53e84fe4af61b423b991"
        },
        "date": 1781477580428,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 61061.34,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3852.28,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 873297.37,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.07,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2038.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1708.86,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11421.42,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11403.29,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.12,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 720.59,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 17149.1,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4245.63,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1486.59,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 14375.34,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 12947.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1870.47,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2474.74,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 472707.81,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 187.08,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409074.31,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18558.22,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1629.43,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 16.54,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9fe3915eaf66e4bfbce6f841ac59329a7a03a9a2",
          "message": "feat: establish portable search foundation (#112)\n\n* feat: establish portable search foundation\n\n* fix: make portable search paths platform-neutral\n\n* test: normalize Windows CLI assertions\n\n* fix: normalize worktree source line endings\n\n* fix: honor symbol filters and benchmark cold path\n\n* test: canonicalize macOS temporary roots",
          "timestamp": "2026-06-15T04:55:39-04:00",
          "tree_id": "7392eb27a4fcbae8a00f17ed59e35c37e8c35e32",
          "url": "https://github.com/bvolpato/ivygrep/commit/9fe3915eaf66e4bfbce6f841ac59329a7a03a9a2"
        },
        "date": 1781515616458,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 66183.68,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3941.7,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 1109508.07,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1997.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1670.25,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10897.08,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10591.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 714.88,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14606.43,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3937.96,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1371.66,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 12204.74,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11067.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 10330.37,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1647.73,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2177.72,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450839.66,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.59,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390147.96,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19028.57,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 371.31,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1211.51,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.59,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7457803479158234fb4ffceb51693466cf3839bd",
          "message": "fix: restore baseline Linux release compatibility (#114)\n\n* fix: restore baseline Linux release compatibility\n\n* fix: remove obsolete release flag plumbing",
          "timestamp": "2026-06-15T06:23:25-04:00",
          "tree_id": "88c76a1f85f600f14eb495511c675050023f87e5",
          "url": "https://github.com/bvolpato/ivygrep/commit/7457803479158234fb4ffceb51693466cf3839bd"
        },
        "date": 1781520074681,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 67847.13,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3902.35,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 1161354.84,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1984.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1698.66,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11233.96,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10835.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 674.57,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14629.49,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3966.31,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1400.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 12265.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11139.11,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 10351.77,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1669.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2187.28,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450893.54,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.37,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390819.01,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20383.58,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 374.18,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1190.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.73,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "49b1571de77ab096512469e324f76a48e4257123",
          "message": "ci: install Python for aarch64 E2E checks (#115)\n\nCo-authored-by: bruno.volpato <bruno.volpato@datadoghq.com>",
          "timestamp": "2026-06-15T09:54:15-04:00",
          "tree_id": "27b30ee7c0446078a81b2873374305e24edb39ae",
          "url": "https://github.com/bvolpato/ivygrep/commit/49b1571de77ab096512469e324f76a48e4257123"
        },
        "date": 1781532615029,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 69594.17,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3909.67,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 1171043.28,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1999.71,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1684,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11283.68,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10818.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 703.74,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14514.35,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3920.3,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1401.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 12260.12,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 11158.39,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 10428.89,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1664.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2216.37,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450335.5,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 182.91,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391932.03,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19931.1,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 372.95,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1194.47,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.71,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "14efc6643aadaf5114768d575610399ee48d5085",
          "message": "feat: complete the evidence-backed search program (#129)\n\n* feat(benchmarks): establish public retrieval matrix\n\n* feat(search): select portable embeddings and route queries\n\n- persist complete model identities and default to static retrieval\n- route query intents with adaptive public-benchmark candidate budgets\n- publish reproducible model-screening and routing metrics\n\n* fix(benchmarks): enforce cached profile query minimums\n\n* feat(benchmarks): bind retrieval claims to evidence\n\n* ci(benchmarks): retain neural evidence artifacts\n\n* docs(benchmarks): publish portable retrieval evidence\n\n* feat(benchmarks): train a public linear reranker\n\n* fix(benchmarks): terminate query option parsing\n\n* feat(benchmarks): add held-out reranker splits\n\n* fix(benchmarks): bound held-out reranker corpora\n\n* feat(benchmarks): strengthen public reranker features\n\n* feat(search): ship a bounded public learned reranker\n\n* docs(benchmarks): publish learned reranker evidence\n\n* perf(search): bound million-scale indexing and queries\n\n* fix(indexing): decouple sqlite checkpoints from tantivy\n\n* perf(search): reuse daemon sessions at million scale\n\n* chore(benchmarks): record million-run provenance\n\n* perf(search): gate million-scale regressions\n\n* docs(benchmarks): publish million-scale evidence\n\n* perf(index): compact million-scale storage\n\n* ci(release): gate published portability artifacts\n\nVerify checksums, provenance, SBOMs, CPU baselines, offline fallback, and exact archive procedures before creating a release.\n\n* build(release): pin artifact provenance inputs\n\nPin cross to an exact revision and retain cargo flags separately from features in every release provenance sidecar.\n\n* docs(benchmarks): publish compact-index evidence\n\nRetain the full public matrix, per-dataset quality variance, source-commit provenance, and million-scale footprint, RSS, disk, and latency gates.\n\n* feat(benchmarks): enforce evidence-backed claims\n\nGenerate comparable metric histories, immutable source links, release size history, explicit unavailable comparisons, and README/site claim checks.\n\n* docs(benchmarks): publish evidence dashboard\n\nRender immutable benchmark and release histories, visible unavailable comparisons, and the mechanically evaluated claim policy.\n\n* chore(release): prepare v0.11.0\n\nDocument the retrieval, compact-index, evidence, and exact-artifact release work and bump the crate version.\n\n* ci: align pinned action annotations\n\nMatch Rust cache version comments to the pinned upstream tag so the repository-wide workflow security audit is clean.\n\n* fix(indexing): reject incompatible index formats\n\n* fix(benchmarks): retain evidence publication commits\n\n* test(windows): use configured neural dimensions\n\n* fix(search): reject neural stores without identity\n\n* fix: address release review findings\n\n* fix(symbols): preserve unbounded call-site searches\n\n* fix(benchmarks): include daemon RSS in evidence\n\n---------\n\nCo-authored-by: bruno.volpato <bruno.volpato@datadoghq.com>",
          "timestamp": "2026-06-16T02:16:40-04:00",
          "tree_id": "a88de51c8437c1566ec531f4988375eb3fdf9db6",
          "url": "https://github.com/bvolpato/ivygrep/commit/14efc6643aadaf5114768d575610399ee48d5085"
        },
        "date": 1781591798411,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51689.53,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3899.47,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 533229.68,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.51,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1958.91,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1663.12,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11314.16,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11063.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.81,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 705.76,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15400.86,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4059.62,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1401.14,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8491.03,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6777.08,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5137.67,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1669.14,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2400.52,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471319.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 187.59,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409596.17,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17317.32,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 371.73,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1217.64,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.31,
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
          "id": "703ce3edb427f415f9963d0caf27270559d89509",
          "message": "docs(evidence): refresh release workflow digest",
          "timestamp": "2026-06-16T07:13:48Z",
          "tree_id": "b188c3ee8696fe08816d8e57668416e443425864",
          "url": "https://github.com/bvolpato/ivygrep/commit/703ce3edb427f415f9963d0caf27270559d89509"
        },
        "date": 1781595014168,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 55492.17,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3695.68,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 569306.41,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1999.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1661.86,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10981.17,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10635.8,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.19,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 733.86,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14762.09,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3831.06,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1317.97,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8392.61,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6605.01,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4950.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1631.76,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2147.98,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 448725.65,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.31,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391270.51,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18748.45,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 362.55,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1202.18,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.87,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cb5639b1d4a2ca94e256643bcdc561b801c888b8",
          "message": "fix(indexer): keep worktree overlays delta-only (#132)\n\n* fix(indexer): keep worktree overlays delta-only\n\n* test(e2e): initialize scoped fixture as git repo\n\n* fix(indexer): reject dirty base reuse\n\n* perf(workspace): remove overlay hot-path git probes\n\n* fix(indexer): reconcile sparse checkout bases",
          "timestamp": "2026-06-16T06:23:08-04:00",
          "tree_id": "bdf6ec64d8cdbb678bacd7c424f6529266420f2f",
          "url": "https://github.com/bvolpato/ivygrep/commit/cb5639b1d4a2ca94e256643bcdc561b801c888b8"
        },
        "date": 1781606947862,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 52790.81,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3974.6,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47198.81,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 541550.83,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1960.68,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1664.72,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11683.43,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11555.07,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.74,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 687.87,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15740.68,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4172.39,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1424.21,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8453.49,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 7075.89,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5139.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1698.29,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2397.39,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470156.28,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 186.11,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410106.59,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17703.74,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 379.54,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1222.16,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.45,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "84c79dd90fcde16431122edba7340b73205aaced",
          "message": "chore(release): prepare v0.11.1",
          "timestamp": "2026-06-16T10:26:40Z",
          "tree_id": "c039816c674713748a14731e0526894ed1df0024",
          "url": "https://github.com/bvolpato/ivygrep/commit/84c79dd90fcde16431122edba7340b73205aaced"
        },
        "date": 1781606994687,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51773.54,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3959.42,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46309.98,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 537663.46,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1957.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1660.36,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11536.63,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11381.47,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.89,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 698.8,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15432.12,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4094.46,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1404.04,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8759.04,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 7118.52,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5181.21,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1746.03,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2399.66,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471028.5,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 187.99,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410232.32,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18390.62,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 375.97,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1226.52,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.27,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "56c98c70dc780a428ad7b6e9854cc77128d12130",
          "message": "fix(release): repair arm64 archive acceptance",
          "timestamp": "2026-06-16T11:02:14Z",
          "tree_id": "5e5fae6fcf9d2e43849e2051ece0b5401aced494",
          "url": "https://github.com/bvolpato/ivygrep/commit/56c98c70dc780a428ad7b6e9854cc77128d12130"
        },
        "date": 1781608845616,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 50375.8,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3923.62,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45708.45,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 532186.13,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1973.31,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1675.46,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11512.79,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11329.46,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 9.96,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 663.56,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15169.67,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4072.14,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1386,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8613.28,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 7018.91,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5097.55,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1733.28,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2396.48,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471122.49,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 183.56,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409613.76,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 16911.63,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 375.89,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1215.36,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.27,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "9f34b9948fa38f1c8e1964b8f01f13c88c80cfbd",
          "message": "test(ci): stabilize release verification",
          "timestamp": "2026-06-16T11:16:50Z",
          "tree_id": "c93af999f92f9abc3e2d7bcf6ff6b1a4f8f0daa3",
          "url": "https://github.com/bvolpato/ivygrep/commit/9f34b9948fa38f1c8e1964b8f01f13c88c80cfbd"
        },
        "date": 1781609613014,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 56882.32,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3883.2,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 49482.22,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 590889.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2013.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1672.1,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11576.4,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10876.54,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.3,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 677.75,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14947.23,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3925.63,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1351.01,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8346.31,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 7016.6,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4930.72,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1630.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2194.11,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450580.81,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.16,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 392015.17,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20207.74,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 363.08,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1204.72,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.87,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "033ebedf4c9b1e24287648c5570e8ddf04b2501b",
          "message": "docs: show current release version",
          "timestamp": "2026-06-16T11:53:09Z",
          "tree_id": "12b5c7099ff52a0e2e836d863feb82e936ddaf54",
          "url": "https://github.com/bvolpato/ivygrep/commit/033ebedf4c9b1e24287648c5570e8ddf04b2501b"
        },
        "date": 1781611778995,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51266.09,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3903.27,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45885.84,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 532304.71,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.51,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1963.42,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1672.8,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11474.79,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11319.33,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.84,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 704.22,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15167.36,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4051.41,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1397.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8646.16,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 7104.15,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5122.19,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1692.67,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2384.26,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470274.96,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 187.85,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409781.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17213.31,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 375.94,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1218.63,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.27,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "1396a6eebe544deebf2cc82180ee25353c98d6d3",
          "message": "chore(release): prepare v0.12.0",
          "timestamp": "2026-06-17T06:57:14Z",
          "tree_id": "a01616540b7b5c16aa8ae9fdad4db4547a260444",
          "url": "https://github.com/bvolpato/ivygrep/commit/1396a6eebe544deebf2cc82180ee25353c98d6d3"
        },
        "date": 1781680745593,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 45971.7,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3349.99,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 41600.91,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 453219.61,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 10.45,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1512.86,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1299.64,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9465.2,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9104.38,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 8.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 552.24,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 11777.97,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3171.29,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1094.13,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 6577.99,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5258.45,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3943.21,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1319.41,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1861.55,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 376718.32,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 146.38,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 321893.43,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 16494.87,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 283.23,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 945.82,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 30.89,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c354eef4dfaa9aee9587a5ac755f4de1fd1addb8",
          "message": "Merge pull request #136 from bvolpato/bvolpato/mcp-agent-install\n\nfix(mcp): keep coding-agent indexes fresh",
          "timestamp": "2026-06-17T16:41:08-04:00",
          "tree_id": "a1fd5bf6a08f9ac8bb615c3bf01cc9be19f2b8f4",
          "url": "https://github.com/bvolpato/ivygrep/commit/c354eef4dfaa9aee9587a5ac755f4de1fd1addb8"
        },
        "date": 1781730125763,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 53832.37,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3684.77,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 44941.69,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 560979.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.64,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2012.37,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1682.34,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10928.92,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10691.81,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.17,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 727.8,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14834.22,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3818.89,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1317.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8229.29,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6621.29,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4959.25,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1597.2,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2155.24,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449184.39,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178.75,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390126.28,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17669.68,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 364.94,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1141.08,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.84,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fd7b05fdd2974a64ad90f34590d51dccad87108e",
          "message": "Merge pull request #137 from bvolpato/bvolpato/release-0.12.1\n\nchore(release): prepare v0.12.1",
          "timestamp": "2026-06-17T20:26:02-04:00",
          "tree_id": "f78295feaf6bb9e91f660cd5c11e0303160ca0e2",
          "url": "https://github.com/bvolpato/ivygrep/commit/fd7b05fdd2974a64ad90f34590d51dccad87108e"
        },
        "date": 1781743608256,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 59633.7,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3875.56,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 49495.87,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 564132.49,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.39,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1973.84,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1675.12,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10870.89,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10678.31,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.16,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 712.01,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14689.91,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3823.25,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1327.53,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8240,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6519.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4874.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1610.41,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2178.92,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449102.84,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178.87,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390737.45,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20916.78,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 360.68,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1135.61,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.91,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "171db66555f74c90aa556bf98c61cfe056a51d00",
          "message": "Merge pull request #139 from bvolpato/bvolpato/fix-arm64-e2e-git\n\nfix(ci): provide git in ARM64 smoke test",
          "timestamp": "2026-06-17T21:13:45-04:00",
          "tree_id": "63ddd20016e16bbd0e4c5dcc0da2fe51365b068a",
          "url": "https://github.com/bvolpato/ivygrep/commit/171db66555f74c90aa556bf98c61cfe056a51d00"
        },
        "date": 1781746467865,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 50669.28,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3825.3,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46384.65,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 528609.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.33,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1959.88,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1655.9,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11286.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11089.36,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 698.73,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15224.03,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4032.04,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1384.04,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8513.7,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6747.25,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5087.68,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1657.36,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2373.75,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470231.89,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 187.1,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409915.55,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17356.94,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 368.6,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1208.02,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.73,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "701b1247d38f7eff1fd81ce3bd01218e62d316c9",
          "message": "Merge pull request #140 from bvolpato/bvolpato/release-0.12.2\n\nchore(release): prepare v0.12.2",
          "timestamp": "2026-06-17T21:43:04-04:00",
          "tree_id": "f3ad226b3f98959be1d247a21e693c40cc520e39",
          "url": "https://github.com/bvolpato/ivygrep/commit/701b1247d38f7eff1fd81ce3bd01218e62d316c9"
        },
        "date": 1781748230861,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 57894.7,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3847.96,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 48693.7,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 599885.61,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.47,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2016.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1699.17,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11419.42,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11216.27,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 679.47,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14853.88,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3911.63,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1351.05,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8266.65,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6787.23,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4929.87,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1615.95,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2206.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449211.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.86,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390002.32,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19761.98,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 369.55,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1159.75,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 38.15,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ea10399bdecbe228382e06b734cca80fc7fa3ad3",
          "message": "Merge pull request #142 from bvolpato/bvolpato/legacy-daemon-status\n\nfix(daemon): restart stale processes before requests",
          "timestamp": "2026-06-17T22:50:03-04:00",
          "tree_id": "fad430b749172854c7cb80c6063c51752f1adce2",
          "url": "https://github.com/bvolpato/ivygrep/commit/ea10399bdecbe228382e06b734cca80fc7fa3ad3"
        },
        "date": 1781752249891,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 50702.51,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3931.19,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45801.71,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 538683.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.46,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1967.98,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1677.46,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11664.71,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11422.73,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.87,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 670.15,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 15220.41,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 4098.23,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1406.17,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8552.36,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6795.39,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5148.82,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1700.18,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2385.04,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471429.28,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 189,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410053.57,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 16978.5,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 377.94,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1240.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.23,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dfcbffe3b28847748c2003a0b612eabdaaaaf7cb",
          "message": "Merge pull request #144 from bvolpato/bvolpato/windows-tantivy-write-retry\n\nfix(indexer): retry transient Windows segment writes",
          "timestamp": "2026-06-18T00:03:25-04:00",
          "tree_id": "aaf5e256988ce2c85bb76187d4e052b41a8d9464",
          "url": "https://github.com/bvolpato/ivygrep/commit/dfcbffe3b28847748c2003a0b612eabdaaaaf7cb"
        },
        "date": 1781756637907,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 55147.54,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3607.62,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46110.53,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 573290.44,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2012.4,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1668.99,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11011.36,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10699.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.85,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 708.55,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14647.8,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3799.41,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1318.55,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8240.34,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6513.68,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4879.44,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1594.88,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2142.51,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449061.97,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178.49,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390313.34,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18036.95,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 364.15,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1136.63,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.68,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b4df67316b4a8c7fab9c96de8798af06da3d681a",
          "message": "Merge pull request #146 from bvolpato/bvolpato/neural-e2e-download-retry\n\nfix(ci): retry transient neural model downloads",
          "timestamp": "2026-06-18T01:11:19-04:00",
          "tree_id": "be79870aacdf84d1854a26a32205d0c01051710c",
          "url": "https://github.com/bvolpato/ivygrep/commit/b4df67316b4a8c7fab9c96de8798af06da3d681a"
        },
        "date": 1781760715518,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 56098.31,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3700.43,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46774.43,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 570611.03,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2002.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1656.52,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11114.02,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10794.46,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.29,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 712.64,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14734.76,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3863.12,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1323.19,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8267.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6579.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4907.64,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1631.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2148.02,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 448768.01,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.28,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 389440.08,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19138.25,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 369.33,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1140.3,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.55,
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
          "id": "71a614e3fd32e2255f82a2ebe453e3be69f751f3",
          "message": "chore(release): prepare v0.12.6",
          "timestamp": "2026-06-18T19:21:46-04:00",
          "tree_id": "48ccc85531ecfaf18bd9db6edb798ad8be38a4c7",
          "url": "https://github.com/bvolpato/ivygrep/commit/71a614e3fd32e2255f82a2ebe453e3be69f751f3"
        },
        "date": 1781826167012,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 46256.77,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2812.59,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 40419.18,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 553811.11,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.05,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1834.05,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1529.65,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 6874.48,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5727.25,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.67,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 842.83,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 13009.09,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3075.32,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1093.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 7681.19,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6246.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4381.91,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1322.41,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1858.8,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 474910.46,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 120.2,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 419165.44,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17096.35,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 215.2,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1029.47,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 40.17,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "76263eface108ade7b8702e165fdcc9bc3f01108",
          "message": "chore(deps): bump the github-actions group across 1 directory with 3 updates (#147)\n\n* chore(deps): bump the github-actions group across 1 directory with 3 updates\n\nBumps the github-actions group with 3 updates in the / directory: [actions/checkout](https://github.com/actions/checkout), [anchore/sbom-action](https://github.com/anchore/sbom-action) and [actions/download-artifact](https://github.com/actions/download-artifact).\n\n\nUpdates `actions/checkout` from 6.0.3 to 7.0.0\n- [Release notes](https://github.com/actions/checkout/releases)\n- [Changelog](https://github.com/actions/checkout/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/actions/checkout/compare/df4cb1c069e1874edd31b4311f1884172cec0e10...9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0)\n\nUpdates `anchore/sbom-action` from 0.23.0 to 0.24.0\n- [Release notes](https://github.com/anchore/sbom-action/releases)\n- [Changelog](https://github.com/anchore/sbom-action/blob/main/RELEASE.md)\n- [Commits](https://github.com/anchore/sbom-action/compare/17ae1740179002c89186b61233e0f892c3118b11...e22c389904149dbc22b58101806040fa8d37a610)\n\nUpdates `actions/download-artifact` from 7.0.0 to 8.0.1\n- [Release notes](https://github.com/actions/download-artifact/releases)\n- [Commits](https://github.com/actions/download-artifact/compare/37930b1c2abaa49bbe596cd826c3c89aef350131...3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c)\n\n---\nupdated-dependencies:\n- dependency-name: actions/checkout\n  dependency-version: 7.0.0\n  dependency-type: direct:production\n  update-type: version-update:semver-major\n  dependency-group: github-actions\n- dependency-name: anchore/sbom-action\n  dependency-version: 0.24.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: github-actions\n- dependency-name: actions/download-artifact\n  dependency-version: 8.0.1\n  dependency-type: direct:production\n  update-type: version-update:semver-major\n  dependency-group: github-actions\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* fix(ci): update million benchmark daemon protocol\n\n---------\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>\nCo-authored-by: bvolpato <brunocvcunha@gmail.com>",
          "timestamp": "2026-06-19T13:34:27-04:00",
          "tree_id": "2398b799c7697c6f4bae11ce50780be850fabd19",
          "url": "https://github.com/bvolpato/ivygrep/commit/76263eface108ade7b8702e165fdcc9bc3f01108"
        },
        "date": 1781891998298,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 56568.13,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3781,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47416.35,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 582804.07,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.69,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1992.46,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1676.38,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11295.22,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10828.01,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.31,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 674.15,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14865.63,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3868.7,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1331.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8208.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6856.89,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4956.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1564.47,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2167.89,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450896.28,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.95,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391256.31,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19389.67,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 367.37,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1148.15,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.97,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c21a223cb2dc0b9bbf2f4067f2b73893ee849948",
          "message": "chore(deps): bump the cargo group across 1 directory with 24 updates (#131)\n\n* chore(deps): bump the cargo group across 1 directory with 24 updates\n\nBumps the cargo group with 24 updates in the / directory:\n\n| Package | From | To |\n| --- | --- | --- |\n| [clap](https://github.com/clap-rs/clap) | `4.6.0` | `4.6.1` |\n| [regex](https://github.com/rust-lang/regex) | `1.12.3` | `1.12.4` |\n| [ignore](https://github.com/BurntSushi/ripgrep) | `0.4.25` | `0.4.26` |\n| [rayon](https://github.com/rayon-rs/rayon) | `1.11.0` | `1.12.0` |\n| [rusqlite](https://github.com/rusqlite/rusqlite) | `0.39.0` | `0.40.1` |\n| [serde_json](https://github.com/serde-rs/json) | `1.0.149` | `1.0.150` |\n| [tantivy](https://github.com/quickwit-oss/tantivy) | `0.26.0` | `0.26.1` |\n| [tokio](https://github.com/tokio-rs/tokio) | `1.51.0` | `1.52.3` |\n| [uuid](https://github.com/uuid-rs/uuid) | `1.23.0` | `1.23.3` |\n| [ratatui](https://github.com/ratatui/ratatui) | `0.30.0` | `0.30.1` |\n| [libc](https://github.com/rust-lang/libc) | `0.2.184` | `0.2.186` |\n| [tree-sitter](https://github.com/tree-sitter/tree-sitter) | `0.26.8` | `0.26.9` |\n| [tree-sitter-c-sharp](https://github.com/tree-sitter/tree-sitter-c-sharp) | `0.23.1` | `0.23.5` |\n| [tree-sitter-php](https://github.com/tree-sitter/tree-sitter-php) | `0.23.11` | `0.24.2` |\n| [tree-sitter-swift](https://github.com/alex-pinkus/tree-sitter-swift) | `0.7.1` | `0.7.3` |\n| [tree-sitter-c](https://github.com/tree-sitter/tree-sitter-c) | `0.24.1` | `0.24.2` |\n| [tree-sitter-scala](https://github.com/tree-sitter/tree-sitter-scala) | `0.25.0` | `0.26.0` |\n| [tree-sitter-ocaml](https://github.com/tree-sitter/tree-sitter-ocaml) | `0.24.2` | `0.25.0` |\n| [tree-sitter-dart](https://github.com/nielsenko/tree-sitter-dart) | `0.1.0` | `0.2.0` |\n| [tokenizers](https://github.com/huggingface/tokenizers) | `0.19.1` | `0.22.2` |\n| [openssl](https://github.com/rust-openssl/rust-openssl) | `0.10.80` | `0.10.81` |\n| [assert_cmd](https://github.com/assert-rs/assert_cmd) | `2.2.0` | `2.2.2` |\n| [insta](https://github.com/mitsuhiko/insta) | `1.47.2` | `1.48.0` |\n| [serial_test](https://github.com/palfrey/serial_test) | `3.4.0` | `3.5.0` |\n\n\n\nUpdates `clap` from 4.6.0 to 4.6.1\n- [Release notes](https://github.com/clap-rs/clap/releases)\n- [Changelog](https://github.com/clap-rs/clap/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/clap-rs/clap/compare/clap_complete-v4.6.0...clap_complete-v4.6.1)\n\nUpdates `regex` from 1.12.3 to 1.12.4\n- [Release notes](https://github.com/rust-lang/regex/releases)\n- [Changelog](https://github.com/rust-lang/regex/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/rust-lang/regex/compare/1.12.3...1.12.4)\n\nUpdates `ignore` from 0.4.25 to 0.4.26\n- [Release notes](https://github.com/BurntSushi/ripgrep/releases)\n- [Changelog](https://github.com/BurntSushi/ripgrep/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/BurntSushi/ripgrep/compare/ignore-0.4.25...ignore-0.4.26)\n\nUpdates `rayon` from 1.11.0 to 1.12.0\n- [Changelog](https://github.com/rayon-rs/rayon/blob/main/RELEASES.md)\n- [Commits](https://github.com/rayon-rs/rayon/compare/rayon-core-v1.11.0...rayon-core-v1.12.0)\n\nUpdates `rusqlite` from 0.39.0 to 0.40.1\n- [Release notes](https://github.com/rusqlite/rusqlite/releases)\n- [Changelog](https://github.com/rusqlite/rusqlite/blob/master/Changelog.md)\n- [Commits](https://github.com/rusqlite/rusqlite/compare/v0.39.0...v0.40.1)\n\nUpdates `serde_json` from 1.0.149 to 1.0.150\n- [Release notes](https://github.com/serde-rs/json/releases)\n- [Commits](https://github.com/serde-rs/json/compare/v1.0.149...v1.0.150)\n\nUpdates `tantivy` from 0.26.0 to 0.26.1\n- [Release notes](https://github.com/quickwit-oss/tantivy/releases)\n- [Changelog](https://github.com/quickwit-oss/tantivy/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/quickwit-oss/tantivy/compare/0.26.0...0.26.1)\n\nUpdates `tokio` from 1.51.0 to 1.52.3\n- [Release notes](https://github.com/tokio-rs/tokio/releases)\n- [Commits](https://github.com/tokio-rs/tokio/compare/tokio-1.51.0...tokio-1.52.3)\n\nUpdates `uuid` from 1.23.0 to 1.23.3\n- [Release notes](https://github.com/uuid-rs/uuid/releases)\n- [Commits](https://github.com/uuid-rs/uuid/compare/v1.23.0...v1.23.3)\n\nUpdates `ratatui` from 0.30.0 to 0.30.1\n- [Release notes](https://github.com/ratatui/ratatui/releases)\n- [Changelog](https://github.com/ratatui/ratatui/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/ratatui/ratatui/compare/ratatui-v0.30.0...ratatui-v0.30.1)\n\nUpdates `libc` from 0.2.184 to 0.2.186\n- [Release notes](https://github.com/rust-lang/libc/releases)\n- [Changelog](https://github.com/rust-lang/libc/blob/0.2.186/CHANGELOG.md)\n- [Commits](https://github.com/rust-lang/libc/compare/0.2.184...0.2.186)\n\nUpdates `tree-sitter` from 0.26.8 to 0.26.9\n- [Release notes](https://github.com/tree-sitter/tree-sitter/releases)\n- [Commits](https://github.com/tree-sitter/tree-sitter/compare/v0.26.8...v0.26.9)\n\nUpdates `tree-sitter-c-sharp` from 0.23.1 to 0.23.5\n- [Release notes](https://github.com/tree-sitter/tree-sitter-c-sharp/releases)\n- [Commits](https://github.com/tree-sitter/tree-sitter-c-sharp/compare/v0.23.1...v0.23.5)\n\nUpdates `tree-sitter-php` from 0.23.11 to 0.24.2\n- [Release notes](https://github.com/tree-sitter/tree-sitter-php/releases)\n- [Commits](https://github.com/tree-sitter/tree-sitter-php/compare/v0.23.11...v0.24.2)\n\nUpdates `tree-sitter-swift` from 0.7.1 to 0.7.3\n- [Release notes](https://github.com/alex-pinkus/tree-sitter-swift/releases)\n- [Commits](https://github.com/alex-pinkus/tree-sitter-swift/compare/0.7.1...0.7.3)\n\nUpdates `tree-sitter-c` from 0.24.1 to 0.24.2\n- [Release notes](https://github.com/tree-sitter/tree-sitter-c/releases)\n- [Commits](https://github.com/tree-sitter/tree-sitter-c/compare/v0.24.1...v0.24.2)\n\nUpdates `tree-sitter-scala` from 0.25.0 to 0.26.0\n- [Release notes](https://github.com/tree-sitter/tree-sitter-scala/releases)\n- [Commits](https://github.com/tree-sitter/tree-sitter-scala/compare/v0.25.0...v0.26.0)\n\nUpdates `tree-sitter-ocaml` from 0.24.2 to 0.25.0\n- [Release notes](https://github.com/tree-sitter/tree-sitter-ocaml/releases)\n- [Commits](https://github.com/tree-sitter/tree-sitter-ocaml/compare/v0.24.2...v0.25.0)\n\nUpdates `tree-sitter-dart` from 0.1.0 to 0.2.0\n- [Changelog](https://github.com/nielsenko/tree-sitter-dart/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/nielsenko/tree-sitter-dart/commits/v0.2.0)\n\nUpdates `tokenizers` from 0.19.1 to 0.22.2\n- [Release notes](https://github.com/huggingface/tokenizers/releases)\n- [Changelog](https://github.com/huggingface/tokenizers/blob/main/RELEASE.md)\n- [Commits](https://github.com/huggingface/tokenizers/compare/v0.19.1...v0.22.2)\n\nUpdates `openssl` from 0.10.80 to 0.10.81\n- [Release notes](https://github.com/rust-openssl/rust-openssl/releases)\n- [Commits](https://github.com/rust-openssl/rust-openssl/compare/openssl-v0.10.80...openssl-v0.10.81)\n\nUpdates `assert_cmd` from 2.2.0 to 2.2.2\n- [Changelog](https://github.com/assert-rs/assert_cmd/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/assert-rs/assert_cmd/compare/v2.2.0...v2.2.2)\n\nUpdates `insta` from 1.47.2 to 1.48.0\n- [Release notes](https://github.com/mitsuhiko/insta/releases)\n- [Changelog](https://github.com/mitsuhiko/insta/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/mitsuhiko/insta/compare/1.47.2...1.48.0)\n\nUpdates `serial_test` from 3.4.0 to 3.5.0\n- [Release notes](https://github.com/palfrey/serial_test/releases)\n- [Commits](https://github.com/palfrey/serial_test/compare/v3.4.0...v3.5.0)\n\n---\nupdated-dependencies:\n- dependency-name: assert_cmd\n  dependency-version: 2.2.2\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: clap\n  dependency-version: 4.6.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: ignore\n  dependency-version: 0.4.26\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: insta\n  dependency-version: 1.48.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: libc\n  dependency-version: 0.2.186\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: openssl\n  dependency-version: 0.10.81\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: ratatui\n  dependency-version: 0.30.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: rayon\n  dependency-version: 1.12.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: regex\n  dependency-version: 1.12.4\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: rusqlite\n  dependency-version: 0.40.1\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: serde_json\n  dependency-version: 1.0.150\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: serial_test\n  dependency-version: 3.5.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: tantivy\n  dependency-version: 0.26.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: tokenizers\n  dependency-version: 0.22.2\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: tokio\n  dependency-version: 1.52.3\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: tree-sitter\n  dependency-version: 0.26.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: tree-sitter-c\n  dependency-version: 0.24.2\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: tree-sitter-c-sharp\n  dependency-version: 0.23.5\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: tree-sitter-dart\n  dependency-version: 0.2.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: tree-sitter-ocaml\n  dependency-version: 0.25.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: tree-sitter-php\n  dependency-version: 0.24.2\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: tree-sitter-scala\n  dependency-version: 0.26.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n- dependency-name: tree-sitter-swift\n  dependency-version: 0.7.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: uuid\n  dependency-version: 1.23.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* fix benchmark daemon protocol version\n\n* fix windows tantivy write retry budget\n\n---------\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>\nCo-authored-by: bvolpato <brunocvcunha@gmail.com>",
          "timestamp": "2026-06-19T14:40:22-04:00",
          "tree_id": "3ed90f3d8681e304dd7cab5d1d05d133c5778825",
          "url": "https://github.com/bvolpato/ivygrep/commit/c21a223cb2dc0b9bbf2f4067f2b73893ee849948"
        },
        "date": 1781895916684,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 56896.96,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3809.22,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 48341.48,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 577609.8,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.55,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2023.47,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1692.78,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11085.65,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10780.28,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.22,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 722.3,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14961.83,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3894.14,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1345.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8271.29,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6970.98,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5012.61,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1615.41,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2158.92,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449856.35,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 182.41,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391900.01,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19967.05,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 370.1,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1183.54,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.78,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a42dfa4ad8df8187ad693f8988c738c5bb039c2d",
          "message": "Merge pull request #150 from bvolpato/bvolpato/sota-search-correctness\n\nfeat: harden search correctness and ranking",
          "timestamp": "2026-06-23T23:28:45-04:00",
          "tree_id": "18f89d1b889e945e35fc604bb3c4580130b01657",
          "url": "https://github.com/bvolpato/ivygrep/commit/a42dfa4ad8df8187ad693f8988c738c5bb039c2d"
        },
        "date": 1782273314185,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 56407.74,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3896.63,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 48704.73,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 607064.96,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2032.33,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1706.2,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11195.84,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10831.72,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.2,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 712.09,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 14856.43,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3842.74,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1327.32,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8430.47,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6930.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 5051.89,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1641.79,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2162.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451286.91,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 182.35,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390698.11,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19181.54,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 366.27,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1183.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.8,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "69607561d36fdefc8c876193011c0e99fc23e678",
          "message": "Merge pull request #151 from bvolpato/bvolpato/search-latency-hillclimb\n\nperf: tighten daemon search routing",
          "timestamp": "2026-06-24T00:27:50-04:00",
          "tree_id": "2aed342fe7444cb7bc66bfc27fcced172ee60c7d",
          "url": "https://github.com/bvolpato/ivygrep/commit/69607561d36fdefc8c876193011c0e99fc23e678"
        },
        "date": 1782276525717,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 55585.79,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3818.09,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45492.04,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 592759.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2023.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1686.46,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11091.07,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10728.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.36,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 680.91,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 12997.58,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3834.33,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1321.86,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 8083.03,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 6306.06,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 4608.56,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1615.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2139.45,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451546.38,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 182.95,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391550.31,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19766.96,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 368.24,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1186.48,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.74,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d59b2614d20cac1a9f21c61a190ee6616d3ed9f1",
          "message": "Merge pull request #152 from bvolpato/bvolpato/search-latency-hillclimb-2\n\nperf: improve hybrid search relevance and tail latency",
          "timestamp": "2026-06-24T16:48:36-04:00",
          "tree_id": "72d8510ee64953a723d1ac31a1c1c33a6bdbf454",
          "url": "https://github.com/bvolpato/ivygrep/commit/d59b2614d20cac1a9f21c61a190ee6616d3ed9f1"
        },
        "date": 1782335405098,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 57942.11,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 4012.01,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47096.38,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 622231.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.53,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2010.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1699.77,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11197.42,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10720.13,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.77,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 700.15,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 6540.45,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3908.52,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1328.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4444.58,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5394.71,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3824.41,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1615.05,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2162.19,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450135.72,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 179.26,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391994.23,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 21026.81,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 362.35,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1204.4,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.71,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ff38d187cb8f5ccf2c73e7b080b212875c206702",
          "message": "Merge pull request #153 from bvolpato/perf/index-search-hillclimb-3\n\nperf: reduce hybrid search and index overhead",
          "timestamp": "2026-06-24T23:19:33-04:00",
          "tree_id": "50ae21ad50e730d636993968706ef3c76a17f340",
          "url": "https://github.com/bvolpato/ivygrep/commit/ff38d187cb8f5ccf2c73e7b080b212875c206702"
        },
        "date": 1782358831094,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51102.8,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3904.79,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 44335,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 550397.34,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 14.1,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1971.77,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1662.78,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11473.67,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11264.89,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.39,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 710.39,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 6247.25,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3909.35,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1414.32,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4266.27,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5746.99,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3937.39,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1691.78,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2465.79,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470769.67,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 188.07,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410165.26,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18271.89,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 370.35,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1237.74,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.12,
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
          "id": "4ae2a08077717c10864f894e9cc78031e92182b7",
          "message": "test: bound local test parallelism",
          "timestamp": "2026-06-25T08:39:20-04:00",
          "tree_id": "e8c87e0ab561a007c1a403c0cecd090cbeca366d",
          "url": "https://github.com/bvolpato/ivygrep/commit/4ae2a08077717c10864f894e9cc78031e92182b7"
        },
        "date": 1782392426902,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 55875.39,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3764.78,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 44550.87,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 597919.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.82,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2011.33,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1730.7,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11013.59,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10805.37,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.86,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 704.46,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 5905.75,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3610.58,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1321.71,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4121.21,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5292.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3605.61,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1615.32,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2159.84,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449759.49,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.62,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 389869.57,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19192.89,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 360.3,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1190.69,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.62,
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
          "id": "f6e5df572d7cc7690e9cc8d1a78c9ded2a0eb1ec",
          "message": "test: silence windows daemon cleanup",
          "timestamp": "2026-06-25T08:49:58-04:00",
          "tree_id": "6a8db6314f0128e9df1f3b80c9c947edc38c4ec0",
          "url": "https://github.com/bvolpato/ivygrep/commit/f6e5df572d7cc7690e9cc8d1a78c9ded2a0eb1ec"
        },
        "date": 1782393071030,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51794.14,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3916.06,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 44785.17,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 564327,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.8,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1995.26,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1671.99,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11528.23,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11372.56,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 697.29,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 6212.53,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3881.85,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1410.27,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4353.11,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5497.07,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3855.72,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1733.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2407.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470911.43,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 189,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410846.28,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18271.91,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 372.4,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1245.77,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.2,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "e27afd5865b4c087b503a8ef5862d9af91e865bf",
          "message": "perf: reduce hybrid search hot-path overhead",
          "timestamp": "2026-06-25T14:25:49Z",
          "tree_id": "8dd8cfb492730d99e2115a088553f6d40ef32938",
          "url": "https://github.com/bvolpato/ivygrep/commit/e27afd5865b4c087b503a8ef5862d9af91e865bf"
        },
        "date": 1782398855743,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51433.18,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3967.48,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 44838.52,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 563800.4,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.56,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1980.99,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1674.55,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11614.8,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11269.62,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 696.79,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 5962.59,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3859.32,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1415.54,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4311.68,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5113.28,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3540.34,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1720.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2382.99,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470418.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 187.6,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410089.73,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17756.95,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 380.16,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1229.57,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 39.23,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7c12f49429e93312c27422e9b715e3d11382fec6",
          "message": "chore(deps): bump the cargo group with 2 updates (#149)\n\nBumps the cargo group with 2 updates: [ratatui](https://github.com/ratatui/ratatui) and [tokenizers](https://github.com/huggingface/tokenizers).\n\n\nUpdates `ratatui` from 0.30.1 to 0.30.2\n- [Release notes](https://github.com/ratatui/ratatui/releases)\n- [Changelog](https://github.com/ratatui/ratatui/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/ratatui/ratatui/compare/ratatui-v0.30.1...ratatui-v0.30.2)\n\nUpdates `tokenizers` from 0.22.2 to 0.23.1\n- [Release notes](https://github.com/huggingface/tokenizers/releases)\n- [Changelog](https://github.com/huggingface/tokenizers/blob/main/RELEASE.md)\n- [Commits](https://github.com/huggingface/tokenizers/compare/v0.22.2...v0.23.1)\n\n---\nupdated-dependencies:\n- dependency-name: ratatui\n  dependency-version: 0.30.2\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: cargo\n- dependency-name: tokenizers\n  dependency-version: 0.23.1\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: cargo\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-25T10:38:02-04:00",
          "tree_id": "ba571097392fc34154a1c55c30737619cc859eaf",
          "url": "https://github.com/bvolpato/ivygrep/commit/7c12f49429e93312c27422e9b715e3d11382fec6"
        },
        "date": 1782399592802,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 56506.46,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3856.12,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46283.42,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 608737.95,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2028.95,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1712.65,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11399.73,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10939.78,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.15,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 717.78,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 5731.22,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3650.86,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1339.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4074.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5139.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3346.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1631.83,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2216.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449889.33,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 179.86,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391045.87,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20714.11,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 365.52,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1166.66,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.98,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "3e11a94c7fabb42fd88870e4556df75425423b76",
          "message": "fix(indexer): retry transient Windows Tantivy write denials",
          "timestamp": "2026-06-25T14:45:09Z",
          "tree_id": "841d8904220f273d00c836c300c2b4af370fb1f6",
          "url": "https://github.com/bvolpato/ivygrep/commit/3e11a94c7fabb42fd88870e4556df75425423b76"
        },
        "date": 1782400371446,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 57129.53,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3851.88,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46211.01,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 608222.09,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2025.12,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1707.13,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11045.38,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10692.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 713.01,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 5703.35,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3612.29,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1309.66,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4095.51,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4960.52,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3304.31,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1644.47,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2167.39,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450565.62,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.84,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391030.65,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20037.71,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 361.59,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1151.4,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.92,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "d95397a22541bd801afc3d7c71ab1619a8699b9b",
          "message": "fix(indexer): retry Tantivy worker I/O failures",
          "timestamp": "2026-06-25T15:07:42Z",
          "tree_id": "73be6312b297699024f538588221702f7f0f56aa",
          "url": "https://github.com/bvolpato/ivygrep/commit/d95397a22541bd801afc3d7c71ab1619a8699b9b"
        },
        "date": 1782401352631,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 43502.36,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2673.56,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 36370.3,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 563515.9,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.06,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1848.07,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1536.25,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 4946.71,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4805.71,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 814.24,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4786.97,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2856.13,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1076.16,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3457.81,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4407.04,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 2770.2,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1351.5,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1870.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470462,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 120.84,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 419977.54,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 16093.19,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 217.6,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1068.59,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 40.46,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ee34902875c6b6329aa959b30934a3216652135b",
          "message": "chore(deps): bump softprops/action-gh-release from 3.0.0 to 3.0.1 in the github-actions group (#148)\n\n* chore(deps): bump softprops/action-gh-release\n\nBumps the github-actions group with 1 update: [softprops/action-gh-release](https://github.com/softprops/action-gh-release).\n\n\nUpdates `softprops/action-gh-release` from 3.0.0 to 3.0.1\n- [Release notes](https://github.com/softprops/action-gh-release/releases)\n- [Changelog](https://github.com/softprops/action-gh-release/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/softprops/action-gh-release/compare/b4309332981a82ec1c5618f44dd2e27cc8bfbfda...718ea10b132b3b2eba29c1007bb80653f286566b)\n\n---\nupdated-dependencies:\n- dependency-name: softprops/action-gh-release\n  dependency-version: 3.0.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: github-actions\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* Update evidence dashboard for release action bump\n\n---------\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>\nCo-authored-by: bvolpato <brunocvcunha@gmail.com>",
          "timestamp": "2026-06-25T11:10:21-04:00",
          "tree_id": "0e1f1b8534e2a69eee0693cab4a016f8e24cfd40",
          "url": "https://github.com/bvolpato/ivygrep/commit/ee34902875c6b6329aa959b30934a3216652135b"
        },
        "date": 1782401556048,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 55292.78,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3751.94,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 44508.05,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 596865.24,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.63,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2014.95,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1685.18,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10982.33,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10636.73,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.26,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 720.35,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 5643.23,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3600.11,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1316.61,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 4030.5,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4942,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3306.61,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1591.83,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2153.78,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451629.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.76,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390265.02,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19430.85,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 360.37,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1196.82,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.64,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e6b262f6e0f337650f4d0acc4febe783a67abd70",
          "message": "Merge pull request #154 from bvolpato/bvolpato/relevance-indexing-safety\n\nperf: improve retrieval relevance and indexing safety",
          "timestamp": "2026-06-26T17:56:21-04:00",
          "tree_id": "5fd58f0e43120e57f7ff9e7f48b105055cb75e41",
          "url": "https://github.com/bvolpato/ivygrep/commit/e6b262f6e0f337650f4d0acc4febe783a67abd70"
        },
        "date": 1782512233402,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51295.38,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3682.28,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45278.53,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 524774.53,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.11,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2004.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1681.49,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10986.06,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10297.64,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 9.79,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 648.62,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4126.4,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2658.62,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1242.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3158.9,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4746.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3162.16,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1566.46,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2095.66,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 417949.87,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 167.54,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 373654.9,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18600.51,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 352.2,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1153.25,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 34.58,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f72cf43e781f0f994457e92d57b8f87f7cd9c8ab",
          "message": "Merge pull request #155 from bvolpato/bvolpato/ast-relationship-relevance\n\nperf: improve structural member retrieval",
          "timestamp": "2026-06-26T23:23:37-04:00",
          "tree_id": "ccba41e7752a5f307bde41e60abbfe42c1309e58",
          "url": "https://github.com/bvolpato/ivygrep/commit/f72cf43e781f0f994457e92d57b8f87f7cd9c8ab"
        },
        "date": 1782531879363,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 43628.65,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2751.77,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 38699.48,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 516752.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1849.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1524.24,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 5125.03,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4942.21,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.63,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 839.34,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 3698.4,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2226.6,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1085.46,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 2784.14,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4471.36,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 2771.57,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1331.63,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1874.19,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 474015.24,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 119.88,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 418985.07,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17181.8,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 219.55,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1038.03,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 38.42,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "gh@brunovolpato.com",
            "name": "Bruno Volpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e2c23509ced8b9a0a19ccb83c16d4138c95c099e",
          "message": "Merge pull request #156 from bvolpato/bvolpato/release-v0.12.10\n\nchore(release): prepare v0.12.10",
          "timestamp": "2026-06-26T23:47:28-04:00",
          "tree_id": "99d82c6e27e7f9bc8077afe2b0582e1714b27452",
          "url": "https://github.com/bvolpato/ivygrep/commit/e2c23509ced8b9a0a19ccb83c16d4138c95c099e"
        },
        "date": 1782533307228,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51584.43,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3940.71,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45777.18,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 548492.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.92,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2047.3,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1686.18,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10961.58,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10681.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.99,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 701.73,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4319.1,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2816.78,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1337.51,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3336.18,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5054.98,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3317.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1608.62,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2148.67,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450102.42,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.2,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390883.88,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19483.9,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 365.41,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1179.6,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.24,
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
          "id": "34d2d1097ce3d9f44ea82bde7702f316cad7a994",
          "message": "perf: batch transformer enhancement on accelerators",
          "timestamp": "2026-06-28T01:00:49-04:00",
          "tree_id": "d6231b3ad18710009728f43d2c478087b335932c",
          "url": "https://github.com/bvolpato/ivygrep/commit/34d2d1097ce3d9f44ea82bde7702f316cad7a994"
        },
        "date": 1782624126561,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 53742.12,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3871.42,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46841.43,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 554436.81,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2032.26,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1696.27,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11128.5,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10840.67,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.17,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 661.55,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4353.45,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2822.79,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1354.32,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3311.67,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5041.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3248.63,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1584.83,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2140.88,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451556.33,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 181.78,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390930.69,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20240.41,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 370.91,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1197.06,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.28,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "committer": {
            "email": "brunocvcunha@gmail.com",
            "name": "bvolpato",
            "username": "bvolpato"
          },
          "distinct": true,
          "id": "8a7478b942b77d2bd0ccdd0862b557f2886634ca",
          "message": "fix(macos): correct available memory calculation",
          "timestamp": "2026-06-28T01:05:18-04:00",
          "tree_id": "0dd3c7de403055719348bab0534c67157958502b",
          "url": "https://github.com/bvolpato/ivygrep/commit/8a7478b942b77d2bd0ccdd0862b557f2886634ca"
        },
        "date": 1782624398345,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 48951.46,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3936.05,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45778.55,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 483406.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.57,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1965.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1671.52,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11496.03,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11291.91,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 9.85,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 656.48,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4556.94,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3052.71,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1403.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3504.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5182.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3417.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1714.6,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2389.84,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470606.53,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 190.9,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410393.98,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18950.85,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 381.07,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1239.22,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.6,
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
          "id": "36eba59ad2cf48bdaf3c274346b72c9a8af78d40",
          "message": "perf: avoid tokenizer clone during embedding",
          "timestamp": "2026-06-28T01:37:25-04:00",
          "tree_id": "002e6c32d20da57353ba5ad055f68b3e38a3fed8",
          "url": "https://github.com/bvolpato/ivygrep/commit/36eba59ad2cf48bdaf3c274346b72c9a8af78d40"
        },
        "date": 1782626470997,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 45031.7,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2773.49,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 39290.87,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 538455.78,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.35,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1869.81,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1532.86,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 5091.22,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4889.21,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.38,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 806.34,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 3735.85,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2233.93,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1089.41,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 2844.02,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4589.81,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 2834.28,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1339.25,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1870.64,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470935.07,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 121.13,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 420781.55,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18037.27,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 223.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1034.09,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 38.4,
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
          "id": "a280c0c936f7b6f6d84b028e290cca8e053162db",
          "message": "perf: warm daemon neural model before search",
          "timestamp": "2026-06-28T02:15:49-04:00",
          "tree_id": "22c2690d08ea96097394831ec239baf529cea888",
          "url": "https://github.com/bvolpato/ivygrep/commit/a280c0c936f7b6f6d84b028e290cca8e053162db"
        },
        "date": 1782628612902,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51744.6,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3777.8,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45568.67,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 542113.79,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.73,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2009.49,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1695.09,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10944.22,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10673.22,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.91,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 675.44,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4278.56,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2784.28,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1308.44,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3282.96,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4855.74,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3215.78,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1554.62,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2147.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450538.05,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 179.82,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390904.95,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19776.22,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 370.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1150.97,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.07,
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
          "id": "4b7cf3c2f9ed080f5be8e6ecaada8617ae4a69b4",
          "message": "release: prepare v0.12.11",
          "timestamp": "2026-06-28T02:44:12-04:00",
          "tree_id": "7d3c261f0ead75a03159c69e55edde03628ac9b0",
          "url": "https://github.com/bvolpato/ivygrep/commit/4b7cf3c2f9ed080f5be8e6ecaada8617ae4a69b4"
        },
        "date": 1782630318374,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 56896.32,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3973.35,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 48203.45,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 528848.23,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.67,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2016.38,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1664.31,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10894.41,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10627.44,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.91,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 717.49,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4318.45,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2808.38,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1322.03,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3282.71,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4858.68,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3215.29,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1587.65,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2151.29,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450478.2,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178.69,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390021.84,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 22798.92,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 367.46,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1139.8,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 35.85,
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
          "id": "acf15c9a4600dbd91fc0281bf1af685ec0ffa298",
          "message": "perf: default foreground transformer search to accelerators",
          "timestamp": "2026-06-28T11:24:26-04:00",
          "tree_id": "51aa291386aa644918ccbab814fa54896d711589",
          "url": "https://github.com/bvolpato/ivygrep/commit/acf15c9a4600dbd91fc0281bf1af685ec0ffa298"
        },
        "date": 1782661840940,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 54297.43,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3906.79,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47911.04,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 550714.66,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2037.75,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1698.3,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11201.36,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10871.03,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.23,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 683.65,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4374.33,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2881.39,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1345.7,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3412.74,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5095.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3277.75,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1650.91,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2187.62,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450727.92,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 183.19,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391764.89,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20685.86,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 376.62,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1156.8,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.25,
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
          "id": "6ccea96f7d76e303b8bc7c9cc7908d554e440397",
          "message": "fix: satisfy foreground accelerator clippy",
          "timestamp": "2026-06-28T11:41:49-04:00",
          "tree_id": "f46b355aaee61d697bb200997753b55c7709c90d",
          "url": "https://github.com/bvolpato/ivygrep/commit/6ccea96f7d76e303b8bc7c9cc7908d554e440397"
        },
        "date": 1782662867749,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 52667.83,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3811.9,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46735.29,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 539365.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2013.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1671.06,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10958.28,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10516.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 725.61,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4370.74,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2865.52,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1315.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3271.14,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4934.11,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3320.37,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1553.07,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2154.76,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451935.09,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 179.17,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390187.5,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20380.95,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 365.93,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1144.24,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36,
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
          "id": "6e9a594549fb2aa79bcb54134c37506702c7c688",
          "message": "perf: prefetch daemon neural query vectors",
          "timestamp": "2026-06-28T13:03:03-04:00",
          "tree_id": "5fea4c05ec044798e3d2b823038caf6eb9dc2cbd",
          "url": "https://github.com/bvolpato/ivygrep/commit/6e9a594549fb2aa79bcb54134c37506702c7c688"
        },
        "date": 1782667323939,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 51659.51,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3524.82,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 52829.85,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 437243.89,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 10.6,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1538.12,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1292.05,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9416.85,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9166.34,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.58,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 502.99,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 3504.17,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2373.99,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1105.88,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 2635.47,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4076.48,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 2724.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1304.71,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1874.34,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 375843.23,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 147.62,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 322145.69,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 26760.1,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 292.66,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 958.81,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 29.52,
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
          "id": "eaefa909d4f058bec6271a6fe7d0d3f98e2f6d7f",
          "message": "perf: skip redundant hash fallback after full neural coverage",
          "timestamp": "2026-06-28T13:31:28-04:00",
          "tree_id": "213b061093b1e8334d629ceed1e35fcf9dfbba0c",
          "url": "https://github.com/bvolpato/ivygrep/commit/eaefa909d4f058bec6271a6fe7d0d3f98e2f6d7f"
        },
        "date": 1782669174509,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 53257.58,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3921.57,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47445.4,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 544262.65,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.99,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2024.68,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1670.09,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11153.18,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10784.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 724.65,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4394.84,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2849.38,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1343.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3422.09,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5098.11,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3339.98,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1629.36,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2169.62,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450936.37,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.31,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391193.81,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20152.5,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 370.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1171.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.43,
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
          "id": "8e68d9bb2694ef5e5ca41665589f1d6aea917228",
          "message": "perf: tune CUDA neural enhancement batches",
          "timestamp": "2026-06-28T14:45:11-04:00",
          "tree_id": "77bc0883832543ff8ae1ab59c058c661562831ae",
          "url": "https://github.com/bvolpato/ivygrep/commit/8e68d9bb2694ef5e5ca41665589f1d6aea917228"
        },
        "date": 1782673593862,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 57822.82,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3981.18,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 49909.28,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 552708.45,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.58,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2024.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1681.33,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10857.67,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11113.06,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.21,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 679.16,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4286.68,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2800.09,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1324.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3405.18,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5381.57,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3291.77,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1580.82,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2139.51,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451926.21,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.51,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390503.14,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 24066.88,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 365.69,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1143.97,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.17,
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
          "id": "d468c185b86b289a25017e211f4ca1611e5b1a6d",
          "message": "perf: trim neural search fusion overhead",
          "timestamp": "2026-06-28T15:03:07-04:00",
          "tree_id": "e5f63e20b6093122a0bc2b7b58cd6d20bd424368",
          "url": "https://github.com/bvolpato/ivygrep/commit/d468c185b86b289a25017e211f4ca1611e5b1a6d"
        },
        "date": 1782674876402,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 55361.93,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3947.91,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 48560.55,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 548633.14,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.85,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2041.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1685.04,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11354.84,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10950.77,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.93,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 723.12,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4363.53,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2881.21,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1351.17,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3429.52,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5069.94,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3331.08,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1654.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2161.65,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451575.31,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 184.39,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391171.76,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20711.03,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 381.42,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1207.46,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.37,
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
          "id": "068d957c49768f486304c6f43159ed1b8a895a61",
          "message": "release: prepare v0.12.12",
          "timestamp": "2026-06-28T15:28:31-04:00",
          "tree_id": "a719980f6d821285a956737d01ca2604d7b1b9a4",
          "url": "https://github.com/bvolpato/ivygrep/commit/068d957c49768f486304c6f43159ed1b8a895a61"
        },
        "date": 1782676197332,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 54074.81,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3844.76,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47227.4,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 534418.62,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2034.08,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1675.67,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11123.88,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10887.68,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 737.92,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4355.08,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2866.46,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1349.03,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3286.97,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5023.78,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3291.85,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1576.81,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2205.94,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451365.57,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 182.5,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390718.89,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19944.7,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 374.23,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1209.9,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.29,
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
          "id": "827cf62df2b38b0f63554df91b944fa31fb94ed4",
          "message": "perf: reuse sqlite hydration query buffers",
          "timestamp": "2026-06-28T17:52:47-04:00",
          "tree_id": "d13f94b7592457e1a7e140ab3fca45ebbe6bf1d0",
          "url": "https://github.com/bvolpato/ivygrep/commit/827cf62df2b38b0f63554df91b944fa31fb94ed4"
        },
        "date": 1782685093650,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 49266.78,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3957.96,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 45986.42,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 487216.36,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.48,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1949.91,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1663.37,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11563.42,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11474.14,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.65,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 702.51,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4575.04,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3103.18,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1413.88,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3556.92,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5079.51,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3532.86,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1699.22,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2374.8,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470389.85,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 188.41,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 409998.17,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18933.21,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 381.32,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1223.09,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.52,
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
          "id": "2d54727bdc0c8a3272e0179c8ea5e856cfc0e41b",
          "message": "perf: speed fresh sqlite indexing",
          "timestamp": "2026-06-28T22:34:07-04:00",
          "tree_id": "b7cbd09599316725c70eb1e78a0d9a4f68b84060",
          "url": "https://github.com/bvolpato/ivygrep/commit/2d54727bdc0c8a3272e0179c8ea5e856cfc0e41b"
        },
        "date": 1782701734500,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 52553.49,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3789.99,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47105.03,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 430991.86,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.68,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2027.27,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1709.21,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 10902.66,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10729.79,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.2,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 700.79,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4312.6,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2845.72,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1321.48,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3274.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4738.65,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3194.25,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1575.38,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2140.53,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449028.15,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178.3,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390504.1,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20114.36,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 365.62,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1173.7,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.09,
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
          "id": "54b7832118e3ef744069e11f552e2e897126d0ca",
          "message": "release: prepare v0.12.13",
          "timestamp": "2026-06-28T23:00:26-04:00",
          "tree_id": "068e7a0d165348f135d659d72399935f64e58756",
          "url": "https://github.com/bvolpato/ivygrep/commit/54b7832118e3ef744069e11f552e2e897126d0ca"
        },
        "date": 1782703304258,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 41754.85,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2758.26,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 37819.26,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 424513.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1861.55,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1531.15,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 5771.57,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 5215.46,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.7,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 822.99,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 3698.97,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2242.34,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1086.99,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 2747.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4418.12,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 2740.51,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1312.76,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1873.32,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 473654.94,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 121.6,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 419227.66,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 16605.63,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 225.91,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1044.47,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 38.44,
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
          "id": "55320be22bf2e08d25e3df928b395a06875eaadf",
          "message": "perf: speed fresh index persistence",
          "timestamp": "2026-06-29T01:21:21-04:00",
          "tree_id": "7f8914114e7f28b746c83f3560d8cd0e99ab1cf1",
          "url": "https://github.com/bvolpato/ivygrep/commit/55320be22bf2e08d25e3df928b395a06875eaadf"
        },
        "date": 1782711752003,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 37554.13,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 2830.19,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 36071.25,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 423277.54,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.17,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1850.84,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1528.56,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 6451.38,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 4810.8,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.87,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 790.58,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 3636.04,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2213.89,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1063.83,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 2765.21,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4349.05,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 2739.37,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1310.35,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1858.86,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470264.81,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 117.6,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 418815.16,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 16250.06,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 223.35,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1048.35,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 38.52,
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
          "id": "b11ab7016a2d493141f85340d91cddfeedd75d65",
          "message": "release: prepare v0.12.14",
          "timestamp": "2026-06-29T07:44:01-04:00",
          "tree_id": "bbb8724df66b8658c96f57a33b8570938d42e5f1",
          "url": "https://github.com/bvolpato/ivygrep/commit/b11ab7016a2d493141f85340d91cddfeedd75d65"
        },
        "date": 1782734727411,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 45869.77,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3926.9,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46709.92,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 403748.59,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1992.5,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1657.88,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11533.02,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11442.82,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.97,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 699.75,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4540.68,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3097.79,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1403.05,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3400.75,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5181.61,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3425.32,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1661.7,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2408.79,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 471114.99,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 189.77,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410348.75,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19433.33,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 380.06,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1256.28,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.63,
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
          "id": "c93fbb9dff2e187491d33c7a0a9ffaa98d9d2218",
          "message": "indexer: avoid random chunk ids",
          "timestamp": "2026-06-29T09:11:44-04:00",
          "tree_id": "21d3e968ce303ca8a3eb2c0e09af7f303aebbd14",
          "url": "https://github.com/bvolpato/ivygrep/commit/c93fbb9dff2e187491d33c7a0a9ffaa98d9d2218"
        },
        "date": 1782739987075,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 49065.59,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3751.47,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 47730.76,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 420449.52,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.96,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1944.19,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1585.66,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11657.59,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11208.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.95,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 699.09,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4326.01,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2842.14,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1351.08,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3453.29,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5136.24,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3457.84,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1637.42,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2192.87,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 451137.28,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 176.6,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 391252.99,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20978.41,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 370.84,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1148.15,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.12,
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
          "id": "23c3b65f580ce4c1c9179068a953a6091b72d2e4",
          "message": "ci: lint portable accelerator features",
          "timestamp": "2026-06-29T09:21:36-04:00",
          "tree_id": "0561de3a05ebe62c262a2021fdcc35269ed780c4",
          "url": "https://github.com/bvolpato/ivygrep/commit/23c3b65f580ce4c1c9179068a953a6091b72d2e4"
        },
        "date": 1782740428661,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 41363.31,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3383.61,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 40428.74,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 310188.33,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 9.93,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1466.27,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1199.5,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 9472.02,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 9150.82,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 7.52,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 507.21,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 3477.44,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2319.78,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1065.98,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 2578.57,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 3794.14,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 2622.4,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1251.72,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 1840.73,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 377344.56,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 144.95,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 322144.27,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 17302.08,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 291.11,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 943.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 29.56,
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
          "id": "b6f73800d26f234293ab9923df617eb944b96e0a",
          "message": "release: prepare v0.12.15",
          "timestamp": "2026-06-29T09:45:45-04:00",
          "tree_id": "97c570a737767547fb8a54e950809ed54120e896",
          "url": "https://github.com/bvolpato/ivygrep/commit/b6f73800d26f234293ab9923df617eb944b96e0a"
        },
        "date": 1782742034856,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 48469.71,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3852.96,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46799.44,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 414203.95,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.17,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 2005.02,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1597.29,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11373.77,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10751.83,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.89,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 716.81,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4298.03,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2847.82,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1348.16,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3258.9,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5024.99,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3235.1,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1553.11,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2155.87,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450522.22,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 180.84,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390190.87,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20732.22,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 369.25,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1156.44,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 35.94,
            "unit": "µs"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c55edcd7ad6305bf1b5ea48022a0d3c6793ea1c2",
          "message": "chore(deps): bump the github-actions group with 2 updates (#157)\n\n* chore(deps): bump the github-actions group with 2 updates\n\nBumps the github-actions group with 2 updates: [actions/setup-python](https://github.com/actions/setup-python) and [actions/attest](https://github.com/actions/attest).\n\n\nUpdates `actions/setup-python` from 6.2.0 to 6.3.0\n- [Release notes](https://github.com/actions/setup-python/releases)\n- [Commits](https://github.com/actions/setup-python/compare/a309ff8b426b58ec0e2a45f0f869d46889d02405...ece7cb06caefa5fff74198d8649806c4678c61a1)\n\nUpdates `actions/attest` from 4.1.0 to 4.1.1\n- [Release notes](https://github.com/actions/attest/releases)\n- [Changelog](https://github.com/actions/attest/blob/main/RELEASE.md)\n- [Commits](https://github.com/actions/attest/compare/59d89421af93a897026c735860bf21b6eb4f7b26...a1948c3f048ba23858d222213b7c278aabede763)\n\n---\nupdated-dependencies:\n- dependency-name: actions/setup-python\n  dependency-version: 6.3.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: github-actions\n- dependency-name: actions/attest\n  dependency-version: 4.1.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: github-actions\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* docs: refresh evidence dashboard hashes\n\n---------\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>\nCo-authored-by: Bruno Volpato <brunocvcunha@gmail.com>",
          "timestamp": "2026-06-29T20:53:29-04:00",
          "tree_id": "4681227a628143131f2bf37627b713d0653dc9d9",
          "url": "https://github.com/bvolpato/ivygrep/commit/c55edcd7ad6305bf1b5ea48022a0d3c6793ea1c2"
        },
        "date": 1782782093780,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 49471.05,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3839.13,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46606.33,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 410218.04,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 13.06,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1970.72,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1561.08,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11155.89,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10774.1,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 10.86,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 718.81,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4306.92,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2837.03,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1338.62,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3289.56,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 5017.34,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3215.19,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1567.3,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2163.49,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 450148.61,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 179.7,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 389936.89,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 20054.87,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 371.31,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1148.99,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.02,
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
          "id": "e5dd87c393228384e8c4c96427a87f4747a85398",
          "message": "perf: slim auxiliary Tantivy fields",
          "timestamp": "2026-06-29T23:11:54-04:00",
          "tree_id": "50fd65c3d95857ebc5395d88eaec22db03e16a68",
          "url": "https://github.com/bvolpato/ivygrep/commit/e5dd87c393228384e8c4c96427a87f4747a85398"
        },
        "date": 1782790418032,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 46774.09,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3822.12,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46305.25,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 409047.38,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.92,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1949.65,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1580.14,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11362.7,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 10803.49,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 11.05,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 678.02,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4157.57,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 2768.47,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1304.93,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3268.11,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 3917.78,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3485.51,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1509.41,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2176.88,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 449686.22,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 178.43,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 390272.73,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 19671.38,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 363.88,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1177.04,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 36.12,
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
          "id": "75ea421469785ad9e569a026e3bbd46ef1386702",
          "message": "security: update anyhow lockfile",
          "timestamp": "2026-06-29T23:20:16-04:00",
          "tree_id": "76cdad95b05b8f34180b83676bb83cb52332f32c",
          "url": "https://github.com/bvolpato/ivygrep/commit/75ea421469785ad9e569a026e3bbd46ef1386702"
        },
        "date": 1782790884135,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "indexer/index_small_workspace",
            "value": 45734.47,
            "unit": "µs"
          },
          {
            "name": "indexer/incremental_reindex_no_change",
            "value": 3958.12,
            "unit": "µs"
          },
          {
            "name": "indexer/worktree_overlay_one_file_delta",
            "value": 46459.84,
            "unit": "µs"
          },
          {
            "name": "indexer_bulk/fresh_index_30k_chunks",
            "value": 387411.97,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_small_file",
            "value": 12.68,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_rust_100_fns",
            "value": 1883.43,
            "unit": "µs"
          },
          {
            "name": "chunking/chunk_python_100_fns",
            "value": 1540.98,
            "unit": "µs"
          },
          {
            "name": "merkle/scan_500_files",
            "value": 11576.39,
            "unit": "µs"
          },
          {
            "name": "merkle/diff_500_files_no_change",
            "value": 11274.83,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_single",
            "value": 9.98,
            "unit": "µs"
          },
          {
            "name": "embedding/hash_embed_batch_100",
            "value": 684.75,
            "unit": "µs"
          },
          {
            "name": "search/hybrid_search_200_files",
            "value": 4438.35,
            "unit": "µs"
          },
          {
            "name": "search/literal_search_200_files",
            "value": 3005.38,
            "unit": "µs"
          },
          {
            "name": "regex_search/regex_200_files",
            "value": 1399.58,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_simple_symbol_1000_files",
            "value": 3456.09,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/hybrid_complex_phrase_1000_files",
            "value": 4103.72,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/bounded_rerank_100_candidates_1000_files",
            "value": 3717.12,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/literal_simple_symbol_1000_files",
            "value": 1589.14,
            "unit": "µs"
          },
          {
            "name": "base_search_patterns/regex_symbol_1000_files",
            "value": 2374.42,
            "unit": "µs"
          },
          {
            "name": "vector_store/upsert_1000_vectors",
            "value": 470004.78,
            "unit": "µs"
          },
          {
            "name": "vector_store/search_in_1000_vectors",
            "value": 188.19,
            "unit": "µs"
          },
          {
            "name": "hash_vector_build/ingest_5k_hash_vectors",
            "value": 410201.73,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/incremental_one_file_change_10k_chunks",
            "value": 18617.89,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/quick_health_cached_10k_chunks",
            "value": 384.62,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k",
            "value": 1223.99,
            "unit": "µs"
          },
          {
            "name": "critical_journeys/vector_search_in_50k_hot",
            "value": 37.48,
            "unit": "µs"
          }
        ]
      }
    ]
  }
}