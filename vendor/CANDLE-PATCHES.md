# Candle ZIP compatibility forks

`candle-core-0.10.2` and `candle-core-0.11.0` preserve the published crates.io
sources and licenses. Their archive checksums are
`6bd9895436c1ba5dc1037a19935d084b838db066ff4e15ef7dded020b7c12a4a`
and `5ecb245093b0f791b89d3420c3df9c6d49c60ab63ba54db896bf8a3baf486706`,
respectively. Cargo's local registry markers and crate-local lockfiles are
omitted; the repository root lockfile controls both dependency builds.

`candle-nn-0.10.2` and `candle-transformers-0.10.2` preserve their published
sources and licenses. Their archive checksums are
`a9317a09d6530b758990ed7f625ac69ff43653bc9ee28b0464644ad1169ada87`
and `f59d08c89e9f4af9c464e2f3a8e16199e7cc601e6f34538c2cfbb42b623b1783`.
Their local registry markers and lockfiles are also omitted.

`candle-flash-attn-0.10.2` preserves the published sources and license from archive
`12512cf8e706744642e9a8579305a6ed1e44a0c636ce20c416cd5c519de19b7d`.
Only its normalized package identity and core dependency change, keeping the
optional Transformers feature on the same maintained tensor types. Registry
markers and its crate-local lockfile are omitted.

Both manifests use ZIP >=9.0.3 (locked to 9.0.5) instead of the yanked ZIP 7.2.0 and 8.6.0 releases,
which are affected by RUSTSEC-2026-0367, RUSTSEC-2026-0368, and
RUSTSEC-2026-0369. NPZ and PTH filename handling propagates ZIP 9's fallible
filename decoding and accepts borrowed UTF-8 and owned legacy names. The source
adaptation follows upstream PR 4041, commit
`70f72e9cfdbfc570846ca363ead2135394af3e9a`:
<https://github.com/huggingface/candle/pull/4041>.

The normalized manifests rename these packages to `ivygrep-candle-core`,
`ivygrep-candle-nn`, `ivygrep-candle-flash-attn`, and
`ivygrep-candle-transformers`, with the upstream version
followed by `-ivygrep.1`. Library names remain unchanged. NN and Transformers
refer to the same exact maintained core version, and Transformers refers to the
maintained NN version. `ivygrep-candle-embed` 0.1.4-ivygrep.3 uses these versioned
path dependencies. This preserves the dependency graph after crates.io
normalization without relying on root-only Cargo patches. Model implementation
and accelerator features are unchanged. Root ivygrep does not expose the optional
Transformers FlashAttention feature; its GPU execution has not been validated.

The manual publication workflow prepares dependency order and requires separate
publication of these versions before the root crate. See `docs/releasing.md`.
Remove these forks after published Candle versions support ZIP 9 and the
embedding integration can migrate without changing existing model behavior.
