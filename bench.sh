#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

usage() {
  cat <<'EOF'
Run ivygrep benchmarks.

Usage:
  ./bench.sh [options] [-- criterion-args...]

Default:
  Runs the critical Criterion benchmark without comparing against the local
  Criterion baseline:
  cargo bench --bench indexer_bench indexer/incremental_reindex_no_change -- --noplot --save-baseline ivygrep-smoke

Options:
  --quick                  Run critical benchmark only (default)
  --full                   Run all Criterion benchmarks in benches/indexer_bench.rs
  --name NAME              Criterion benchmark name/filter
  --keep-baseline          Compare/save Criterion's local target/criterion baseline
  --guard BASELINE_REF     Compare NAME against git ref using scripts/benchmark_guard.py
  --threshold FLOAT        Regression threshold for --guard (default: 1.15)
  --linux-kernel PATH      Run large Linux checkout latency/index benchmark
  --linux-relevance PATH   Run Linux relevance benchmark
  --bench-home PATH        IVYGREP_HOME path for Linux benchmark scripts
  --skip-build             Pass through to Linux benchmark scripts
  --skip-index             Pass through to Linux benchmark scripts
  -h, --help               Show help

Examples:
  ./bench.sh
  ./bench.sh --full
  ./bench.sh --guard origin/main
  ./bench.sh --linux-kernel /home/bruno/githubworkspace/linux --skip-index
  ./bench.sh --linux-relevance /home/bruno/githubworkspace/linux --skip-build --skip-index
EOF
  echo
  echo "Note:"
  echo "  Criterion reports per-operation latency. Microbenchmarks run repeated"
  echo "  logical operations so timed samples last long enough to be stable."
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

cleanup_criterion_baseline() {
  local baseline="$1"
  [[ -d target/criterion ]] || return 0
  find target/criterion -type d -name "$baseline" -prune -exec rm -rf {} +
}

mode="quick"
bench_name="indexer/incremental_reindex_no_change"
keep_baseline=0
smoke_baseline="ivygrep-smoke"
guard_ref=""
threshold="1.15"
linux_kernel=""
linux_relevance=""
bench_home=""
skip_build=0
skip_index=0
extra_args=()

while (($#)); do
  case "$1" in
    --quick)
      mode="quick"
      ;;
    --full)
      mode="full"
      ;;
    --name)
      [[ $# -ge 2 ]] || { echo "--name needs value" >&2; exit 2; }
      bench_name="$2"
      shift
      ;;
    --keep-baseline)
      keep_baseline=1
      ;;
    --guard)
      [[ $# -ge 2 ]] || { echo "--guard needs baseline ref" >&2; exit 2; }
      mode="guard"
      guard_ref="$2"
      shift
      ;;
    --threshold)
      [[ $# -ge 2 ]] || { echo "--threshold needs value" >&2; exit 2; }
      threshold="$2"
      shift
      ;;
    --linux-kernel)
      [[ $# -ge 2 ]] || { echo "--linux-kernel needs checkout path" >&2; exit 2; }
      mode="linux-kernel"
      linux_kernel="$2"
      shift
      ;;
    --linux-relevance)
      [[ $# -ge 2 ]] || { echo "--linux-relevance needs checkout path" >&2; exit 2; }
      mode="linux-relevance"
      linux_relevance="$2"
      shift
      ;;
    --bench-home)
      [[ $# -ge 2 ]] || { echo "--bench-home needs path" >&2; exit 2; }
      bench_home="$2"
      shift
      ;;
    --skip-build)
      skip_build=1
      ;;
    --skip-index)
      skip_index=1
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

export IVYGREP_NO_AUTOSPAWN="${IVYGREP_NO_AUTOSPAWN:-1}"
export IVYGREP_ENHANCE_MAX_LOAD_RATIO="${IVYGREP_ENHANCE_MAX_LOAD_RATIO:-0}"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"

criterion_args=(--noplot)
if ((keep_baseline == 0)); then
  cleanup_criterion_baseline "$smoke_baseline"
  criterion_args+=(--save-baseline "$smoke_baseline")
fi

cleanup_smoke_baseline_on_exit() {
  if ((keep_baseline == 0)); then
    cleanup_criterion_baseline "$smoke_baseline"
  fi
}
trap cleanup_smoke_baseline_on_exit EXIT

case "$mode" in
  quick)
    run cargo bench --bench indexer_bench "$bench_name" -- "${criterion_args[@]}" "${extra_args[@]}"
    ;;
  full)
    run cargo bench --bench indexer_bench -- "${criterion_args[@]}" "${extra_args[@]}"
    ;;
  guard)
    [[ -n "$guard_ref" ]] || { echo "--guard needs baseline ref" >&2; exit 2; }
    run python3 scripts/benchmark_guard.py \
      --baseline-ref "$guard_ref" \
      --bench-target indexer_bench \
      --bench-name "$bench_name" \
      --threshold "$threshold"
    ;;
  linux-kernel)
    [[ -n "$linux_kernel" ]] || { echo "--linux-kernel needs checkout path" >&2; exit 2; }
    cmd=(uv run scripts/bench_linux_kernel.py --kernel "$linux_kernel")
    [[ -z "$bench_home" ]] || cmd+=(--bench-home "$bench_home")
    ((skip_build == 0)) || cmd+=(--skip-build)
    ((skip_index == 0)) || cmd+=(--skip-index)
    run "${cmd[@]}"
    ;;
  linux-relevance)
    [[ -n "$linux_relevance" ]] || { echo "--linux-relevance needs checkout path" >&2; exit 2; }
    cmd=(uv run scripts/bench_linux_relevance.py --kernel "$linux_relevance")
    [[ -z "$bench_home" ]] || cmd+=(--bench-home "$bench_home")
    ((skip_build == 0)) || cmd+=(--skip-build)
    ((skip_index == 0)) || cmd+=(--skip-index)
    run "${cmd[@]}"
    ;;
  *)
    echo "unknown benchmark mode: $mode" >&2
    exit 2
    ;;
esac
