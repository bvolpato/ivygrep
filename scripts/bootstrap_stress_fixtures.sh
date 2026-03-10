#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-tests/stress-data}"

WORKSPACES_DIR="${ROOT}/workspaces"
REPOS_DIR="${ROOT}/repos"
TMP_DIR="${ROOT}/.tmp"

mkdir -p "${WORKSPACES_DIR}" "${REPOS_DIR}" "${TMP_DIR}"

require_tool() {
  local tool="$1"
  if ! command -v "${tool}" >/dev/null 2>&1; then
    echo "error: missing required tool '${tool}'" >&2
    exit 1
  fi
}

download_text_blob() {
  local url="$1"
  local destination="$2"
  local tmp_file="${TMP_DIR}/$(basename "${destination}").download"

  if [[ -s "${destination}" ]]; then
    echo "[skip] text exists: ${destination}"
    return
  fi

  echo "[download] ${url} -> ${destination}"
  curl -fsSL "${url}" -o "${tmp_file}"
  tr -d '\r' < "${tmp_file}" > "${destination}"
  rm -f "${tmp_file}"
}

clone_repo_once() {
  local url="$1"
  local destination="$2"

  if [[ -d "${destination}/.git" ]]; then
    echo "[skip] repo exists: ${destination}"
    return
  fi

  echo "[clone] ${url} -> ${destination}"
  git clone --depth 1 "${url}" "${destination}"
}

require_tool curl
require_tool git

mkdir -p "${WORKSPACES_DIR}/shakespeare"
mkdir -p "${WORKSPACES_DIR}/alice"

# Public domain text corpora from Project Gutenberg.
download_text_blob \
  "https://www.gutenberg.org/cache/epub/100/pg100.txt" \
  "${WORKSPACES_DIR}/shakespeare/complete_works.txt"

download_text_blob \
  "https://www.gutenberg.org/cache/epub/11/pg11.txt" \
  "${WORKSPACES_DIR}/alice/alice_in_wonderland.txt"

# Medium-size, well-known codebases for realistic indexing/search stress.
clone_repo_once "https://github.com/BurntSushi/ripgrep.git" "${REPOS_DIR}/ripgrep"
clone_repo_once "https://github.com/quickwit-oss/tantivy.git" "${REPOS_DIR}/tantivy"

echo
echo "Stress fixtures are ready under: ${ROOT}"
echo "Run ignored stress tests with:"
echo "  cargo test --test stress_harness -- --ignored --nocapture"
