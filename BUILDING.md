# Build and installation

`build.sh` is the single implementation behind Make and curl installation. It
builds from source on **Linux x86_64/amd64 or aarch64/arm64**; it does not download
an x86 binary onto an ARM system or install any NVIDIA kernel driver.

## Requirements

- Bash, Git, a C++ compiler, Meson, Ninja, Python 3.10 or later.
- Python `packaging`, PyGObject (`gi`) with GTK 4 and Cairo integration, pycairo.
- A compatible NVIDIA driver driving the display for real hardware controls.
- Xvfb and xauth for `make test-gui` only.

Use `make deps` or `./build.sh deps` to install dependencies on Debian/Ubuntu,
Arch, Fedora or openSUSE. This explicitly opts into system package installation;
normal builds and user-prefix installs do not invoke sudo.

For example, on Arch:

```bash
sudo pacman -S --needed git gcc make meson ninja python python-packaging python-gobject python-cairo gtk4
```

On Debian/Ubuntu:

```bash
sudo apt-get install git build-essential meson ninja-build python3 python3-packaging python3-gi python3-gi-cairo python3-cairo gir1.2-gtk-4.0
```

Use the distro Python that has `gi` available. The default is `/usr/bin/python3`;
override with `NVIBRANT_PYTHON=/absolute/python` or `make PYTHON=/absolute/python`.
The installed launcher records that interpreter.

## Local checkout

```bash
make check
make JOBS=4
make install PREFIX="$HOME/.local"
make test
make test-gui
```

Direct equivalents:

```bash
./build.sh check
./build.sh build --jobs 4
./build.sh install --prefix "$HOME/.local"
```

`make` creates `build/nvibrant` by default. A custom `BUILD_DIR` must be absolute;
`make install BUILD_DIR=/absolute/build` installs the binary from that build.
`./run-gtk` uses the default `build` directory when running from the checkout.

## Curl bootstrap

```bash
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/main/build.sh | bash -s -- install --install-deps
```

The script clones this fork into a temporary directory, builds it, copies the
complete payload into the chosen prefix, and removes the temporary checkout.
You may download the script to a file and run it separately as shown in the
[README](readme.md). `--ref BRANCH_OR_TAG` or `NVIBRANT_REF` chooses another source
branch/tag in this fork. `--source-dir /checkout` explicitly uses local sources.
For a downloaded **build-only** run, specify an absolute `--build-dir` so the
compiled output survives cleanup. Installation is the default action.

## NVIDIA headers

Headers are selected in this order:

1. `--driver-version` / Make's `DRIVER_VERSION` override.
2. `/sys/module/nvidia/version` from the running driver.
3. `data/nvidia-driver-version` when building on a system without NVIDIA.

If the local `open-gpu` submodule already has the exact tag and headers, it is
reused. Otherwise, the script performs a sparse checkout of the matching NVIDIA
header tag into `${XDG_CACHE_HOME:-~/.cache}/nvibrant/headers/VERSION`. It never
switches your tracked submodule to another driver version. Failed downloads are
not marked as ready. Network access is required on the first uncached build.

For drivers whose matching open-gpu header tag is unavailable, choose an available
compatible version explicitly; hardware compatibility must be checked separately.
Rebuild and reinstall after a driver upgrade if discovery reports an ioctl error.

For direct Meson users:

```bash
meson setup build --buildtype=release -Dnvidia_headers=/absolute/open-gpu-source
meson compile -C build
```

Use `-Dnvidia_headers=open-gpu` for the submodule in this checkout. Relative paths
are relative to the source root. Meson's own install target installs only the
native CLI; use Make or `build.sh install` for the complete GTK application.

## Installed layout

With the default prefix `~/.local`:

```text
bin/nvibrant-gtk                         GUI launcher
bin/nvibrant                             Native CLI symlink
lib/nvibrant/build/nvibrant               Private, driver-matched native backend
lib/nvibrant/nvibrant/                    Python package and glass theme
lib/nvibrant/.nvibrant-install.json       Installation/driver metadata
share/applications/io.github.nvibrant.Aero.desktop
share/icons/hicolor/scalable/apps/io.github.nvibrant.Aero.svg
```

The GUI resolves its private backend beside the package; an upstream system CLI
is never used for discovery. You can remove the original checkout after install.
The installer does not apply colours, enable autostart, change your shell profile,
or install Python modules into the system site-packages directory.

A recognized installation can be upgraded in place. Existing unrelated launchers
or an unrecognized payload directory cause installation to fail rather than be
overwritten. The app payload is staged before replacement. Custom prefixes must
be absolute and writable; choose a user-owned prefix unless intentionally doing
a system installation.

## Remove

```bash
make uninstall
# A custom install must use the same prefix:
./build.sh uninstall --prefix /absolute/prefix
```

Without a checkout:

```bash
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/main/build.sh | bash -s -- uninstall
```

Uninstall removes this application's payload, two launchers and desktop file.
The custom app icon is also removed and desktop/icon caches are refreshed when
the corresponding tools are available. Other prefix contents, cached headers and user preferences in
`~/.config/nvibrant/settings.json` (or `XDG_CONFIG_HOME`) remain.

## Verification

Local checks are `make test` and `make test-gui`. The installer tests cover
argument rejection, stdin execution, prefixes with spaces, installed launchers,
failed-copy rollback and removal without touching unrelated files.

A ready-to-enable CI template is included at [`ci/native-build.yml`](ci/native-build.yml).
Copy it to `.github/workflows/native-build.yml` to run on native GitHub-hosted
**ubuntu-24.04 (x86-64)** and **ubuntu-24.04-arm (ARM64)** runners. Publishing an
active workflow with a GitHub OAuth token requires its `workflow` scope.

Each job builds, checks the ELF machine type, runs backend/installer tests,
exercises GTK from both the checkout and the installed payload, validates the
desktop entry, and uninstalls. CPU-only build/install checks do not verify
colour application on ARM64 NVIDIA hardware.
