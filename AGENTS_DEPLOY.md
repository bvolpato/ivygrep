# Deployment Instructions

## Release Process

ivygrep releases are driven by Git tags. To deploy a new version:

1. **Bump version** in `Cargo.toml`
2. **Update** `CHANGELOG.md` with the new version entry
3. **Commit** all changes
4. **Tag** the release: `git tag v<VERSION>`
5. **Push** with tags: `git push && git push --tags`

The `release.yml` GitHub Actions workflow will automatically:
- Build binaries for Linux (x86_64 musl, aarch64 musl), macOS (x86_64, aarch64)
- Create a GitHub Release with the binaries and SHA256 checksums
- Update the Homebrew tap at `bvolpato/homebrew-tap`

## Verify Release

After pushing the tag, check:
- GitHub Actions: `https://github.com/bvolpato/ivygrep/actions/workflows/release.yml`
- GitHub Releases: `https://github.com/bvolpato/ivygrep/releases`
