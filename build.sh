#!/usr/bin/env bash
# NVibrant: native Linux source builder and curl-compatible installer.
set -Eeuo pipefail
trap 'printf "NVibrant: command failed (line %s). Installation was not reported as successful.\n" "$LINENO" >&2' ERR

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

die() { printf 'NVibrant: %s\n' "$*" >&2; exit 1; }
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
        "${elevate[@]}" apt-get install -y git ca-certificates build-essential curl clang libclang-dev pkg-config libgtk-4-dev librsvg2-common
    elif command -v pacman >/dev/null 2>&1; then
        "${elevate[@]}" pacman -S --needed --noconfirm git ca-certificates base-devel curl clang pkgconf gtk4 librsvg
    elif command -v dnf >/dev/null 2>&1; then
        "${elevate[@]}" dnf install -y git ca-certificates gcc make curl clang clang-devel pkgconf-pkg-config gtk4-devel librsvg2
    elif command -v zypper >/dev/null 2>&1; then
        "${elevate[@]}" zypper --non-interactive install git ca-certificates gcc make curl clang clang-devel pkg-config gtk4-devel gdk-pixbuf-loader-rsvg
    elif command -v apk >/dev/null 2>&1; then
        "${elevate[@]}" apk add --no-cache git ca-certificates curl build-base clang clang-dev llvm-dev pkgconf gtk4.0-dev librsvg
    elif command -v xbps-install >/dev/null 2>&1; then
        "${elevate[@]}" xbps-install -Sy git ca-certificates curl base-devel clang pkg-config gtk4-devel librsvg
    else
        die 'No supported package manager found. Install Git, Rust/Cargo >=1.85, clang/libclang, pkg-config and GTK4 development headers manually.'
    fi
}

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --silent --show-error --retry 3 --connect-timeout 20 "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$1" -O "$2"
    else
        die 'curl or wget is required to download the Rust toolchain.'
    fi
}

rust_ready() {
    command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1 || return 1
    local version major minor
    version=$(rustc --version) || return 1
    version=${version#rustc }; version=${version%% *}
    major=${version%%.*}; version=${version#*.}; minor=${version%%.*}
    [[ $major =~ ^[0-9]+$ && $minor =~ ^[0-9]+$ ]] || return 1
    ((major > 1 || (major == 1 && minor >= 85)))
}

ensure_rust() {
    if rust_ready; then return; fi
    [[ $deps == true ]] || die 'Rust/Cargo >=1.85 is required. Run install.sh, or build.sh deps to prepare dependencies.'
    local toolchain_cache=${XDG_CACHE_HOME:-$HOME/.cache}/nvibrant/toolchain
    # This modifies only the current installer process, not the user's profile,
    # existing Rust installation, system interpreter or global toolchain defaults.
    export CARGO_HOME=$toolchain_cache/cargo RUSTUP_HOME=$toolchain_cache/rustup
    export PATH="$CARGO_HOME/bin:$PATH"
    if rust_ready; then return; fi
    mkdir -p -- "$toolchain_cache"
    printf 'Preparing a private Rust toolchain in %s\n' "$toolchain_cache"
    local rustup_script
    rustup_script=$(mktemp "$toolchain_cache/rustup-init.XXXXXXXX")
    fetch https://sh.rustup.rs "$rustup_script"
    sh "$rustup_script" -y --no-modify-path --profile minimal --default-toolchain stable
    rm -f -- "$rustup_script"
    rust_ready || die 'The Rust toolchain could not be prepared.'
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
    if [[ $action == deps || $deps == true ]]; then install_deps; deps=true; fi
    if [[ $action == deps ]]; then ensure_rust; return; fi
    if [[ $action == uninstall ]]; then
        [[ -x $prefix/lib/nvibrant/nvibrant ]] || die 'No Rust NVibrant installation found at this prefix.'
        "$prefix/lib/nvibrant/nvibrant" --uninstall "$prefix"
        return
    fi
    ensure_rust
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
    local target_dir=$source_dir/target
    if [[ -n $temporary_source ]]; then
        target_dir=${XDG_CACHE_HOME:-$HOME/.cache}/nvibrant/target/$arch
    fi
    (cd -- "$source_dir" && cargo build --release --locked --target-dir "$target_dir" -j "$jobs")
    mkdir -p -- "$build_dir"
    cp -- "$target_dir/release/nvibrant" "$build_dir/nvibrant"
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
