#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
root=$(CDPATH='' cd "$script_dir/.." && pwd)
ig_bin="$root/target/release/ig"
keep_tmp=0

usage() {
  cat <<'EOF'
Run bundled Web UI browser acceptance with Playwright.

Usage:
  ./scripts/e2e_web_ui.sh [--binary PATH] [--keep-tmp]

The script creates a disposable indexed workspace, starts an isolated daemon,
and drives the compiled Web UI through a real Chromium DOM session.
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

command -v git >/dev/null 2>&1 || fail "git is required"
command -v pnpm >/dev/null 2>&1 || fail "pnpm is required; run pnpm -C web install --frozen-lockfile"

tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/ivygrep-web-e2e.XXXXXX")
tmp_root=$(CDPATH='' cd "$tmp_root" && pwd -P) || fail "could not resolve temp root"
project="$tmp_root/project"
out_dir="$tmp_root/out"
mkdir -p "$project/src" "$out_dir"
git -C "$project" init -q
cat > "$project/src/web.ts" <<'EOF'
export function semanticBrowserResult(): string {
  return "semantic browser marker";
}
EOF

home="$tmp_root/ivygrep-home"
daemon_pid=""
cleanup() {
  if [ -n "$daemon_pid" ]; then
    kill "$daemon_pid" 2>/dev/null || true
    wait "$daemon_pid" 2>/dev/null || true
  fi
  if [ "$keep_tmp" -eq 0 ]; then
    rm -rf "$tmp_root"
  else
    echo "kept temp root: $tmp_root"
  fi
}
trap cleanup EXIT INT TERM

export IVYGREP_HOME="$home"
export IVYGREP_NO_AUTOSPAWN=1
export IVYGREP_NO_BROWSER=1
export IVYGREP_ENHANCE_MAX_LOAD_RATIO=0
export CARGO_TERM_COLOR=never

"$ig_bin" --add "$project" --force --no-watch --hash --json >"$out_dir/add.json"

"$ig_bin" --daemon >"$out_dir/daemon.log" 2>&1 &
daemon_pid=$!
ready=0
attempt=1
while [ "$attempt" -le 100 ]; do
  if [ -S "$home/daemon.sock" ] || [ -f "$home/daemon.port" ]; then
    ready=1
    break
  fi
  if ! kill -0 "$daemon_pid" 2>/dev/null; then
    cat "$out_dir/daemon.log" >&2
    fail "daemon exited before becoming ready"
  fi
  sleep 0.1
  attempt=$((attempt + 1))
done
[ "$ready" -eq 1 ] || fail "daemon did not become ready"

"$ig_bin" --web --host 127.0.0.1 --port 0 "semantic browser marker" "$project" >"$out_dir/web.txt"
web_url=$(sed -n 's/^ivygrep web listening at //p' "$out_dir/web.txt" | tail -n 1)
[ -n "$web_url" ] || {
  cat "$out_dir/web.txt" >&2
  fail "web server did not report a URL"
}

IVYGREP_WEB_URL="$web_url" \
  pnpm -C "$root/web" exec playwright test e2e/web-ui.pw.ts --config=playwright.config.ts

echo "Web UI browser procedure passed"
