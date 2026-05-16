# Testing Instructions

## Quick Validation

```bash
./test.sh --quick
```

## Full Validation (with clippy)

```bash
./test.sh
```

## Build

```bash
./build.sh
./build.sh --help
```

## Benchmarks

```bash
./bench.sh
./bench.sh --help
```

## Stress Tests (requires fixture download)

```bash
./scripts/bootstrap_stress_fixtures.sh
./test.sh --stress
```

## CI Matrix

CI tests all combinations of `neural` vs `hash-only` mode across:
- Linux (ubuntu-latest)
- macOS ARM (macos-latest)
- macOS Intel (macos-15-intel)
