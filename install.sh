#!/bin/sh
set -eu

repository="bvolpato/ivygrep"
install_dir="${IVYGREP_INSTALL_DIR:-$HOME/.local/bin}"
version="${IVYGREP_VERSION:-latest}"

for command_name in curl tar; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "ivygrep installer: $command_name is required" >&2
        exit 1
    fi
done

case "$version" in
    latest)
        latest_url="$(
            curl -fsSL -o /dev/null -w '%{url_effective}' \
                "https://github.com/$repository/releases/latest"
        )"
        tag="${latest_url##*/}"
        ;;
    v*) tag="$version" ;;
    *) tag="v$version" ;;
esac

if [ -z "$tag" ]; then
    echo "ivygrep installer: could not determine the latest release" >&2
    exit 1
fi

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target="linux-x86_64-musl" ;;
    Linux-aarch64 | Linux-arm64) target="linux-aarch64-musl" ;;
    Darwin-x86_64) target="macos-x86_64" ;;
    Darwin-arm64 | Darwin-aarch64) target="macos-aarch64" ;;
    *)
        echo "ivygrep installer: unsupported platform $(uname -s)-$(uname -m)" >&2
        exit 1
        ;;
esac

archive="ivygrep-$tag-$target.tar.gz"
base_url="https://github.com/$repository/releases/download/$tag"
tmp_dir="$(mktemp -d)"
install_tmp=""
cleanup() {
    rm -rf "$tmp_dir"
    if [ -n "$install_tmp" ]; then
        rm -f "$install_tmp"
    fi
}
trap cleanup EXIT HUP INT TERM

curl -fsSL "$base_url/$archive" -o "$tmp_dir/$archive"
curl -fsSL "$base_url/$archive.sha256" -o "$tmp_dir/$archive.sha256"

(
    cd "$tmp_dir"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c "$archive.sha256"
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 -c "$archive.sha256"
    else
        echo "ivygrep installer: sha256sum or shasum is required" >&2
        exit 1
    fi
)

tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"
mkdir -p "$install_dir"
install_tmp="$install_dir/.ig.tmp.$$"
cp "$tmp_dir/ivygrep-$tag-$target/ig" "$install_tmp"
chmod 0755 "$install_tmp"
mv -f "$install_tmp" "$install_dir/ig"
install_tmp=""

echo "Installed ivygrep $tag to $install_dir/ig"
"$install_dir/ig" --version

case ":${PATH:-}:" in
    *":$install_dir:"*) ;;
    *)
        echo "Add $install_dir to PATH to run ig from any shell."
        ;;
esac
