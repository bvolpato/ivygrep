# Runtime portability and Rust coverage

This change fixes the reproduced ARM64 neural debug-build failure and a
linked-worktree watcher bug during Git metadata directory replacement. It also
adds a repeatable coverage job, installs Git in the QEMU Python E2E container,
and requires actual Metal execution in the Metal CI job.

The [JSON report](runtime-coverage-2026-09-02.json) records source fingerprints,
coverage totals and every measured Rust file, negative-control checksums, and
the limits of the measurements.

## ARM64 debug builds

Unmodified main fails `cargo test --locked --lib --no-run` on this baseline
ARM64 target with 11 `fullfp16` assembler errors. `gemm-f16` 0.19.0 instantiates
FP16 helpers that lack their own target-feature annotation at debug optimization
levels. Release inlining hides the issue.

The workaround sets `opt-level = 1` only for `gemm-f16` in the dev profile,
which the test profile inherits. Application code keeps debug assertions and
debug symbols. There is no global `+fp16` requirement. This is a dependency
workaround to remove once an upstream release fixes the helper annotations.

The complete default-feature debug suite passed: 1,084 tests before the new
F16 test, and 1,085 in the final instrumented run. The new CPU F16 matrix-multiply
test also passed through `qemu-aarch64 -cpu cortex-a53`, checking execution on
an older ARM CPU without FP16 arithmetic. This does not test every ARM CPU.
Native ARM neural CI and the default local test runner now use debug tests;
release builds remain independently tested.

## Worktree metadata replacement

The extended real-watcher test renames and recreates the external Git `info/`
directory three times, requiring each new `exclude` file both to hide and to
restore indexed results. Before the fix, Linux failed the first replacement:
the pathname was unchanged but inotify still watched the old inode.

Unix registrations now track device/inode identity and detach an obsolete
watch before registering the new directory at the same pathname. Windows
keeps a recursive watch on the stable common Git directory so asynchronous
retirement of a disposable `info/` handle cannot remove its replacement watch.
Unrelated Git metadata is still rejected by the existing event filter.

The native regression passes. Windows requires the PR's remote CI result;
the originating failure was in its real linked-worktree watcher test, not a
mocked filesystem. Windows receives more raw Git metadata notifications with
the stable parent watch, although they do not trigger index updates. Unix
keeps the narrower nonrecursive watch and pays an identity stat only when
registering or reconciling it.

## Measured coverage

`cargo-llvm-cov 0.9.0` with Rust 1.96.0 ran all library, binary, and integration
tests with default neural features on Linux ARM64:

| Measure | Covered | Total | Percentage |
| --- | ---: | ---: | ---: |
| Rust source lines | 49,380 | 55,610 | 88.80% |
| Functions | 4,484 | 5,049 | 88.81% |
| Regions | 78,773 | 89,071 | 88.44% |

These totals **include inline test modules**. Integration-test source and vendor
source are excluded. Native C/C++, GPU execution, Python, browser tests,
ignored stress tests, and branch coverage are not measured. Child processes
killed before flushing their profiles can also be undercounted. This number
is not a correctness or relevance guarantee.

The lowest measured modules include web (34.83%), TUI (48.96%), IPC (56.14%),
CLI (65.27%), and embedding (68.90%). The new PR/scheduled/manual workflow
uploads LCOV and per-file JSON so future work can target real gaps; it does
not impose an arbitrary percentage floor or pad the suite to reach one.

The initial build exhausted the memory-backed temporary filesystem before
tests ran. Only the stopped task-owned build artifacts were moved to instance
storage, and the complete rerun passed. No product failure or successful
coverage measurement is inferred from that interrupted build.

```bash
cargo install cargo-llvm-cov --version 0.9.0 --locked
rustup component add llvm-tools-preview
CARGO_TARGET_DIR=/tmp/ivygrep-coverage cargo llvm-cov \
  --locked --lib --bins --tests --json --summary-only \
  --ignore-filename-regex '/(tests|vendor)/' \
  --output-path /tmp/ivygrep-coverage.json
```

Allow at least 20 GiB of build space. This host's full debug and coverage
builds are substantially larger than the release binary.

## Remaining hardware scope

An earlier main CI job executed real Candle Metal successfully; this change
removes the Metal job's accepted CPU fallback so a fresh green result must
prove Metal again. The QEMU/musl workflow tests its own current build, not a
previous release archive. Its Python container needs Git for the real worktree
fixtures; the separate installer smoke container still runs without network.

This Linux host has no CUDA device, and the repository has no registered GPU
runner. CUDA device execution remains unverified. A CPU fallback or successful
CUDA compilation must not be reported as a GPU execution pass.
