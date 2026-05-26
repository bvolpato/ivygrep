#!/usr/bin/env bash
#
# stress_large_repo.sh — measure ivygrep end-to-end on a large repository.
#
# Runs the three heavy phases — index (hash) → neural enhance → query — and
# reports, per phase: wall-clock time, peak resident memory (RSS), chunk/file
# counts, and per-query latency. A per-phase watchdog kills and flags any phase
# that exceeds its time budget, so a hang/deadlock surfaces as a clear failure
# instead of an indefinite stall.
#
# PRIVACY: this script prints ONLY metrics (counts, seconds, megabytes) and the
# query *strings you pass in*. It never prints file paths, file contents, or any
# matched code, so it is safe to run on private/internal repositories and paste
# the output. (Pass only generic query strings if even those are sensitive.)
#
# Usage:
#   scripts/stress_large_repo.sh /path/to/repo [options]
#
# Options:
#   --bin PATH              ig binary (default: target/release/ig, else `ig` on PATH)
#   --home DIR              IVYGREP_HOME to use (default: a fresh mktemp dir, removed after)
#   --keep-home             do not delete the temp IVYGREP_HOME on exit
#   --enhance-timeout SECS  watchdog for the neural enhance phase (default: 3600)
#   --index-timeout SECS    watchdog for the index phase (default: 3600)
#   --queries "a|b|c"       pipe-separated query strings (default: a generic code set)
#   --skip-enhance          index + query only (skip the neural enhance phase)
#
# Exit code is non-zero if any phase hangs (watchdog fired) or errors.
set -uo pipefail

# ---- defaults ---------------------------------------------------------------
REPO=""
BIN=""
HOME_DIR=""
KEEP_HOME=0
ENH_TIMEOUT=3600
IDX_TIMEOUT=3600
SKIP_ENHANCE=0
QUERIES='error handling and recovery|retry with exponential backoff|parse configuration file|user authentication and login|database connection pool|rate limiting middleware|structured logging setup|http request timeout'

# ---- arg parsing ------------------------------------------------------------
while [ $# -gt 0 ]; do
  case "$1" in
    --bin) BIN="$2"; shift 2 ;;
    --home) HOME_DIR="$2"; shift 2 ;;
    --keep-home) KEEP_HOME=1; shift ;;
    --enhance-timeout) ENH_TIMEOUT="$2"; shift 2 ;;
    --index-timeout) IDX_TIMEOUT="$2"; shift 2 ;;
    --queries) QUERIES="$2"; shift 2 ;;
    --skip-enhance) SKIP_ENHANCE=1; shift ;;
    -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
    -*) echo "unknown option: $1" >&2; exit 2 ;;
    *) if [ -z "$REPO" ]; then REPO="$1"; else echo "unexpected arg: $1" >&2; exit 2; fi; shift ;;
  esac
done

if [ -z "$REPO" ] || [ ! -d "$REPO" ]; then
  echo "usage: $0 /path/to/repo [options]   (see --help)" >&2
  exit 2
fi
REPO="$(cd "$REPO" && pwd -P)"

# ---- resolve binary ---------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
if [ -z "$BIN" ]; then
  if [ -x "$REPO_ROOT/target/release/ig" ]; then BIN="$REPO_ROOT/target/release/ig"
  elif command -v ig >/dev/null 2>&1; then BIN="$(command -v ig)"
  else echo "no ig binary found; build with 'cargo build --release' or pass --bin" >&2; exit 2; fi
fi
BIN="$(cd "$(dirname "$BIN")" && pwd -P)/$(basename "$BIN")"

# ---- isolated IVYGREP_HOME --------------------------------------------------
CLEAN_HOME=0
if [ -z "$HOME_DIR" ]; then
  HOME_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ig-stress.XXXXXX")"
  [ "$KEEP_HOME" -eq 0 ] && CLEAN_HOME=1
fi
mkdir -p "$HOME_DIR"
cleanup() { [ "$CLEAN_HOME" -eq 1 ] && rm -rf "$HOME_DIR"; }
trap cleanup EXIT

