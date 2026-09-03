# ivygrep patch

Fork of `usearch` 2.24.0 used by ivygrep vector indexes.

Changes disable bundled SimSIMD, preserve Cargo-selected Windows CRT linkage,
fix Windows allocation failure checks, prevent reserve-time capacity inflation,
keep reserve operations monotonic, and use renamed CXX bridge include. Package
name is distinct so crates.io installs reproduce release behavior without a
consumer-side Cargo patch.

The Rust/CXX bridge also exposes `inspect_serialized_header`. It interprets
header fields and graph layout sizes using the pinned native definitions,
including the existing pre-2.10 scalar-type conversion. ivygrep checks matrix
dimensions and population fields, then streams the node-level table through an
8 KiB buffer to validate levels and serialized-length bounds. Count and
availability queries can read the header's live count without constructing a
native view or allocating its per-node lookup arrays and key table.

This metadata path does not validate full vector or node payload integrity.
Search and deep health checks still open the native store. The serialized
USearch format is unchanged.

On AArch64 GCC/Clang builds, half-to-single conversion uses the compiler's
`__fp16` conversion instead of the software bit-manipulation path. This does not
enable optional FP16 arithmetic or change the stored representation. ivygrep's
vector-store tests round-trip every finite half value, including signed zeros
and subnormals. Other architectures keep the existing portable conversion.

Upstream drafts cover reserve sizing in
[unum-cloud/USearch#777](https://github.com/unum-cloud/USearch/pull/777) and
Cargo-selected Windows CRT linkage in
[unum-cloud/USearch#778](https://github.com/unum-cloud/USearch/pull/778).
