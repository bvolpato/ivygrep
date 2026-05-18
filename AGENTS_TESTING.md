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

`./bench.sh` uses a temporary Criterion baseline by default so smoke runs do not report stale `target/criterion` regressions. Use `./bench.sh --keep-baseline` only when comparing against local Criterion history.

Benchmark output reports per-operation latency. Microbenchmarks run repeated logical operations so actual timed samples last long enough to be stable.

## End-to-End Procedures

```bash
./build.sh
./scripts/e2e_procedures.sh --binary ./target/release/ig
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
