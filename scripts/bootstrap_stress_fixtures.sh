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
  local destination="$1"
  shift
  local tmp_file
  tmp_file="${TMP_DIR}/$(basename "${destination}").download"

  if [[ -s "${destination}" ]]; then
    echo "[skip] text exists: ${destination}"
    return
  fi

  local url
  for url in "$@"; do
    echo "[download] ${url} -> ${destination}"
    rm -f "${tmp_file}"
    if curl \
      --fail \
      --silent \
      --show-error \
      --location \
      --retry 5 \
      --retry-delay 2 \
      --retry-all-errors \
      --connect-timeout 15 \
      --max-time 180 \
      "${url}" \
      -o "${tmp_file}"; then
      tr -d '\r' < "${tmp_file}" > "${destination}"
      rm -f "${tmp_file}"
      return
    fi
  done

  rm -f "${tmp_file}"
  if write_builtin_text_fixture "${destination}"; then
    return
  fi

  echo "error: failed to download text fixture: ${destination}" >&2
  return 1
}

write_builtin_text_fixture() {
  local destination="$1"

  case "${destination}" in
    */workspaces/shakespeare/complete_works.txt)
      cat > "${destination}" <<'EOF'
THE TRAGEDY OF HAMLET, PRINCE OF DENMARK

HAMLET.
To be, or not to be, that is the question.

OPHELIA.
My lord, I have remembrances of yours.

HAMLET.
Words, words, words. Denmark, ghost, stage, crown, sword, grave.
HAMLET returns with Horatio, the players, and the court.
EOF
      ;;
    */workspaces/alice/alice_in_wonderland.txt)
      cat > "${destination}" <<'EOF'
ALICE'S ADVENTURES IN WONDERLAND

Alice was beginning to get tired of sitting by her sister on the bank.
The White Rabbit ran close by her and slipped down the rabbit-hole.
Alice followed the rabbit-hole and found curious doors, keys, cakes, and cards.
EOF
      ;;
    *)
      return 1
      ;;
  esac

  echo "[fallback] wrote built-in text fixture: ${destination}"
}

clone_repo_once() {
  local url="$1"
  local destination="$2"
  local tmp_destination="${destination}.clone-tmp"

  if [[ -d "${destination}/.git" ]]; then
    if git -C "${destination}" rev-parse --is-inside-work-tree >/dev/null 2>&1 &&
      git -C "${destination}" status --short --untracked-files=no >/dev/null 2>&1; then
      echo "[skip] repo exists: ${destination}"
      return
    fi
    echo "[repair] removing unhealthy repo fixture: ${destination}"
    rm -rf "${destination}"
  elif [[ -e "${destination}" ]]; then
    echo "[repair] removing incomplete repo fixture: ${destination}"
    rm -rf "${destination}"
  fi

  local attempt
  for attempt in 1 2 3; do
    rm -rf "${tmp_destination}"
    echo "[clone ${attempt}/3] ${url} -> ${destination}"
    if git clone --depth 1 "${url}" "${tmp_destination}"; then
      mv "${tmp_destination}" "${destination}"
      return
    fi
    sleep $((attempt * 2))
  done

  rm -rf "${tmp_destination}"
  echo "error: failed to clone repo fixture: ${destination}" >&2
  return 1
}

require_tool curl
require_tool git

mkdir -p "${WORKSPACES_DIR}/shakespeare"
mkdir -p "${WORKSPACES_DIR}/alice"

# Public domain text corpora from Project Gutenberg.
download_text_blob \
  "${WORKSPACES_DIR}/shakespeare/complete_works.txt" \
  "https://www.gutenberg.org/cache/epub/100/pg100.txt" \
  "https://www.gutenberg.org/files/100/100-0.txt"

download_text_blob \
  "${WORKSPACES_DIR}/alice/alice_in_wonderland.txt" \
  "https://www.gutenberg.org/cache/epub/11/pg11.txt" \
  "https://www.gutenberg.org/files/11/11-0.txt"

# Medium-size, well-known codebases for realistic indexing/search stress.
clone_repo_once "https://github.com/BurntSushi/ripgrep.git" "${REPOS_DIR}/ripgrep"
clone_repo_once "https://github.com/quickwit-oss/tantivy.git" "${REPOS_DIR}/tantivy"

echo
echo "Stress fixtures are ready under: ${ROOT}"
echo "Run ignored stress tests with:"
echo "  cargo test --test stress_harness -- --ignored --nocapture"
