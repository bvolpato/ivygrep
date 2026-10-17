#!/bin/sh

# Include the longest daemon socket path used by the E2E scripts. macOS has a
# 104-byte sun_path, including its terminating NUL.
e2e_socket_tmp_parent() {
  parent=$(CDPATH='' cd "${TMPDIR:-/tmp}" && pwd -P) || return 1
  case "$(uname -s)" in
    CYGWIN*|MINGW*|MSYS*)
      printf '%s\n' "$parent"
      return
      ;;
  esac

  socket_suffix=/ivygrep-web-e2e.XXXXXX/ivygrep-home/daemon.sock
  socket_bytes=$(printf '%s' "$parent$socket_suffix" | LC_ALL=C wc -c | tr -d '[:space:]')
  if [ "$socket_bytes" -ge 104 ]; then
    parent=$(CDPATH='' cd /tmp && pwd -P) || return 1
    socket_bytes=$(printf '%s' "$parent$socket_suffix" | LC_ALL=C wc -c | tr -d '[:space:]')
    if [ "$socket_bytes" -ge 104 ]; then
      echo "temporary directory is too long for a daemon socket: $parent" >&2
      return 1
    fi
  fi
  printf '%s\n' "$parent"
}