# No daemon/auto-spawn: we drive each phase explicitly and measure it in
# isolation. CI=1 disables the background load/thermal throttle so timings are
# not confounded by pauses (it does NOT change indexing/search behaviour).
export IVYGREP_HOME="$HOME_DIR"
export IVYGREP_NO_AUTOSPAWN=1
export CI=1

NCPU="$( (command -v nproc >/dev/null && nproc) || sysctl -n hw.ncpu 2>/dev/null || echo '?')"
echo "════════════════════════════════════════════════════════════════"
echo " ivygrep large-repo stress"
echo "   repo:   $REPO"
echo "   bin:    $BIN  ($("$BIN" --version 2>/dev/null | head -1))"
echo "   home:   $HOME_DIR"
echo "   cores:  $NCPU"
echo "════════════════════════════════════════════════════════════════"

OVERALL_RC=0

# run_phase LABEL TIMEOUT_SECS cmd...
# Runs cmd in the background; samples peak RSS of the process subtree every 2s;
# kills + flags it if it exceeds TIMEOUT_SECS. Sets PHASE_SECS / PHASE_RSS_MB.
run_phase() {
  local label="$1" timeout_s="$2"; shift 2
  local logf; logf="$(mktemp)"
  "$@" >"$logf" 2>&1 &
  local pid=$! start peak=0 now rss
  start=$(date +%s)
  while kill -0 "$pid" 2>/dev/null; do
    # Sum RSS (KB) of the phase process and any descendants it spawned.
    rss=$(
      { echo "$pid"; pgrep -P "$pid" 2>/dev/null; } | while read -r p; do
        ps -o rss= -p "$p" 2>/dev/null
      done | awk '{s+=$1} END{print s+0}'
    )
    [ -n "$rss" ] && [ "$rss" -gt "$peak" ] 2>/dev/null && peak=$rss
    now=$(date +%s)
    if [ $(( now - start )) -ge "$timeout_s" ]; then
      echo "  ⚠️  HANG: '$label' exceeded ${timeout_s}s — killing (possible deadlock)."
      kill -KILL "$pid" 2>/dev/null
      pgrep -P "$pid" 2>/dev/null | xargs -r kill -KILL 2>/dev/null
      PHASE_SECS=$(( now - start )); PHASE_RSS_MB=$(( peak / 1024 )); rm -f "$logf"
      return 124
    fi
    sleep 2
  done
  wait "$pid"; local rc=$?
  PHASE_SECS=$(( $(date +%s) - start )); PHASE_RSS_MB=$(( peak / 1024 ))
  if [ "$rc" -ne 0 ]; then
    echo "  ⚠️  '$label' exited $rc. Last output:"
    tail -3 "$logf" | sed 's/^/      /'
  fi
  rm -f "$logf"
  return $rc
}

