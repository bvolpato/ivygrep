window.BENCHMARK_DATA = {
  "lastUpdate": 1775432077246,
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
      }
    ]
  }
}