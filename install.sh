#!/bin/sh
set -eu

repository="bvolpato/ivygrep"
install_dir="${IVYGREP_INSTALL_DIR:-$HOME/.local/bin}"
version="${IVYGREP_VERSION:-latest}"
accelerator="${IVYGREP_ACCELERATOR:-auto}"

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

case "$accelerator" in
    auto | portable | none | cuda | metal) ;;
    *)
        echo "ivygrep installer: unsupported IVYGREP_ACCELERATOR=$accelerator" >&2
        echo "Use auto, portable, cuda, or metal." >&2
        exit 1
        ;;
esac

os="$(uname -s)"
arch="$(uname -m)"
case "$os-$arch" in
    Linux-x86_64) target="linux-x86_64-musl" ;;
    Linux-aarch64 | Linux-arm64) target="linux-aarch64-musl" ;;
    Darwin-x86_64) target="macos-x86_64" ;;
    Darwin-arm64 | Darwin-aarch64) target="macos-aarch64" ;;
    *)
        echo "ivygrep installer: unsupported platform $os-$arch" >&2
        exit 1
        ;;
esac

base_url="https://github.com/$repository/releases/download/$tag"

has_library() {
    name=$1
    if [ -n "${IVYGREP_CUDA_LIBRARY_PATH:-}" ]; then
        library_dirs="$(printf '%s' "$IVYGREP_CUDA_LIBRARY_PATH" | tr ':' ' ')"
    else
        if command -v ldconfig >/dev/null 2>&1 && ldconfig -p 2>/dev/null | grep -Fq "$name"; then
            return 0
        fi
        library_dirs="$(printf '%s' "${LD_LIBRARY_PATH:-}" | tr ':' ' ')"
        library_dirs="$library_dirs /usr/local/cuda/lib64 /usr/local/cuda/targets/x86_64-linux/lib /usr/lib/x86_64-linux-gnu /lib/x86_64-linux-gnu"
    fi
    # shellcheck disable=SC2086
    for dir in $library_dirs; do
        [ -n "$dir" ] || continue
        if [ -e "$dir/$name" ]; then
            return 0
        fi
    done
    return 1
}

has_cuda_runtime() {
    has_library libcuda.so.1 &&
        has_library libcublas.so.13 &&
        has_library libcublasLt.so.13 &&
        has_library libcurand.so.10
}

has_nvidia_gpu() {
    command -v nvidia-smi >/dev/null 2>&1 &&
        nvidia-smi -L >/dev/null 2>&1
}

nvidia_compute_capability() {
    nvidia-smi --query-gpu=compute_cap --format=csv,noheader,nounits 2>/dev/null |
        sed -n '1{s/[[:space:]]//g;p;}'
}

has_supported_cuda_gpu() {
    capability="$(nvidia_compute_capability)"
    [ -n "$capability" ] || return 1
    major=${capability%%.*}
    minor=${capability#*.}
    case "$major:$minor" in
        7:[5-9] | [89]:* | [1-9][0-9]:*) return 0 ;;
        *) return 1 ;;
    esac
}

missing_cuda_libraries() {
    missing=""
    for library in libcuda.so.1 libcublas.so.13 libcublasLt.so.13 libcurand.so.10; do
        if ! has_library "$library"; then
            missing="${missing}${missing:+, }$library"
        fi
    done
    printf '%s' "$missing"
}

asset_exists() {
    curl -fsI "$base_url/$1" >/dev/null 2>&1
}

accelerator_target=""
accelerator_label=""
case "$accelerator" in
    cuda)
        if [ "$os-$arch" != "Linux-x86_64" ]; then
            echo "ivygrep installer: CUDA archive is only supported on Linux x86_64" >&2
            exit 1
        fi
        if ! has_nvidia_gpu; then
            echo "ivygrep installer: CUDA requested, but no NVIDIA GPU is visible through nvidia-smi" >&2
            exit 1
        fi
        if ! has_supported_cuda_gpu; then
            echo "ivygrep installer: CUDA 13 requires NVIDIA compute capability 7.5 or newer (detected $(nvidia_compute_capability))" >&2
            exit 1
        fi
        if ! has_cuda_runtime; then
            echo "ivygrep installer: CUDA requested, but CUDA 13 runtime is incomplete ($(missing_cuda_libraries))" >&2
            exit 1
        fi
        accelerator_target="linux-x86_64-cuda"
        accelerator_label="CUDA"
        ;;
    metal)
        case "$os-$arch" in
            Darwin-arm64 | Darwin-aarch64)
                accelerator_target="macos-aarch64-metal"
                ;;
            Darwin-x86_64)
                accelerator_target="macos-x86_64-metal"
                ;;
            *)
                echo "ivygrep installer: Metal archive is only supported on macOS" >&2
                exit 1
                ;;
        esac
        accelerator_label="Metal"
        ;;
    auto)
        case "$os-$arch" in
            Linux-x86_64)
                if has_nvidia_gpu; then
                    if ! has_supported_cuda_gpu; then
                        echo "ivygrep installer: NVIDIA compute capability $(nvidia_compute_capability) is unsupported by CUDA 13; using portable CPU archive"
                    elif has_cuda_runtime; then
                        accelerator_target="linux-x86_64-cuda"
                        accelerator_label="CUDA"
                    else
                        echo "ivygrep installer: NVIDIA GPU detected, but CUDA 13 runtime is incomplete ($(missing_cuda_libraries)); using portable CPU archive"
                    fi
                else
                    echo "ivygrep installer: no ready NVIDIA CUDA GPU detected; using portable CPU archive"
                fi
                ;;
            Darwin-arm64 | Darwin-aarch64)
                accelerator_target="macos-aarch64-metal"
                accelerator_label="Metal"
                ;;
        esac
        ;;
esac

if [ -n "$accelerator_target" ]; then
    accelerator_archive="ivygrep-$tag-$accelerator_target.tar.gz"
    if asset_exists "$accelerator_archive"; then
        target="$accelerator_target"
        echo "ivygrep installer: selected $accelerator_label archive ($target)"
    elif [ "$accelerator" = "auto" ]; then
        echo "ivygrep installer: $accelerator_label archive not available for $tag; using $target"
    else
        echo "ivygrep installer: requested $accelerator_label archive is not available for $tag" >&2
        exit 1
    fi
fi

if [ -z "$accelerator_target" ] || [ "$target" != "$accelerator_target" ]; then
    echo "ivygrep installer: selected portable archive ($target)"
fi

archive="ivygrep-$tag-$target.tar.gz"
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
