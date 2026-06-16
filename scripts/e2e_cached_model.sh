#!/bin/sh
set -eu

binary=
cache=

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
pub fn cached_model_search() -> &'static str {
    "portable neural cache"
}
EOF

export HF_HOME="$cache"
export IVYGREP_HOME="$tmp_root/home"
export IVYGREP_NO_AUTOSPAWN=1
export IVYGREP_ENHANCE_MAX_LOAD_RATIO=0

"$binary" --add "$project" --force --no-watch --hash --json > "$tmp_root/add.json"
"$binary" --enhance-internal "$project"
"$binary" --status --json > "$tmp_root/status.json"
grep -Fq '"has_neural_vectors": true' "$tmp_root/status.json"
grep -Fq '"neural_model": {' "$tmp_root/status.json"

echo "cached neural model import passed"
