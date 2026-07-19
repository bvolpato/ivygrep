# ivygrep patch

Fork of `candle_embed` 0.1.4 used by ivygrep transformer profiles.

Changes upgrade Candle, add Metal selection and device reporting, batch inference,
and shared immutable model tensors for bounded parallel workers. Package name is
distinct so crates.io installs reproduce release behavior without relying on a
consumer-side Cargo patch.

Upstream draft: [shelbyJenkins/candle_embed#2](https://github.com/ShelbyJenkins/candle_embed/pull/2).
