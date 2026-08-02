#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
root=$(CDPATH='' cd "$script_dir/.." && pwd)
ig_bin="${IVYGREP_E2E_BINARY:-$root/target/release/ig}"
skip_web=0

usage() {
  cat <<'EOF'
Run local ivygrep end-to-end acceptance.

Usage:
  ./scripts/e2e_all.sh [--binary PATH] [--skip-web]

Runs documented CLI procedures, daemon/local equivalence, and the bundled Web
UI browser flow. Neural model downloads stay opt-in through
scripts/e2e_neural_backend.sh.
EOF
}

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary)
      [ "$#" -ge 2 ] || fail "--binary needs path"
      ig_bin=$2
      shift 2
      ;;
    --skip-web)
      skip_web=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

case "$ig_bin" in
  /*) ;;
  *)
    ig_dir=$(CDPATH='' cd "$(dirname "$ig_bin")" && pwd) || fail "could not resolve binary directory: $ig_bin"
    ig_bin="$ig_dir/$(basename "$ig_bin")"
    ;;
esac

if [ ! -x "$ig_bin" ]; then
  echo "Building release binary for E2E acceptance: $ig_bin" >&2
  cargo build --locked --release
  if [ ! -x "$ig_bin" ] && [ "$ig_bin" = "$root/target/debug/ig" ]; then
    ig_bin="$root/target/release/ig"
  fi
fi
[ -x "$ig_bin" ] || fail "ig binary not executable: $ig_bin"

"$script_dir/e2e_procedures.sh" --binary "$ig_bin"

bench_home="$(mktemp -d "${TMPDIR:-/tmp}/ivygrep-e2e-daemon.XXXXXX")"
trap 'rm -rf "$bench_home"' EXIT INT TERM
python3 "$script_dir/check_daemon_equivalence.py" \
  --skip-build \
  --binary "$ig_bin" \
  --bench-home "$bench_home"

if [ "$skip_web" -eq 0 ]; then
  "$script_dir/e2e_web_ui.sh" --binary "$ig_bin"
fi

echo "All local E2E procedures passed"
