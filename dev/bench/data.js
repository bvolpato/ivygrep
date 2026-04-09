window.BENCHMARK_DATA = {
  "lastUpdate": 1775694061398,
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
      }
    ]
  }
}