# Post-Deployment Monitoring

## Verify Release Pipeline

1. **CI checks pass**: `https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml`
2. **Release workflow completes**: `https://github.com/bvolpato/ivygrep/actions/workflows/release.yml`
3. **GitHub Release published**: `https://github.com/bvolpato/ivygrep/releases/latest`
4. **All 4 platform binaries present**: linux-x86_64-musl, linux-aarch64-musl, macos-x86_64, macos-aarch64

## Verify Homebrew Tap

```bash
brew update
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
