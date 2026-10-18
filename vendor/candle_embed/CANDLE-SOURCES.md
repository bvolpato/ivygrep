# Candle BERT sources

The embedder uses only BERT from Candle Transformers. These private modules copy
the corresponding files from `candle-transformers` 0.10.2, whose crates.io archive
SHA-256 is `f59d08c89e9f4af9c464e2f3a8e16199e7cc601e6f34538c2cfbb42b623b1783`:

| Local module | Upstream file |
| --- | --- |
| `src/bert.rs` | `src/models/bert.rs` |
| `src/bert_tracing.rs` | `src/models/with_tracing.rs` |
| `src/quantized_var_builder.rs` | `src/quantized_var_builder.rs` |

Module bodies are unchanged except that BERT imports `super::bert_tracing`
instead of `super::with_tracing`. The public embedder API and model computations
remain unchanged. CUDA and Metal still flow through the same Candle core and NN
features. No unused Transformers models or optional FlashAttention dependency
are included.

The upstream source revision is
`7c7a8c570e6a16ea24cc30a7501c8ffbbdb51680`. Its MIT and Apache-2.0 license texts
are preserved in `CANDLE-LICENSE-MIT` and `CANDLE-LICENSE-APACHE`; the copied
modules are used under the MIT option, matching this crate's existing license.
The original embedder license remains in `LICENSE`.
