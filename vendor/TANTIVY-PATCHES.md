# Tantivy Zstandard compatibility forks

`tantivy`, `tantivy-columnar`, and `tantivy-sstable` preserve the library sources from upstream commit `5ca39332002c2c87fb5d2abc707cf527b3319d42` and its MIT license. The source files are unchanged. The root Tantivy copy omits upstream workspace, example, benchmark, and integration-test targets that are not part of its library.

The manifests require Zstandard 0.14.3 or newer instead of 0.13. This follows the dependency update in upstream commit `dd0bc2336080ccb5b078fab6e8e21f99eca56fbd`, with a patched version floor for RUSTSEC-2026-0391. Zstandard 0.14.3 requires zstd-safe 8.2.0, which fixes the affected streaming and seekable APIs. Default compression features remain enabled so existing compressed column dictionaries remain readable.

The package names are `ivygrep-tantivy` 0.27.0-ivygrep.1, `ivygrep-tantivy-columnar` 0.7.0-ivygrep.1, and `ivygrep-tantivy-sstable` 0.7.0-ivygrep.1. Library names remain unchanged. Exact versioned path dependencies preserve the maintained graph after crates.io normalization without root-only Cargo patches. Other Tantivy component dependencies retain the original upstream revision and published version constraints.

Publish the SSTable fork, then the columnar fork, then Tantivy before publishing the root ivygrep crate. Binary archive builds use these sources directly and do not publish the forks. Remove the compatibility forks after an upstream release can supply the patched dependency graph without unrelated search or index changes.
