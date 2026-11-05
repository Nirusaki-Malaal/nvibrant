#!/bin/sh
# NVibrant's portable curl entry point. The build/install implementation is build.sh.
set -eu
fail() { printf 'NVibrant: %s\n' "$*" >&2; exit 1; }
fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --silent --show-error --retry 3 --connect-timeout 20 "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$1" -O "$2"
    else
        fail 'Install curl or wget, then run the installer again.'
    fi
}
case ${1:-} in
    -h|--help)
        cat <<'HELP'
NVibrant installer for Linux x86-64 and ARM64

curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/main/install.sh | sh

Options: --prefix /absolute/path, --ref BRANCH_OR_TAG, --driver-version VERSION,
         --jobs N, --source-dir /checkout, --no-deps (must be the first option).

The default installs build dependencies, compiles for your NVIDIA driver and
installs the app for your user. Package installation may use sudo; Rust fallback
is kept in a private user cache. Use --no-deps when dependencies are already ready.
HELP
        exit 0 ;;
esac
[ "$(uname -s)" = Linux ] || fail 'NVibrant requires Linux and a Wayland-capable NVIDIA driver.'
case $(uname -m) in x86_64|amd64|aarch64|arm64) ;; *) fail 'Supported CPU architectures: x86-64 and ARM64.' ;; esac
install_deps=yes
if [ "${1:-}" = --no-deps ]; then install_deps=no; shift; fi
ref=${NVIBRANT_REF:-main}
expect_ref=no
for argument in "$@"; do
    if [ "$expect_ref" = yes ]; then ref=$argument; expect_ref=no; fi
    if [ "$argument" = --ref ]; then expect_ref=yes; fi
done
[ "$expect_ref" = no ] || fail '--ref needs a branch or tag.'
case $ref in ''|*[!A-Za-z0-9._/-]*) fail 'Invalid source ref.' ;; esac
if ! command -v bash >/dev/null 2>&1; then
    # Minimal Alpine installations commonly provide ash but not Bash.
    if [ "$install_deps" = yes ] && command -v apk >/dev/null 2>&1; then
        if [ "$(id -u)" -eq 0 ]; then apk add --no-cache bash; else
            command -v sudo >/dev/null 2>&1 || fail 'Install bash with apk, then retry.'
            sudo apk add --no-cache bash
        fi
    else
        fail 'Install Bash first, then run the installer again.'
    fi
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/nvibrant-bootstrap.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT HUP INT TERM
fetch "https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/$ref/build.sh" "$work/build.sh"
# Keep argument boundaries intact, including installation prefixes with spaces.
if [ "$install_deps" = yes ]; then
    bash "$work/build.sh" install --install-deps "$@"
else
    bash "$work/build.sh" install "$@"
fi
