#!/usr/bin/env bash
# NVibrant: native Linux source builder and curl-compatible installer.
set -Eeuo pipefail
trap 'printf "nVibrant: command failed (line %s). Installation was not reported as successful.\n" "$LINENO" >&2' ERR

usage() {
    cat <<'HELP'
Usage: bash build.sh [build|install|uninstall|check|deps] [options]

Builds natively on Linux x86_64/amd64 and aarch64/arm64.
Default action: install. Default prefix: ~/.local. No display settings are changed.

  --prefix PATH          Installation prefix (also NVIBRANT_PREFIX)
  --build-dir PATH       Binary output directory (default: SOURCE/build)
  --source-dir PATH      Use an existing checkout instead of downloading one
  --driver-version VER  NVIDIA header tag (default: installed driver, then pinned tag)
  --install-deps         Install dependencies with the system package manager
  --jobs N               Parallel build jobs (default: 2)
  --ref REF              Branch/tag to download (also NVIBRANT_REF)
  -h, --help             Show this help

Environment: NVIBRANT_NVIDIA_HEADERS, NVIBRANT_PREFIX,
NVIBRANT_REF, XDG_CACHE_HOME. Prefix and build directory must be absolute paths.
Package installation is opt-in; only that step may use sudo.
HELP
}

die() { printf 'nVibrant: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "Missing $1. Run ./build.sh deps or pass --install-deps."; }

architecture() {
    [[ $(uname -s) == Linux ]] || die 'Only Linux is supported.'
    case $(uname -m) in
        x86_64|amd64) arch=x86_64 ;;
        aarch64|arm64) arch=arm64 ;;
        *) die "Unsupported architecture: $(uname -m). Use x86-64 or ARM64 Linux." ;;
    esac
}

install_deps() {
    local elevate=()
    if [[ $(id -u) != 0 ]]; then need sudo; elevate=(sudo); fi
    if command -v apt-get >/dev/null 2>&1; then
        "${elevate[@]}" apt-get update
        "${elevate[@]}" apt-get install -y git ca-certificates build-essential cargo rustc clang libclang-dev pkg-config libgtk-4-dev
    elif command -v pacman >/dev/null 2>&1; then
        "${elevate[@]}" pacman -S --needed --noconfirm git ca-certificates base-devel rust clang pkgconf gtk4
    elif command -v dnf >/dev/null 2>&1; then
        "${elevate[@]}" dnf install -y git ca-certificates gcc make cargo rust clang clang-devel pkgconf-pkg-config gtk4-devel
    elif command -v zypper >/dev/null 2>&1; then
        "${elevate[@]}" zypper --non-interactive install git ca-certificates gcc make cargo rust clang clang-devel pkg-config gtk4-devel
    else
        die 'No supported package manager found. Install Git, Rust/Cargo >=1.85, clang/libclang, pkg-config and GTK4 development headers manually.'
    fi
}

check_deps() {
    need git; need cargo; need rustc; need pkg-config; need clang
    pkg-config --atleast-version=4.8 gtk4 || die 'GTK >=4.8 development files are required.'
    printf 'Rust/Cargo and GTK runtime dependencies are ready.\n'

}

get_source() {
    if [[ -z $source_dir && -f ${BASH_SOURCE[0]:-} ]]; then
        local candidate
        candidate=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
        if [[ -f $candidate/Cargo.toml && -f $candidate/src/main.rs ]]; then
            source_dir=$candidate
        fi
    fi
    if [[ -z $source_dir ]]; then
        need git
        temporary_source=$(mktemp -d -t nvibrant-source.XXXXXXXX)
        git clone --filter=blob:none --depth 1 --branch "$ref" \
            https://github.com/Nirusaki-Malaal/nvibrant.git "$temporary_source/source"
        source_dir=$temporary_source/source
    fi
    [[ -f $source_dir/Cargo.toml && -f $source_dir/src/main.rs ]] || die "Not a NVibrant source directory: $source_dir"
    source_dir=$(cd -- "$source_dir" && pwd)
}

