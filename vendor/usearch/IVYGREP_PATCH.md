# ivygrep patch

Fork of `usearch` 2.24.0 used by ivygrep vector indexes.

Changes disable bundled SimSIMD, preserve Cargo-selected Windows CRT linkage,
fix Windows allocation failure checks, prevent reserve-time capacity inflation,
keep reserve operations monotonic, and use renamed CXX bridge include. Package
name is distinct so crates.io installs reproduce release behavior without a
consumer-side Cargo patch.

Upstream drafts cover reserve sizing in
[unum-cloud/USearch#777](https://github.com/unum-cloud/USearch/pull/777) and
Cargo-selected Windows CRT linkage in
[unum-cloud/USearch#778](https://github.com/unum-cloud/USearch/pull/778).
