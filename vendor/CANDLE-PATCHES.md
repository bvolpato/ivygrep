# Candle ZIP compatibility forks

`candle-core-0.10.2` and `candle-core-0.11.0` preserve the published crates.io
sources and licenses. Their archive checksums are
`6bd9895436c1ba5dc1037a19935d084b838db066ff4e15ef7dded020b7c12a4a`
and `5ecb245093b0f791b89d3420c3df9c6d49c60ab63ba54db896bf8a3baf486706`,
respectively. Cargo's local registry markers and crate-local lockfiles are
omitted; the repository root lockfile controls both dependency builds.

`candle-nn-0.10.2` preserves the published sources and license metadata from
archive `a9317a09d6530b758990ed7f625ac69ff43653bc9ee28b0464644ad1169ada87`.
Its normalized manifest uses the maintained core. Registry markers and the
crate-local lockfile are omitted.

Both manifests use ZIP >=9.0.3 (locked to 9.0.5) instead of the yanked ZIP 7.2.0 and 8.6.0 releases,
which are affected by RUSTSEC-2026-0367, RUSTSEC-2026-0368, and
RUSTSEC-2026-0369. NPZ and PTH filename handling propagates ZIP 9's fallible
filename decoding and accepts borrowed UTF-8 and owned legacy names. The source
adaptation follows upstream PR 4041, commit
`70f72e9cfdbfc570846ca363ead2135394af3e9a`:
<https://github.com/huggingface/candle/pull/4041>.

The normalized manifests rename these packages to `ivygrep-candle-core` and
`ivygrep-candle-nn`, with the upstream version followed by `-ivygrep.1`. Library
names remain unchanged. NN and `ivygrep-candle-embed` 0.1.4-ivygrep.3 use the same
exact maintained core version. The embedder includes private copies of the BERT
implementation and its helpers, described in `candle_embed/CANDLE-SOURCES.md`;
unrelated Transformers models and FlashAttention are not dependencies. The
public embedder API, model mathematics, and CUDA/Metal forwarding are unchanged.
Versioned path dependencies preserve the graph after crates.io normalization
without relying on root-only Cargo patches.

The manual publication workflow prepares dependency order and requires separate
publication of these versions before the root crate. See `docs/releasing.md`.
Remove these forks after published Candle versions support ZIP 9 and the
embedding integration can migrate without changing existing model behavior.
