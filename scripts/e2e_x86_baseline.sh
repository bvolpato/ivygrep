#!/bin/sh
set -eu

qemu=${QEMU_X86_64:-qemu-x86_64}
binary=

while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary)
      binary=$2
      shift 2
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

[ -n "$binary" ] || { echo "--binary is required" >&2; exit 2; }
[ -x "$binary" ] || { echo "binary is not executable: $binary" >&2; exit 1; }
command -v "$qemu" >/dev/null 2>&1 || { echo "missing $qemu" >&2; exit 1; }

tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/ivygrep-x86-baseline.XXXXXX")
trap 'rm -rf "$tmp_root"' EXIT INT TERM
project="$tmp_root/project"
mkdir -p "$project/.git"
cat > "$project/lib.rs" <<'EOF'
pub fn baseline_cpu_search() -> &'static str {
    "portable x86 index"
}
EOF

run_qemu() {
  "$qemu" -cpu qemu64 "$binary" "$@"
}

export IVYGREP_HOME="$tmp_root/home"
export IVYGREP_NO_AUTOSPAWN=1
run_qemu --version
run_qemu --add "$project" --force --no-watch --hash --json > "$tmp_root/add.json"
run_qemu --literal --json "baseline_cpu_search" "$project" > "$tmp_root/search.json"
grep -Fq "lib.rs" "$tmp_root/search.json"

echo "baseline x86-64 artifact passed under QEMU qemu64"
