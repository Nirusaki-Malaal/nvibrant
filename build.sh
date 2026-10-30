#!/usr/bin/env bash
# nVibrant Glass: native Linux source builder and curl-compatible installer.
set -Eeuo pipefail
trap 'printf "nVibrant: command failed (line %s). Installation was not reported as successful.\n" "$LINENO" >&2' ERR

usage() {
    cat <<'HELP'
Usage: bash build.sh [build|install|uninstall|check|deps] [options]

Builds natively on Linux x86_64/amd64 and aarch64/arm64.
Default action: install. Default prefix: ~/.local. No display settings are changed.

  --prefix PATH          Installation prefix (also NVIBRANT_PREFIX)
  --build-dir PATH       Meson build directory (default: SOURCE/build)
  --source-dir PATH      Use an existing checkout instead of downloading one
  --driver-version VER  NVIDIA header tag (default: installed driver, then pinned tag)
  --install-deps         Install dependencies with the system package manager
  --jobs N               Parallel build jobs (default: Meson's default)
  --ref REF              Branch/tag to download (also NVIBRANT_REF)
  -h, --help             Show this help

Environment: NVIBRANT_PYTHON (default /usr/bin/python3), NVIBRANT_PREFIX,
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
        "${elevate[@]}" apt-get install -y git ca-certificates build-essential meson ninja-build \
            python3 python3-packaging python3-gi python3-gi-cairo python3-cairo gir1.2-gtk-4.0
    elif command -v pacman >/dev/null 2>&1; then
        "${elevate[@]}" pacman -S --needed --noconfirm git ca-certificates gcc make meson ninja \
            python python-packaging python-gobject python-cairo gtk4
    elif command -v dnf >/dev/null 2>&1; then
        "${elevate[@]}" dnf install -y git ca-certificates gcc-c++ make meson ninja-build \
            python3 python3-packaging python3-gobject python3-cairo gtk4
    elif command -v zypper >/dev/null 2>&1; then
        "${elevate[@]}" zypper --non-interactive install git ca-certificates gcc-c++ make meson ninja \
            python3 python3-packaging python3-gobject python3-gobject-Gdk python3-cairo typelib-1_0-Gtk-4_0
    else
        die 'No supported package manager found. Install git, a C++ compiler, Meson, Ninja, Python >=3.10, packaging, GTK4, PyGObject and pycairo manually.'
    fi
}

check_deps() {
    need git; need meson; need ninja; need "${CXX:-c++}"
    [[ -x $python ]] || die "Python interpreter is not executable: $python"
    "$python" - <<'PY' || die 'Missing Python/GTK dependencies. Run ./build.sh deps or pass --install-deps (use your distro Python).'
import sys
assert sys.version_info >= (3, 10), 'Python >= 3.10 required'
import packaging, cairo, gi
gi.require_version('Gtk', '4.0')
gi.require_foreign('cairo')
from gi.repository import Gtk
PY
}

get_source() {
    if [[ -z $source_dir && -f ${BASH_SOURCE[0]:-} ]]; then
        local candidate
        candidate=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
        if [[ -f $candidate/meson.build && -f $candidate/scripts/install.py ]]; then
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
    [[ -f $source_dir/meson.build && -f $source_dir/scripts/install.py ]] || die "Not a nVibrant Glass source directory: $source_dir"
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
    local action=install source_dir='' build_dir='' driver_version='' jobs='' headers='' arch=''
    local ref=${NVIBRANT_REF:-codex/frutiger-aero-gtk}
    local prefix=${NVIBRANT_PREFIX:-$HOME/.local} python=${NVIBRANT_PYTHON:-/usr/bin/python3}
    local deps=false
    if [[ ${1:-} != -* && -n ${1:-} ]]; then action=$1; shift; fi
    case $action in build|install|uninstall|check|deps) ;; *) usage >&2; die "Unknown action: $action" ;; esac
    while (($#)); do
        case $1 in
            -h|--help) usage; return ;;
            --install-deps) deps=true; shift ;;
            --prefix|--build-dir|--source-dir|--driver-version|--jobs|--ref)
                (($# >= 2)) && [[ -n $2 ]] || die "Missing value for $1"
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
    printf 'nVibrant Glass · Linux %s · %s\n' "$arch" "$action"
    if [[ $action == deps || $deps == true ]]; then install_deps; fi
    [[ $action != deps ]] || return 0
    if [[ $action == uninstall ]]; then
        get_source
        "$python" "$source_dir/scripts/install.py" uninstall --prefix "$prefix"
        return
    fi
    check_deps
    [[ $action != check ]] || { printf 'Build and GTK runtime dependencies are ready.\n'; return 0; }
    get_source
    if [[ $action == build && -n $temporary_source && -z $build_dir ]]; then
        die 'For a downloaded source build, supply --build-dir /absolute/output/path; use install for curl installation.'
    fi
    build_dir=${build_dir:-$source_dir/build}
    get_headers
    printf 'Using NVIDIA headers %s from %s\n' "$driver_version" "$headers"
    local setup_args=()
    if [[ -f $build_dir/meson-private/coredata.dat ]]; then
        # Existing pre-installer builds do not yet know the new project option.
        if "$python" -c 'import json, sys; raise SystemExit(not any(o["name"] == "nvidia_headers" for o in json.load(open(sys.argv[1]))))' "$build_dir/meson-info/intro-buildoptions.json"; then
            setup_args+=(--reconfigure)
        else
            setup_args+=(--wipe)
        fi
    fi
    meson setup "${setup_args[@]}" "$build_dir" "$source_dir" --buildtype=release "-Dnvidia_headers=$headers"
    local compile_args=(-C "$build_dir")
    [[ -z $jobs ]] || compile_args+=(-j "$jobs")
    meson compile "${compile_args[@]}"
    [[ -x $build_dir/nvibrant ]] || die 'Build completed without a backend executable.'
    if [[ $action == install ]]; then
        "$python" "$source_dir/scripts/install.py" install --source "$source_dir" \
            --build-dir "$build_dir" --prefix "$prefix" --driver-version "$driver_version"
    else
        printf 'Built: %s/nvibrant\n' "$build_dir"
    fi
}

temporary_source=''
temporary_headers=''
trap cleanup EXIT
main "$@"
