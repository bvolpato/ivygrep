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
  ./build.sh --features cuda
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

profile="release"
cargo_args=(--bin ig)
extra_args=()
wants_cuda=0

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
      if features_include_cuda "$2"; then
        wants_cuda=1
      fi
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

configure_cuda_compute_cap
run cargo build "${cargo_args[@]}" "${extra_args[@]}"
