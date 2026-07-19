# ivygrep patch

Fork of `hf-hub` 0.3.2 used by ivygrep model downloads.

Relative redirect locations are resolved against configured endpoint. Package
name is distinct so crates.io installs reproduce release behavior without a
consumer-side Cargo patch.

Equivalent handling on current upstream API is tracked by
[huggingface/hf-hub#176](https://github.com/huggingface/hf-hub/pull/176).
