#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
root=$(CDPATH='' cd "$script_dir/.." && pwd)
ig_bin="$root/target/release/ig"
expected_backend=""
allowed_backend=""
model_profile=""
expected_file=""
check_worktree=0
semantic_query="where is the routine that matches user intent to source after a warm model load"
python=python3
if [ "${RUNNER_OS:-}" = "Windows" ]; then
  python=python
fi

usage() {
  cat <<'EOF'
Validate neural embedding execution through an expected local backend.

Usage:
  ./scripts/e2e_neural_backend.sh --expect-backend TEXT [options]

Options:
  --binary PATH          Use this ig binary (default: ./target/release/ig)
  --expect-backend TEXT  Status substring required after neural enhancement
  --allow-backend TEXT   Alternate accepted status substring
  --model-profile NAME   Set IVYGREP_MODEL_PROFILE for this backend check
  --expect-file PATH     Require a semantic query to return PATH
  --check-worktree       Also require branch-local neural retrieval from a Git worktree
  --semantic-query TEXT  Natural-language query used with --expect-file
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
    --expect-file)
      [ "$#" -ge 2 ] || fail "--expect-file needs path"
      expected_file=$2
      shift 2
      ;;
    --semantic-query)
      [ "$#" -ge 2 ] || fail "--semantic-query needs text"
      semantic_query=$2
      shift 2
      ;;
    --check-worktree)
      check_worktree=1
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
/// Reuses persisted model output to match user intent to source code.
pub fn retrieve_local_semantic_result(query: &str) -> String {
    format!("local semantic retrieval result for {query}")
}
EOF
cat > "$project/README.md" <<'EOF'
# Release notes

This decoy documents packaging and command-line flags. It does not implement source retrieval.
EOF

if [ "$check_worktree" -eq 1 ]; then
  cat > "$project/src/shadow.rs" <<'EOF'
pub fn base_shadow_generation_marker() -> bool { false }
EOF
  cat > "$project/src/deleted.rs" <<'EOF'
pub fn base_deleted_generation_marker() -> bool { false }
EOF
  git -C "$project" init -q
  git -C "$project" config user.name "ivygrep neural smoke test"
  git -C "$project" config user.email "neural-smoke@example.invalid"
  git -C "$project" config core.hooksPath "$tmp_root/no-hooks"
  git -C "$project" add .
  git -C "$project" -c commit.gpgsign=false commit -qm "seed neural smoke project"
fi

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
      'status code (429|500|502|503|504)([^0-9]|$)|cas-bridge\.xethub\.hf\.co.*status code 403([^0-9]|$)|connection (reset|refused|timed out)|operation timed out|temporary failure|dns error|failed to lookup address|network is unreachable' \
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
"$ig_bin" --status --json > "$tmp_root/status.json"
grep -Eq '"has_neural_vectors"[[:space:]]*:[[:space:]]*true' "$tmp_root/status.json" || {
  cat "$tmp_root/status.json" >&2
  fail "neural enhancement completed without persisted neural vectors"
}

reported_backend=""
if grep -Fq "$expected_backend" "$tmp_root/status.txt"; then
  reported_backend=$expected_backend
elif [ -n "$allowed_backend" ] && grep -Fq "$allowed_backend" "$tmp_root/status.txt"; then
  reported_backend=$allowed_backend
else
  fail "expected backend '$expected_backend' not reported; status follows: $(cat "$tmp_root/status.txt")"
fi

if [ -n "$expected_file" ]; then
  semantic_json="$tmp_root/semantic-search.json"
  if ! "$ig_bin" --json --force-neural --limit 1 "$semantic_query" "$project" >"$semantic_json" 2>"$tmp_root/semantic-search.err"; then
    cat "$tmp_root/semantic-search.err" >&2
    fail "semantic query failed"
  fi
  grep -Fq -- "$expected_file" "$semantic_json" || {
    echo "semantic query did not return expected file '$expected_file'" >&2
    cat "$semantic_json" >&2
    fail "semantic retrieval assertion failed"
  }
  grep -Eq '"neural_executed"[[:space:]]*:[[:space:]]*true' "$semantic_json" || {
    cat "$semantic_json" >&2
    fail "forced semantic query did not execute neural retrieval"
  }
  echo "Semantic retrieval procedure passed: $expected_file"
