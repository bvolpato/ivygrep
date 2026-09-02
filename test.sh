#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

usage() {
  cat <<'EOF'
Run ivygrep validation.

Usage:
  ./test.sh [options] [-- cargo-test-args...]

Options:
  --ci                  Run CI-like local checks (default)
  --quick               Run only library tests, skip fmt/clippy
  --all-targets         Compile/test all Cargo targets
  --stress              Run ignored stress tests (requires fixtures)
  --e2e                 Run local CLI, daemon, and browser E2E after checks
  --release             Compile Clippy/tests with the release profile
  --hash-only           Test without default neural feature
  --features FEATURES   Pass explicit Cargo features
  --no-fmt              Skip cargo fmt -- --check
  --no-shellcheck       Skip ShellCheck
  --no-clippy           Skip cargo clippy
  --no-web              Skip pnpm web frontend checks
  --no-python           Skip Python harness tests
  --filter NAME         Pass Cargo test filter
  --nocapture           Pass --nocapture to test harness
  --test-threads N      Pass --test-threads to test harness
  -h, --help            Show help

Examples:
  ./test.sh
  ./test.sh --quick
  ./test.sh --release
  ./test.sh --hash-only
  ./test.sh --e2e
  ./test.sh --features cuda
  ./test.sh --filter query_aliases --nocapture
  ./scripts/bootstrap_stress_fixtures.sh && ./test.sh --stress
EOF
  cat <<'EOF'

Environment:
  CARGO_BUILD_JOBS=N                 Override local Cargo build parallelism.
  RUST_TEST_THREADS=N                Override local Rust test parallelism.
  IVYGREP_UNBOUNDED_LOCAL_TESTS=1    Keep Cargo/Rust default parallelism outside CI.
EOF
  echo
  echo "Available sessions:"
  echo "  ./build.sh      Build ivygrep binary with selected profile/features."
  echo "  ./test.sh       Run ivygrep validation suite."
  echo "  ./bench.sh      Run performance benchmarks and benchmark guards."
}

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  "$@"
}

web_dist_fingerprint() {
  find web/dist -type f -print0 | LC_ALL=C sort -z |
    while IFS= read -r -d '' path; do
      printf '%s  %s\n' "$(git hash-object -- "$path")" "$path"
    done
}

mode="ci"
do_fmt=1
do_shellcheck=1
do_clippy=1
do_web=1
do_python_tests=1
cargo_flags=(--locked)
profile_flags=()
scope_flags=(--lib --bins --tests)
filter=()
test_args=()
extra_args=()
wants_cuda=0
force_release=0
run_e2e=0

features_include_cuda() {
  local features=$1
  [[ ",$features," == *",cuda,"* ]]
}

configure_cuda_compute_cap() {
  ((wants_cuda)) || return 0
  [[ -z "${CUDA_COMPUTE_CAP:-}" ]] || return 0

  local cap=""
  if command -v nvidia-smi >/dev/null 2>&1; then
    cap="$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader,nounits 2>/dev/null | head -n1 | tr -d '[:space:].')" || cap=""
  fi

  if [[ -n "$cap" ]]; then
    export CUDA_COMPUTE_CAP="$cap"
    echo "CUDA_COMPUTE_CAP=$CUDA_COMPUTE_CAP (from nvidia-smi)"
    return 0
  fi

  local pci_devices=""
  if command -v lspci >/dev/null 2>&1; then
    pci_devices="$(lspci 2>/dev/null || true)"
  fi
  if grep -Eiq 'NVIDIA.*(GB20|RTX 50|Blackwell)' <<<"$pci_devices"; then
    export CUDA_COMPUTE_CAP=120
    echo "CUDA_COMPUTE_CAP=120 (inferred from NVIDIA Blackwell GPU)"
  fi
}

configure_build_profile() {
  if ((force_release)); then
    profile_flags=(--release)
  fi
}

