# Candle ZIP compatibility patches

`candle-core-0.10.2` and `candle-core-0.11.0` preserve the published crates.io
sources and licenses. Their archive checksums are
`6bd9895436c1ba5dc1037a19935d084b838db066ff4e15ef7dded020b7c12a4a`
and `5ecb245093b0f791b89d3420c3df9c6d49c60ab63ba54db896bf8a3baf486706`,
respectively. Cargo's local registry markers and crate-local lockfiles are
omitted; the repository root lockfile controls both dependency builds.

Both manifests use ZIP >=9.0.3 (locked to 9.0.5) instead of the yanked ZIP 7.2.0 and 8.6.0 releases,
which are affected by RUSTSEC-2026-0367, RUSTSEC-2026-0368, and
RUSTSEC-2026-0369. NPZ and PTH filename handling propagates ZIP 9's fallible
filename decoding and accepts borrowed UTF-8 and owned legacy names. The source
adaptation follows upstream PR 4041, commit
`70f72e9cfdbfc570846ca363ead2135394af3e9a`:
<https://github.com/huggingface/candle/pull/4041>.

Remove these patches after published Candle versions support ZIP 9 and the
embedding integration can migrate without changing existing model behavior.
