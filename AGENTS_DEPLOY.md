# Deployment Instructions

## Docs Site

Docs deploy from `main` through `.github/workflows/docs-pages.yml` whenever
`assets/**`, `docs/**`, or the workflow itself changes. The workflow syncs
`docs/` plus `assets/` to `gh-pages` and keeps `dev/bench/` benchmark history.

After pushing docs changes, verify:
- Docs Pages workflow: `https://github.com/bvolpato/ivygrep/actions/workflows/docs-pages.yml`
- Pages deployment: `https://github.com/bvolpato/ivygrep/deployments/github-pages`
- Live site: `https://bvolpato.github.io/ivygrep/`

## Release Process

ivygrep releases are driven by Git tags. To deploy a new version:

1. **Bump version** in `Cargo.toml`
2. **Update** `CHANGELOG.md` with the new version entry
3. **Commit** all changes
4. **Tag** the release: `git tag v<VERSION>`
5. **Push** with tags: `git push && git push --tags`

The `release.yml` GitHub Actions workflow will automatically:
- Build binaries for Linux (x86_64 musl, aarch64 musl), macOS (x86_64, aarch64),
  and Windows (x86_64)
- Create a GitHub Release with the binaries, SHA256 checksums, SPDX SBOMs, and
  build provenance
- Update the Homebrew tap at `bvolpato/homebrew-tap`

## Verify Release

After pushing the tag, check:
- GitHub Actions: `https://github.com/bvolpato/ivygrep/actions/workflows/release.yml`
- GitHub Releases: `https://github.com/bvolpato/ivygrep/releases`
