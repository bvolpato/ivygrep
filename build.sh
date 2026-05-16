#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

usage() {
  cat <<'EOF'
Build ivygrep.

Usage:
  ./build.sh [options] [-- cargo-build-args...]

Options:
  --release              Build optimized release binary (default)
  --debug                Build debug binary
  --hash-only            Build without default neural feature
  --features FEATURES    Pass explicit Cargo features
  --target TARGET        Build for target triple
  --locked               Require Cargo.lock to stay unchanged
  -h, --help             Show help

Examples:
  ./build.sh
  ./build.sh --debug
  ./build.sh --hash-only
  ./build.sh --features accelerate
EOF
  echo
  echo "Available sessions:"
  echo "  ./build.sh      Build ivygrep binary with selected profile/features."
  echo "  ./test.sh       Run ivygrep validation suite."
}

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  "$@"
}

profile="release"
cargo_args=(--bin ig)
extra_args=()

while (($#)); do
  case "$1" in
    --release)
      profile="release"
      ;;
    --debug)
      profile="debug"
      ;;
    --hash-only)
      cargo_args+=(--no-default-features)
      ;;
    --features)
      [[ $# -ge 2 ]] || { echo "--features needs value" >&2; exit 2; }
      cargo_args+=(--features "$2")
      shift
      ;;
    --target)
      [[ $# -ge 2 ]] || { echo "--target needs value" >&2; exit 2; }
      cargo_args+=(--target "$2")
      shift
      ;;
    --locked)
      cargo_args+=(--locked)
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

if [[ "$profile" == "release" ]]; then
  cargo_args=(--release "${cargo_args[@]}")
fi

run cargo build "${cargo_args[@]}" "${extra_args[@]}"
