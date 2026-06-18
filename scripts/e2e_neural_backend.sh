#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
root=$(CDPATH='' cd "$script_dir/.." && pwd)
ig_bin="$root/target/release/ig"
expected_backend=""
allowed_backend=""
model_profile=""

usage() {
  cat <<'EOF'
Validate neural embedding execution through an expected local backend.

Usage:
  ./scripts/e2e_neural_backend.sh --expect-backend TEXT [--binary PATH]

Options:
  --binary PATH          Use this ig binary (default: ./target/release/ig)
  --expect-backend TEXT  Status substring required after neural enhancement
  --allow-backend TEXT   Alternate accepted status substring
  --model-profile NAME   Set IVYGREP_MODEL_PROFILE for this backend check
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
    --model-profile)
      [ "$#" -ge 2 ] || fail "--model-profile needs name"
      model_profile=$2
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
if [ -n "$model_profile" ]; then
  export IVYGREP_MODEL_PROFILE="$model_profile"
fi

"$ig_bin" --add "$project" --force --json --no-watch --hash >/dev/null

download_attempts=${IVYGREP_E2E_DOWNLOAD_ATTEMPTS:-5}
retry_delay=${IVYGREP_E2E_RETRY_DELAY_SECONDS:-15}
case "$download_attempts" in
  ''|*[!0-9]*) fail "IVYGREP_E2E_DOWNLOAD_ATTEMPTS must be a positive integer" ;;
esac
[ "$download_attempts" -gt 0 ] ||
  fail "IVYGREP_E2E_DOWNLOAD_ATTEMPTS must be a positive integer"
case "$retry_delay" in
  ''|*[!0-9]*) fail "IVYGREP_E2E_RETRY_DELAY_SECONDS must be a non-negative integer" ;;
esac

enhance_log="$tmp_root/enhance.log"
attempt=1
while ! "$ig_bin" --enhance-internal "$project" >"$enhance_log" 2>&1; do
  cat "$enhance_log" >&2
  if [ "$attempt" -ge "$download_attempts" ] ||
    ! grep -Eiq \
      'status code (429|500|502|503|504)([^0-9]|$)|connection (reset|refused|timed out)|operation timed out|temporary failure|dns error|failed to lookup address|network is unreachable' \
      "$enhance_log"
  then
    fail "neural enhancement failed on attempt $attempt"
  fi

  echo "Transient model download failure; retrying in ${retry_delay}s (attempt $((attempt + 1))/$download_attempts)" >&2
  sleep "$retry_delay"
  attempt=$((attempt + 1))
  if [ "$retry_delay" -lt 120 ]; then
    retry_delay=$((retry_delay * 2))
    [ "$retry_delay" -le 120 ] || retry_delay=120
  fi
done

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