fi

if [ "$check_worktree" -eq 1 ]; then
  worktree="$tmp_root/worktree"
  git -C "$project" worktree add --quiet --detach "$worktree" HEAD
  cat > "$worktree/src/branch_local.rs" <<'EOF'
/// Retrieves branch-local source by semantic intent.
pub fn branch_local_semantic_retrieval_marker() -> bool {
    true
}
EOF
  cat > "$worktree/src/shadow.rs" <<'EOF'
pub fn overlay_shadow_generation_marker() -> bool { true }
EOF
  rm "$worktree/src/deleted.rs"
  for model in "$IVYGREP_HOME"/indexes/*/neural_model.json; do
    if [ -f "$model" ]; then
      sed 's/"revision": "[^"]*"/"revision": "stale-worktree-profile"/' \
        "$model" > "$model.tmp"
      mv "$model.tmp" "$model"
      break
    fi
  done
  "$ig_bin" --add "$worktree" --force --json --no-watch --hash > /dev/null
  "$ig_bin" --enhance-internal "$worktree" > "$tmp_root/worktree-enhance.log" 2>&1 || {
    cat "$tmp_root/worktree-enhance.log" >&2
    fail "worktree neural enhancement failed"
  }
  if [ -f "$model" ] && grep -Fq 'stale-worktree-profile' "$model"; then
    fail "worktree enhancement did not refresh its incompatible base model"
  fi
  worktree_json="$tmp_root/worktree-search.json"
  "$ig_bin" --json --force-neural --limit 3 \
    "branch_local_semantic_retrieval_marker" "$worktree" > "$worktree_json" ||
    fail "worktree forced neural search failed"
  grep -Fq 'src/branch_local.rs' "$worktree_json" || {
    cat "$worktree_json" >&2
    fail "worktree neural search omitted branch-local source"
  }
  grep -Eq '"neural_executed"[[:space:]]*:[[:space:]]*true' "$worktree_json" || {
    cat "$worktree_json" >&2
    fail "worktree forced search did not execute neural retrieval"
  }
  echo "Worktree neural retrieval procedure passed: src/branch_local.rs"

  for source in shadow inherited deleted; do
    case "$source" in
      shadow) query=overlay_shadow_generation_marker; expected=src/shadow.rs ;;
      inherited) query=$semantic_query; expected=src/lib.rs ;;
      deleted) query=base_deleted_generation_marker; expected='' ;;
    esac
    "$ig_bin" --json --force-neural --limit 5 "$query" "$worktree" > "$worktree_json"
    "$python" - "$worktree_json" "$expected" <<'PY'
import json
from pathlib import Path
import sys

groups = json.loads(Path(sys.argv[1]).read_text())
if sys.argv[2]:
    assert any(group["file_path"] == sys.argv[2] and group["hits"] for group in groups), groups
hits = [hit for group in groups for hit in group["hits"]]
assert any(hit.get("neural_executed") is True for hit in hits), groups
assert all(group["file_path"] != "src/deleted.rs" for group in groups), groups
assert all("base_shadow_generation_marker" not in hit["preview"] for hit in hits), groups
PY
  done
  "$ig_bin" --json --force-neural --limit 5 base_shadow_generation_marker "$project" > "$tmp_root/base-search.json"
  "$python" - "$tmp_root/base-search.json" <<'PY'
import json
from pathlib import Path
import sys

groups = json.loads(Path(sys.argv[1]).read_text())
hits = [hit for group in groups for hit in group["hits"]]
assert any(hit.get("neural_executed") is True for hit in hits), groups
assert any("base_shadow_generation_marker" in hit["preview"] for hit in hits), groups
assert all("overlay_shadow_generation_marker" not in hit["preview"] for hit in hits), groups
assert all(group["file_path"] != "src/branch_local.rs" for group in groups), groups
PY
  echo "Worktree neural inheritance, shadowing, deletion, and base isolation passed"
fi

echo "Neural backend procedure passed: $reported_backend"
