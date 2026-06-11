# Testing Instructions

## Quick Validation

```bash
./test.sh --quick
```

## Full Validation (with clippy)

```bash
./test.sh
```

Full validation also runs Python harness tests. Use `./test.sh --no-python`
only when iterating on Rust-only changes.

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

## Static Docs

For `README.md`, `docs/**`, or `assets/**` changes:

```bash
rg -n "v0\.9\.0|TODO|FIXME" README.md docs
python3 -m http.server 8765 --directory docs
```

Then open `http://127.0.0.1:8765/` and `http://127.0.0.1:8765/benchmarks/`
with a browser, checking layout, console errors, key links, and benchmark images.
Stop the server before continuing.

## End-to-End Procedures

```bash
./build.sh
./scripts/e2e_procedures.sh --binary ./target/release/ig
python3 scripts/check_daemon_equivalence.py \
  --skip-build \
  --binary ./target/release/ig \
  --bench-home /tmp/ivygrep-daemon-equivalence
```

The procedure smoke covers documented CLI workflows. The equivalence check
compares daemon and local results across hybrid, literal, regex, type, glob,
scope, and multi-workspace searches.

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
