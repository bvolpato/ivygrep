# Testing Instructions

## Quick Validation

```bash
cargo test -- --test-threads=1
```

## Full Validation (with clippy)

```bash
cargo clippy --all-targets
cargo test -- --test-threads=1
```

## Stress Tests (requires fixture download)

```bash
./scripts/bootstrap_stress_fixtures.sh
cargo test --test stress_harness -- --ignored --nocapture --test-threads=1
```

## CI Matrix

CI tests all combinations of `neural` vs `hash-only` mode across:
- Linux (ubuntu-latest)
- macOS ARM (macos-latest)
- macOS Intel (macos-15-intel)