configure_local_resource_limits() {
  [[ "${CI:-}" == "true" ]] && return 0
  [[ "${IVYGREP_UNBOUNDED_LOCAL_TESTS:-}" == "1" ]] && return 0

  local cpus jobs
  cpus="$(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || printf '1\n')"
  if ! [[ "$cpus" =~ ^[0-9]+$ ]] || ((cpus < 1)); then
    cpus=1
  fi

  jobs="$cpus"
  if ((jobs > 4)); then
    jobs=4
  fi

  if [[ -z "${CARGO_BUILD_JOBS:-}" ]]; then
    export CARGO_BUILD_JOBS="$jobs"
    echo "CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS (local default; set CARGO_BUILD_JOBS or IVYGREP_UNBOUNDED_LOCAL_TESTS=1 to override)"
  fi

  if [[ -z "${RUST_TEST_THREADS:-}" ]] && ((${#test_args[@]} == 0)); then
    export RUST_TEST_THREADS="$jobs"
    echo "RUST_TEST_THREADS=$RUST_TEST_THREADS (local default; pass --test-threads, set RUST_TEST_THREADS, or set IVYGREP_UNBOUNDED_LOCAL_TESTS=1 to override)"
  fi
}

while (($#)); do
  case "$1" in
    --ci)
      mode="ci"
      do_fmt=1
      do_shellcheck=1
      do_clippy=1
      do_web=1
      do_python_tests=1
      scope_flags=(--lib --bins --tests)
      ;;
    --quick)
      mode="quick"
      do_fmt=0
      do_shellcheck=0
      do_clippy=0
      do_web=0
      do_python_tests=0
      scope_flags=(--lib)
      ;;
    --all-targets)
      scope_flags=(--all-targets)
      ;;
    --stress)
      mode="stress"
      do_fmt=0
      do_shellcheck=0
      do_clippy=0
      do_web=0
      do_python_tests=0
      ;;
    --e2e)
      run_e2e=1
      ;;
    --release)
      force_release=1
      ;;
    --hash-only)
      cargo_flags+=(--no-default-features)
      ;;
    --features)
      [[ $# -ge 2 ]] || { echo "--features needs value" >&2; exit 2; }
      cargo_flags+=(--features "$2")
      if features_include_cuda "$2"; then
        wants_cuda=1
      fi
      shift
      ;;
    --no-fmt)
      do_fmt=0
      ;;
    --no-shellcheck)
      do_shellcheck=0
      ;;
    --no-clippy)
      do_clippy=0
      ;;
    --no-web)
      do_web=0
      ;;
    --no-python)
      do_python_tests=0
      ;;
    --filter)
      [[ $# -ge 2 ]] || { echo "--filter needs value" >&2; exit 2; }
      filter=("$2")
      shift
      ;;
    --nocapture)
      test_args+=(--nocapture)
      ;;
    --test-threads)
      [[ $# -ge 2 ]] || { echo "--test-threads needs value" >&2; exit 2; }
      test_args+=(--test-threads "$2")
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      extra_args=("$@")
      break
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

export IVYGREP_NO_AUTOSPAWN="${IVYGREP_NO_AUTOSPAWN:-1}"
export IVYGREP_ENHANCE_MAX_LOAD_RATIO="${IVYGREP_ENHANCE_MAX_LOAD_RATIO:-0}"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
configure_cuda_compute_cap
configure_build_profile
configure_local_resource_limits

if ((do_fmt)); then
  run cargo fmt -- --check
fi

if ((do_shellcheck)); then
  if ! command -v shellcheck >/dev/null 2>&1; then
    echo "shellcheck not found; install it or pass --no-shellcheck" >&2
    exit 127
  fi
  run shellcheck install.sh build.sh test.sh bench.sh scripts/bootstrap_stress_fixtures.sh scripts/e2e_all.sh scripts/e2e_web_ui.sh scripts/e2e_procedures.sh scripts/e2e_neural_backend.sh scripts/e2e_x86_baseline.sh scripts/e2e_cached_model.sh scripts/stress_large_repo.sh
fi

if ((do_clippy)); then
  run cargo clippy "${profile_flags[@]}" --all-targets "${cargo_flags[@]}" -- -D warnings
fi

if ((do_web)); then
  if ! command -v pnpm >/dev/null 2>&1; then
    echo "pnpm not found; install it or pass --no-web" >&2
    exit 127
  fi
  run pnpm -C web install --frozen-lockfile
  run pnpm -C web check
  web_dist_before="$(web_dist_fingerprint)"
  run pnpm -C web build
  web_dist_after="$(web_dist_fingerprint)"
  if [[ "$web_dist_before" != "$web_dist_after" ]]; then
    echo "pnpm build changed web/dist; commit regenerated assets" >&2
    diff -u <(printf '%s\n' "$web_dist_before") <(printf '%s\n' "$web_dist_after") || true
    exit 1
  fi
fi

if [[ "$mode" == "stress" ]]; then
  cmd=(cargo test "${profile_flags[@]}" --test stress_harness "${cargo_flags[@]}" "${extra_args[@]}" -- --ignored --nocapture --test-threads 1)
else
  cmd=(cargo test "${profile_flags[@]}" "${scope_flags[@]}" "${cargo_flags[@]}" "${extra_args[@]}" "${filter[@]}")
  if ((${#test_args[@]})); then
    cmd+=(-- "${test_args[@]}")
  fi
fi

run "${cmd[@]}"

if ((do_python_tests)); then
  run python3 -m unittest discover -s tests -p 'test_*.py' -v
fi

if ((run_e2e)); then
  # Quick/library-only tests do not produce the CLI used by acceptance tests.
  run cargo build "${profile_flags[@]}" "${cargo_flags[@]}" --bin ig
  target_dir="$(cargo metadata --format-version 1 --no-deps |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')"
  e2e_binary="$target_dir/debug/ig"
  if ((${#profile_flags[@]})) && [[ "${profile_flags[*]}" == *--release* ]]; then
    e2e_binary="$target_dir/release/ig"
  fi
  run scripts/e2e_all.sh --binary "$e2e_binary"
fi
