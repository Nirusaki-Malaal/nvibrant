# Building NVibrant in Rust

NVibrant 2 is a Rust application. Cargo builds one executable containing the
GTK interface, direct NVIDIA ioctl backend, settings persistence, login restore
and installer. No Python interpreter or separate C++ backend is used.

## Dependencies

- Rust and Cargo >=1.85 (2024 edition).
- GTK >=4.8 development headers, pkg-config and a system C linker.
- Clang and libclang for build-time NVIDIA header bindings.
- Git, Bash and Make; Xvfb/xauth for GUI smoke tests only.
- A compatible loaded NVIDIA driver for real hardware operations.

`make deps` installs distro packages on Debian/Ubuntu, Arch, Fedora or openSUSE.
It may use sudo. Other build and user-prefix installation steps run as your user.
Distro packages can lag behind Rust crate requirements; update Rust if Cargo
reports an unsupported compiler version. Ubuntu font is optional; GTK falls back
to Cantarell or the system sans-serif font.

## Build and install

```bash
make check
make JOBS=2
make test
make test-gui
make install PREFIX="$HOME/.local"
```

`build.sh` selects headers in this order: explicit `--driver-version`, the running
version at `/sys/module/nvidia/version`, then `data/nvidia-driver-version` on
machines without NVIDIA. An exact matching `open-gpu` submodule is reused;
otherwise a sparse checkout is cached at
`${XDG_CACHE_HOME:-~/.cache}/nvibrant/headers/VERSION`.

Automatic GUI discovery reads GPU device minors from `/proc/driver/nvidia/gpus/`,
then queries connected outputs on each GPU. A single window controls them all;
GPU indices are exposed only in CLI commands and error diagnostics.

`build.rs` generates ABI structures, enums, ioctl constants and compile-time layout
assertions from those headers with bindgen. Rust owns file descriptors and frees
NVIDIA device handles on drop. The only unsafe operation in application code is
the documented synchronous ioctl boundary. The pinned version is the currently
tested ABI; other driver versions need their own build and hardware verification.

Override the version or provide a local header checkout:

```bash
make DRIVER_VERSION=615.78.08
NVIBRANT_NVIDIA_HEADERS=/absolute/open-gpu-source make
```

Direct Cargo builds use the `open-gpu` directory by default, so initialize its
pinned submodule first (`git submodule update --init`) or set
`NVIBRANT_NVIDIA_HEADERS`. `build.sh` fetches headers automatically and does not
switch your tracked submodule. Cargo uses the committed lockfile. It builds for
the host architecture; the shell installer accepts Linux x86-64 and ARM64.

`BUILD_DIR=/absolute/path` changes where the final executable is copied (default
`build/nvibrant`). Cargo's intermediate output remains under `target/`.
`make clean` removes Cargo output. `./run-gtk --demo` launches the built executable.

The curl installer downloads this repository into a temporary checkout and builds
it. `--ref TAG_OR_BRANCH` selects a ref; `--source-dir /checkout` selects existing
sources. For a downloaded build-only command, specify an absolute `--build-dir`
so the output survives cleanup. Flags are described by `bash build.sh --help`.

## Installed layout

```text
~/.local/lib/nvibrant/nvibrant                  Rust executable, embedded theme/icon
~/.local/lib/nvibrant/.nvibrant-install.json    Installation metadata
~/.local/lib/nvibrant/license.txt
~/.local/bin/nvibrant                          Symlink to executable
~/.local/bin/nvibrant-gtk                      Symlink to executable
~/.local/share/applications/io.github.nvibrant.Aero.desktop
~/.local/share/icons/hicolor/scalable/apps/io.github.nvibrant.Aero.svg
~/.config/autostart/io.github.nvibrant.Aero.Restore.desktop
```

Custom prefixes must be absolute and writable. The menu may need their `share`
directory in `XDG_DATA_DIRS`. The login entry uses an absolute executable path,
so it does not depend on shell profiles or PATH. It is installed for the current
user under an absolute `XDG_CONFIG_HOME`, falling back to `~/.config`.

The Rust installer recognizes the previous Python version's marker and upgrades
it in place. It stages the new payload and restores the old payload and managed
files if replacement fails. Unrelated installations and autostart entries are
refused. A desktop-disabled login entry survives upgrades. Installation does not
apply colours. Saved settings retain the existing version-1 JSON schema, physical
connector identities, atomic replacement and cross-process merge lock.

## Verification

`make test` covers legacy settings migration, validation, corrupt-file protection,
concurrent saves, restore retries, selective restore, numeric CLI validation,
installation in prefixes with spaces, upgrades, disabled startup entries and
uninstall preserving user files. `make test-gui` exercises actual Rust/GTK widgets
under Xvfb, with no hardware changes or preference writes.

```bash
NVIBRANT_SCREENSHOT="$PWD/screenshots/rust.png" make test-gui
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Real hardware auto-detection: `build/nvibrant --list-all-json`.
Single-GPU discovery: `build/nvibrant --list-json`. Real restoration:
`~/.local/bin/nvibrant-gtk --restore`. These must be checked separately on NVIDIA
hardware; CPU-only CI cannot establish driver compatibility or visible colour.

## Uninstall

```bash
make uninstall
# Without a checkout:
~/.local/bin/nvibrant --uninstall "$HOME/.local"
```

Uninstall removes the recognized payload, two launcher symlinks, icon, desktop
entry and matching login entry. Preferences and cached NVIDIA headers remain.
