#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
root=$(CDPATH='' cd "$script_dir/.." && pwd)
ig_bin="$root/target/release/ig"
keep_tmp=0

usage() {
  cat <<'EOF'
Run documented ivygrep CLI procedures end-to-end.

Usage:
  ./scripts/e2e_procedures.sh [options]

Options:
  --binary PATH   Use this ig binary (default: ./target/release/ig)
  --keep-tmp      Keep temporary IVYGREP_HOME and fixture project
  -h, --help      Show help

Covered procedures:
  --help, --version, --status --json, first-run auto-index search,
  --add, scoped search, --include, --exclude, --literal,
  --file-name-only, --first-line-only, --doctor, and --rm.
EOF
}

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

run() {
  {
    printf '+'
    printf ' %s' "$@"
    printf '\n'
  } >&2
  "$@"
}

contains() {
  file=$1
  needle=$2
  label=$3
  grep -Fq -- "$needle" "$file" || fail "$label: expected $needle in $file"
}

not_contains() {
  file=$1
  needle=$2
  label=$3
  if grep -Fq -- "$needle" "$file"; then
    fail "$label: did not expect $needle in $file"
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary)
      [ "$#" -ge 2 ] || fail "--binary needs path"
      ig_bin=$2
      shift 2
      ;;
    --keep-tmp)
      keep_tmp=1
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

[ -x "$ig_bin" ] || fail "ig binary not executable: $ig_bin"

tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/ivygrep-e2e.XXXXXX")
cleanup() {
  if [ "$keep_tmp" -eq 0 ]; then
    rm -rf "$tmp_root"
  else
    echo "kept temp root: $tmp_root"
  fi
}
trap cleanup EXIT INT TERM

project="$tmp_root/project"
out_dir="$tmp_root/out"
mkdir -p "$project/.git" "$project/src/auth" "$project/src/payments" "$project/docs" "$project/vendor" "$out_dir"

cat > "$project/src/payments/tax.rs" <<'EOF'
pub fn calculate_sales_tax(subtotal: u64, region: &str) -> u64 {
    let regional_tax_rate = if region == "CA" { 8 } else { 5 };
    subtotal * regional_tax_rate / 100
}
EOF

cat > "$project/src/auth/session.rs" <<'EOF'
pub fn refresh_session_token(user_id: &str) -> String {
    format!("session-{user_id}")
}
EOF

cat > "$project/docs/usage.md" <<'EOF'
# Agent setup guide
Configure MCP agent search context here.
EOF

cat > "$project/vendor/tracker.rs" <<'EOF'
pub fn tracking_pixel() {}
EOF

export IVYGREP_HOME="$tmp_root/ivygrep-home"
export IVYGREP_NO_AUTOSPAWN="${IVYGREP_NO_AUTOSPAWN:-1}"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"

run "$ig_bin" --version > "$out_dir/version.txt"
contains "$out_dir/version.txt" "ivygrep" "version output"

run "$ig_bin" --help > "$out_dir/help.txt"
contains "$out_dir/help.txt" "--status" "help includes status flag"
contains "$out_dir/help.txt" "--mcp" "help includes mcp flag"
contains "$out_dir/help.txt" "--literal" "help includes literal flag"

run "$ig_bin" --status --json > "$out_dir/status-empty.json"
contains "$out_dir/status-empty.json" "[" "status json"

run "$ig_bin" --json --hash -n 5 "where is tax calculated" "$project" > "$out_dir/first-search.json"
contains "$out_dir/first-search.json" "src/payments/tax.rs" "first-run auto-index search"
not_contains "$out_dir/first-search.json" "vendor/" "first-run search should not prefer vendor"

run "$ig_bin" --add "$project" --force --json --no-watch --hash > "$out_dir/add.json"
contains "$out_dir/add.json" "\"indexed_files\"" "add json"

run "$ig_bin" --status --json > "$out_dir/status.json"
contains "$out_dir/status.json" "$project" "status lists indexed project"
contains "$out_dir/status.json" "\"watch_enabled\": false" "no-watch add status"

run "$ig_bin" --json --hash -n 5 "refresh session token" "$project/src/auth" > "$out_dir/scoped-search.json"
contains "$out_dir/scoped-search.json" "src/auth/session.rs" "scoped search"
not_contains "$out_dir/scoped-search.json" "src/payments/tax.rs" "scoped search excludes unrelated directory"

run "$ig_bin" --json --hash --include "*.md" -n 5 "agent setup guide" "$project" > "$out_dir/include.json"
contains "$out_dir/include.json" "docs/usage.md" "include glob search"

run "$ig_bin" --json --hash --exclude "vendor/**" -n 5 "tracking pixel" "$project" > "$out_dir/exclude.json"
contains "$out_dir/exclude.json" "[]" "exclude glob removes vendor-only hit"

run "$ig_bin" --literal --json -n 5 "calculate_sales_tax" "$project" > "$out_dir/literal.json"
contains "$out_dir/literal.json" "src/payments/tax.rs" "literal search"

run "$ig_bin" --file-name-only --literal "calculate_sales_tax" "$project" > "$out_dir/file-name-only.txt"
contains "$out_dir/file-name-only.txt" "src/payments/tax.rs" "file-name-only output"
not_contains "$out_dir/file-name-only.txt" "regional_tax_rate" "file-name-only omits snippets"

run "$ig_bin" --first-line-only --literal "refresh_session_token" "$project" > "$out_dir/first-line-only.txt"
contains "$out_dir/first-line-only.txt" "refresh_session_token" "first-line-only output"

(cd "$project" && run "$ig_bin" --doctor --json) > "$out_dir/doctor.json"
contains "$out_dir/doctor.json" "\"healthy\": true" "doctor healthy"

run "$ig_bin" --rm "$project" --json > "$out_dir/rm.json"
contains "$out_dir/rm.json" "\"removed\"" "remove json"

echo "E2E procedures passed"
