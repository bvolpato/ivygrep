#!/bin/sh
set -eu

binary=
cache=
expected_backend=
expected_file=
semantic_query="${IVYGREP_E2E_SEMANTIC_QUERY:-where is the routine that matches user intent to source after a warm model load}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary)
      binary=$2
      shift 2
      ;;
    --cache)
      cache=$2
      shift 2
      ;;
    --expect-backend)
      expected_backend=$2
      shift 2
      ;;
    --expect-file)
      expected_file=$2
      shift 2
      ;;
    --semantic-query)
      semantic_query=$2
      shift 2
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

[ -n "$binary" ] || { echo "--binary is required" >&2; exit 2; }
[ -n "$cache" ] || { echo "--cache is required" >&2; exit 2; }
[ -x "$binary" ] || { echo "binary is not executable: $binary" >&2; exit 1; }

tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/ivygrep-cached-model.XXXXXX")
trap 'rm -rf "$tmp_root"' EXIT INT TERM
project="$tmp_root/project"
mkdir -p "$project/.git" "$cache"
cat > "$project/lib.rs" <<'EOF'
/// Reuses persisted model output to match user intent to source code.
pub fn cached_model_search() -> &'static str {
    "portable neural cache"
}
EOF
cat > "$project/README.md" <<'EOF'
# Release notes

This decoy documents packaging and command-line flags. It does not implement source retrieval.
EOF

export HF_HOME="$cache"
export IVYGREP_HOME="$tmp_root/home"
export IVYGREP_NO_AUTOSPAWN=1
export IVYGREP_ENHANCE_MAX_LOAD_RATIO=0

"$binary" --add "$project" --force --no-watch --hash --json > "$tmp_root/add.json"
"$binary" --enhance-internal "$project"
"$binary" --status --json > "$tmp_root/status.json"
grep -Fq '"chunk_count":' "$tmp_root/status.json"
grep -Eq '"has_neural_vectors"[[:space:]]*:[[:space:]]*true' "$tmp_root/status.json"
grep -Eq '"neural_model"[[:space:]]*:[[:space:]]*\{' "$tmp_root/status.json"
if [ -n "$expected_backend" ]; then
  grep -Fq "$expected_backend" "$tmp_root/status.json"
fi

if [ -n "$expected_file" ]; then
  semantic_json="$tmp_root/semantic-search.json"
  "$binary" --json --force-neural --limit 1 "$semantic_query" "$project" >"$semantic_json"
  grep -Fq -- "$expected_file" "$semantic_json" || {
    echo "semantic query did not return expected file '$expected_file'" >&2
    cat "$semantic_json" >&2
    exit 1
  }
  grep -Eq '"neural_executed"[[:space:]]*:[[:space:]]*true' "$semantic_json" || {
    echo "forced semantic query did not execute neural retrieval" >&2
    cat "$semantic_json" >&2
    exit 1
  }
  echo "cached semantic retrieval passed: $expected_file"
fi

echo "cached neural model import passed"
