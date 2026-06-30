# Post-Deployment Monitoring

## Verify Docs Site

After docs/site changes:

```bash
gh run list --workflow docs-pages.yml --limit 5
gh run list --limit 10 --json workflowName,headBranch,headSha,status,conclusion,url
curl -fsSL https://bvolpato.github.io/ivygrep/ >/tmp/ivygrep-site.html
curl -fsSL https://bvolpato.github.io/ivygrep/benchmarks/ >/tmp/ivygrep-benchmarks.html
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
rg -n "v${version}|Performance evidence|Daemon hot-query" /tmp/ivygrep-site.html /tmp/ivygrep-benchmarks.html
```

Docs Pages and `pages-build-deployment` should complete successfully. The live
site should show the current release badge and benchmark index.

## Verify Release Pipeline

1. **CI checks pass**: `https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml`
2. **Release workflow completes**: `https://github.com/bvolpato/ivygrep/actions/workflows/release.yml`
3. **GitHub Release published**: `https://github.com/bvolpato/ivygrep/releases/latest`
4. **All 5 platform binaries present**: linux-x86_64-musl, linux-aarch64-musl,
   macos-x86_64, macos-aarch64, windows-x86_64
5. **Supply-chain assets present for every platform**: SHA256 checksum, SPDX
   SBOM, and build provenance

## Verify Homebrew Tap

```bash
brew update
brew tap bvolpato/tap
brew info bvolpato/tap/ivygrep
```

The version should match the newly released tag.

## Smoke Test (after install)

```bash
ig --version
ig --help
ig --add .
ig "test query"
```