get_headers() {
    local tag cache_dir
    if [[ -z $driver_version ]]; then
        if [[ -r /sys/module/nvidia/version ]]; then
            read -r driver_version < /sys/module/nvidia/version
        else
            read -r driver_version < "$source_dir/data/nvidia-driver-version"
            printf 'No running NVIDIA driver; building against pinned headers %s.\n' "$driver_version"
        fi
    fi
    [[ $driver_version =~ ^[0-9]+(\.[0-9]+){1,3}$ ]] || die "Invalid NVIDIA driver version: $driver_version"
    tag=$(git -C "$source_dir/open-gpu" describe --tags --exact-match 2>/dev/null || true)
    if [[ $tag == "$driver_version" && -f $source_dir/open-gpu/src/nvidia-modeset/interface/nvkms-api.h ]]; then
        headers=open-gpu
        return
    fi
    cache_dir=${XDG_CACHE_HOME:-$HOME/.cache}/nvibrant/headers
    mkdir -p -- "$cache_dir"
    headers=$cache_dir/$driver_version
    if [[ ! -f $headers/.nvibrant-headers-ready ]]; then
        # Publish the cache only after clone and sparse checkout both succeed.
        temporary_headers=$(mktemp -d "$cache_dir/.download.XXXXXXXX")
        git clone --filter=blob:none --sparse --depth 1 --branch "$driver_version" \
            https://github.com/NVIDIA/open-gpu-kernel-modules.git "$temporary_headers/source"
        git -C "$temporary_headers/source" sparse-checkout set \
            src/nvidia-modeset/interface src/nvidia-modeset/include src/common/inc \
            src/common/sdk/nvidia/inc src/common/unix/common/inc
        touch "$temporary_headers/source/.nvibrant-headers-ready"
        [[ ! -e $headers ]] || die "Incomplete header cache exists: $headers. Move it aside and retry."
        mv -- "$temporary_headers/source" "$headers"
    fi
}

cleanup() {
    [[ -z ${temporary_source:-} ]] || rm -rf -- "$temporary_source"
    [[ -z ${temporary_headers:-} ]] || rm -rf -- "$temporary_headers"
    return 0
}

main() {
    local action=install source_dir='' build_dir='' driver_version='' jobs=2 headers='' arch=''
    local ref=${NVIBRANT_REF:-main}
    local prefix=${NVIBRANT_PREFIX:-$HOME/.local}
    local deps=false
    if [[ ${1:-} != -* && -n ${1:-} ]]; then action=$1; shift; fi
    case $action in build|install|uninstall|check|deps) ;; *) usage >&2; die "Unknown action: $action" ;; esac
    while (($#)); do
        case $1 in
            -h|--help) usage; return ;;
            --install-deps) deps=true; shift ;;
            --prefix|--build-dir|--source-dir|--driver-version|--jobs|--ref)
                if (($# < 2)) || [[ -z ${2:-} ]]; then die "Missing value for $1"; fi
                case $1 in
                    --prefix) prefix=$2 ;; --build-dir) build_dir=$2 ;;
                    --source-dir) source_dir=$2 ;; --driver-version) driver_version=$2 ;;
                    --jobs) jobs=$2 ;; --ref) ref=$2 ;;
                esac
                shift 2 ;;
            *) die "Unknown option: $1" ;;
        esac
    done
    [[ $prefix == /* && $prefix != / ]] || die 'Prefix must be an absolute directory other than /.'
    [[ -z $build_dir || $build_dir == /* ]] || die 'Build directory must be absolute.'
    [[ -z $jobs || $jobs =~ ^[1-9][0-9]*$ ]] || die '--jobs must be a positive integer.'
    architecture
    printf 'NVibrant · Linux %s · %s\n' "$arch" "$action"
    if [[ $action == deps || $deps == true ]]; then install_deps; fi
    [[ $action != deps ]] || return 0
    if [[ $action == uninstall ]]; then
        [[ -x $prefix/lib/nvibrant/nvibrant ]] || die 'No Rust NVibrant installation found at this prefix.'
        "$prefix/lib/nvibrant/nvibrant" --uninstall "$prefix"
        return
    fi
    check_deps
    [[ $action != check ]] || { printf 'Build and GTK runtime dependencies are ready.\n'; return 0; }
    get_source
    if [[ $action == build && -n $temporary_source && -z $build_dir ]]; then
        die 'For a downloaded source build, supply --build-dir /absolute/output/path; use install for curl installation.'
    fi
    build_dir=${build_dir:-$source_dir/build}
    if [[ -n ${NVIBRANT_NVIDIA_HEADERS:-} ]]; then
        headers=$NVIBRANT_NVIDIA_HEADERS
    else
        get_headers
    fi
    [[ $headers == /* ]] || headers=$source_dir/$headers
    printf 'Using NVIDIA headers %s from %s\n' "$driver_version" "$headers"
    export NVIBRANT_NVIDIA_HEADERS=$headers
    (cd -- "$source_dir" && cargo build --release --locked --target-dir "$source_dir/target" -j "$jobs")
    mkdir -p -- "$build_dir"
    cp -- "$source_dir/target/release/nvibrant" "$build_dir/nvibrant"
    if [[ $action == install ]]; then
        "$build_dir/nvibrant" --install "$prefix"
    else
        printf 'Built: %s/nvibrant\n' "$build_dir"
    fi
}

temporary_source=''
temporary_headers=''
trap cleanup EXIT
main "$@"
