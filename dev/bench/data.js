window.BENCHMARK_DATA = {
  "lastUpdate": 1776111156698,
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
      }
    ]
  }
}