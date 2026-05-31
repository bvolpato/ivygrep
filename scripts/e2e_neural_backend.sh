#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
root=$(CDPATH='' cd "$script_dir/.." && pwd)
ig_bin="$root/target/release/ig"
expected_backend=""
allowed_backend=""

usage() {
  cat <<'EOF'
Validate neural embedding execution through an expected local backend.

Usage:
  ./scripts/e2e_neural_backend.sh --expect-backend TEXT [--binary PATH]

Options:
  --binary PATH          Use this ig binary (default: ./target/release/ig)
  --expect-backend TEXT  Status substring required after neural enhancement
  --allow-backend TEXT   Alternate accepted status substring
  -h, --help             Show help
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
    --expect-backend)
      [ "$#" -ge 2 ] || fail "--expect-backend needs text"
      expected_backend=$2
      shift 2
      ;;
    --allow-backend)
      [ "$#" -ge 2 ] || fail "--allow-backend needs text"
      allowed_backend=$2
      shift 2
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

[ -n "$expected_backend" ] || fail "--expect-backend is required"
[ -x "$ig_bin" ] || fail "ig binary not executable: $ig_bin"

tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/ivygrep-neural-e2e.XXXXXX")
cleanup() {
  rm -rf "$tmp_root"
}
trap cleanup EXIT INT TERM

project="$tmp_root/project"
mkdir -p "$project/src"
cat > "$project/src/lib.rs" <<'EOF'
pub fn retrieve_local_semantic_result(query: &str) -> String {
    format!("local result for {query}")
}
EOF

export IVYGREP_HOME="$tmp_root/home"
export IVYGREP_NO_AUTOSPAWN=1

"$ig_bin" --add "$project" --force --json --no-watch --hash >/dev/null
"$ig_bin" --enhance-internal "$project" >/dev/null
"$ig_bin" --status > "$tmp_root/status.txt"

reported_backend=""
if grep -Fq "$expected_backend" "$tmp_root/status.txt"; then
  reported_backend=$expected_backend
elif [ -n "$allowed_backend" ] && grep -Fq "$allowed_backend" "$tmp_root/status.txt"; then
  reported_backend=$allowed_backend
else
  fail "expected backend '$expected_backend' not reported; status follows: $(cat "$tmp_root/status.txt")"
fi

echo "Neural backend procedure passed: $reported_backend"
