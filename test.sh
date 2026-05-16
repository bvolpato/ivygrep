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
  --hash-only           Test without default neural feature
  --features FEATURES   Pass explicit Cargo features
  --no-fmt              Skip cargo fmt -- --check
  --no-shellcheck       Skip ShellCheck
  --no-clippy           Skip cargo clippy
  --filter NAME         Pass Cargo test filter
  --nocapture           Pass --nocapture to test harness
  --test-threads N      Pass --test-threads to test harness
  -h, --help            Show help

Examples:
  ./test.sh
  ./test.sh --quick
  ./test.sh --hash-only
  ./test.sh --filter query_aliases --nocapture
  ./scripts/bootstrap_stress_fixtures.sh && ./test.sh --stress
EOF
}

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  "$@"
}

mode="ci"
do_fmt=1
do_shellcheck=1
do_clippy=1
cargo_flags=()
scope_flags=(--lib --bins --tests)
filter=()
test_args=()
extra_args=()

while (($#)); do
  case "$1" in
    --ci)
      mode="ci"
      do_fmt=1
      do_shellcheck=1
      do_clippy=1
      scope_flags=(--lib --bins --tests)
      ;;
    --quick)
      mode="quick"
      do_fmt=0
      do_shellcheck=0
      do_clippy=0
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
      ;;
    --hash-only)
      cargo_flags+=(--no-default-features)
      ;;
    --features)
      [[ $# -ge 2 ]] || { echo "--features needs value" >&2; exit 2; }
      cargo_flags+=(--features "$2")
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
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"

if ((do_fmt)); then
  run cargo fmt -- --check
fi

if ((do_shellcheck)); then
  if ! command -v shellcheck >/dev/null 2>&1; then
    echo "shellcheck not found; install it or pass --no-shellcheck" >&2
    exit 127
  fi
  run shellcheck build.sh test.sh bench.sh scripts/bootstrap_stress_fixtures.sh
fi

if ((do_clippy)); then
  run cargo clippy --all-targets "${cargo_flags[@]}" -- -D warnings
fi

if [[ "$mode" == "stress" ]]; then
  cmd=(cargo test --test stress_harness "${cargo_flags[@]}" "${extra_args[@]}" -- --ignored --nocapture --test-threads 1)
else
  cmd=(cargo test "${scope_flags[@]}" "${cargo_flags[@]}" "${extra_args[@]}" "${filter[@]}")
  if ((${#test_args[@]})); then
    cmd+=(-- "${test_args[@]}")
  fi
fi

run "${cmd[@]}"