status_json() { "$BIN" --status --json "$REPO" 2>/dev/null; }
# `--status --json` is a top-level ARRAY of workspace objects; pull the field
# from the entry whose root matches $REPO (fall back to the first entry).
jq_get() { REPO="$REPO" python3 -c "import sys,json,os
try:
    d=json.load(sys.stdin); r=os.environ.get('REPO','')
    e=next((w for w in d if isinstance(w,dict) and w.get('root') in (r, os.path.realpath(r))), (d[0] if isinstance(d,list) and d else (d if isinstance(d,dict) else {})))
    print(e.get('$1',''))
except Exception: print('')"; }

# ── Phase 1: index (hash) ────────────────────────────────────────────────────
echo
echo "▶ Phase 1/3: index (hash vectors)"
run_phase "index" "$IDX_TIMEOUT" "$BIN" --add "$REPO" --no-watch
RC=$?; IDX_SECS=$PHASE_SECS; IDX_RSS=$PHASE_RSS_MB
[ "$RC" -ne 0 ] && OVERALL_RC=1
ST="$(status_json)"
FILES="$(echo "$ST" | jq_get file_count)"
CHUNKS="$(echo "$ST" | jq_get chunk_count)"
echo "  files=$FILES  chunks=$CHUNKS  time=${IDX_SECS}s  peakRSS=${IDX_RSS}MB"
if [ -n "$CHUNKS" ] && [ "$CHUNKS" -gt 0 ] 2>/dev/null && [ "$IDX_SECS" -gt 0 ]; then
  echo "  throughput≈ $(( CHUNKS / IDX_SECS )) chunks/s"
fi

# ── Phase 2: neural enhance ──────────────────────────────────────────────────
if [ "$SKIP_ENHANCE" -eq 0 ]; then
  echo
  echo "▶ Phase 2/3: neural enhance (background-mode model)"
  run_phase "enhance" "$ENH_TIMEOUT" "$BIN" --enhance-internal "$REPO"
  RC=$?; ENH_SECS=$PHASE_SECS; ENH_RSS=$PHASE_RSS_MB
  if [ "$RC" -eq 124 ]; then
    echo "  ❌ enhance HUNG — this is the failure mode #69 fixed; please report."
    OVERALL_RC=1
  elif [ "$RC" -ne 0 ]; then
    OVERALL_RC=1
  fi
  HASNEURAL="$(status_json | jq_get has_neural_vectors)"
  echo "  has_neural_vectors=$HASNEURAL  time=${ENH_SECS}s  peakRSS=${ENH_RSS}MB"
else
  echo
  echo "▶ Phase 2/3: neural enhance — SKIPPED (--skip-enhance)"
  ENH_SECS=0; ENH_RSS=0; HASNEURAL="skipped"
fi

# ── Phase 3: query latency ───────────────────────────────────────────────────
echo
echo "▶ Phase 3/3: query latency (no daemon — each is a cold index load)"
IFS='|' read -r -a QARR <<< "$QUERIES"
LAT_SUM=0; LAT_N=0; LAT_MAX=0; LAT_MIN=999999
for q in "${QARR[@]}"; do
  [ -z "$q" ] && continue
  t0=$(python3 -c 'import time;print(time.time())')
  "$BIN" --no-watch -n 20 "$q" "$REPO" >/dev/null 2>&1
  ms=$(python3 -c "import time;print(int((time.time()-$t0)*1000))")
  printf '  %6dms  «%s»\n' "$ms" "$q"
  LAT_SUM=$((LAT_SUM+ms)); LAT_N=$((LAT_N+1))
  [ "$ms" -gt "$LAT_MAX" ] && LAT_MAX=$ms
  [ "$ms" -lt "$LAT_MIN" ] && LAT_MIN=$ms
done
[ "$LAT_N" -gt 0 ] && echo "  queries=$LAT_N  min=${LAT_MIN}ms  avg=$((LAT_SUM/LAT_N))ms  max=${LAT_MAX}ms"

# ── Summary ──────────────────────────────────────────────────────────────────
echo
echo "════════════════════════ SUMMARY ═══════════════════════════════"
printf ' files=%s chunks=%s\n' "$FILES" "$CHUNKS"
printf ' index:   %5ds  peakRSS %5dMB\n' "${IDX_SECS:-0}" "${IDX_RSS:-0}"
printf ' enhance: %5ds  peakRSS %5dMB  neural=%s\n' "${ENH_SECS:-0}" "${ENH_RSS:-0}" "${HASNEURAL:-?}"
[ "$LAT_N" -gt 0 ] && printf ' query:   min %dms / avg %dms / max %dms (n=%d, no daemon)\n' "$LAT_MIN" "$((LAT_SUM/LAT_N))" "$LAT_MAX" "$LAT_N"
if [ "$OVERALL_RC" -eq 0 ]; then echo " result:  ✅ all phases completed"; else echo " result:  ❌ a phase hung or errored (see above)"; fi
echo "═════════════════════════════════════════════════════════════════"
exit $OVERALL_RC
