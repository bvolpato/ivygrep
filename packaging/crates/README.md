# crates.io publication

ivygrep depends on four behavior-critical forks. Their package names are
distinct so `cargo install ivygrep --locked` does not depend on local
`[patch.crates-io]` configuration.

Publication order:

1. `ivygrep-hf-hub`
2. `ivygrep-candle-embed`
3. `ivygrep-usearch`
4. `ivygrep-tree-sitter-haskell`
5. `ivygrep`

First publication requires crates.io token because trusted publishing cannot
create a crate. Add token as repository Actions secret `CRATES_IO_TOKEN`, then
run `Publish crates.io packages` with `bootstrap=true`.

After bootstrap, configure trusted publisher for each crate with:

- GitHub owner: `bvolpato`
- repository: `ivygrep`
- workflow: `publish-crates.yml`
- environment: `release`

Delete bootstrap secret after trusted publishing works. Future runs use GitHub
OIDC with `bootstrap=false`.
