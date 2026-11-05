# Development

## Repository layout

```text
assets/       Embedded SVG icons and GTK stylesheet
data/        Desktop entry and pinned NVIDIA header version
docs/        Usage guide, development guide and screenshot
licenses/    Third-party license notices
scripts/     Native builder and development launcher
src/         Rust application, driver, settings and installer
tests/       Rust integration tests
install.sh   Portable public curl entry point
```

## Build

Requirements: Rust/Cargo >=1.85, GTK >=4.8 development files, Clang/libclang,
pkg-config, a C linker, Git, Bash and Make. Ubuntu is the preferred optional font.

```sh
make deps
make
make install
```

`make deps` prepares distro packages and, if necessary, a private Rust toolchain.
It may use sudo. Builds and user-prefix installation run as your normal user.
The private toolchain lives in `${XDG_CACHE_HOME:-~/.cache}/nvibrant/toolchain`;
existing Rust installations and shell profiles are preserved.

Dependency setup supports Debian/Ubuntu, Arch, Fedora, openSUSE, Alpine and Void.
On other distributions, install the requirements manually and run `make`.

## NVIDIA headers

`scripts/build.sh` selects the explicit `DRIVER_VERSION`, the running version at
`/sys/module/nvidia/version`, or the pinned `data/nvidia-driver-version` on a
machine without NVIDIA. It downloads matching official NVIDIA headers into
`${XDG_CACHE_HOME:-~/.cache}/nvibrant/headers/VERSION`. No vendor source or Git
submodule is kept inside the repository.

```sh
make DRIVER_VERSION=615.78.08 JOBS=2
NVIBRANT_NVIDIA_HEADERS=/absolute/header-checkout make
```

`build.rs` generates ABI definitions and layout checks with bindgen. Driver
operations use a documented ioctl boundary and Rust-owned device handles.
The executable checks its header version before contacting the driver.

Run `make` once before direct Cargo commands to prepare matching headers.
For a different version, set `NVIBRANT_NVIDIA_HEADERS` to that cached checkout.

## Output and installation

Generated local files stay under `.build/`:

```text
.build/bin/nvibrant    Native executable
.build/target/        Cargo output
```

`BUILD_DIR=/absolute/path` overrides the executable output directory. Downloaded
curl builds reuse `${XDG_CACHE_HOME:-~/.cache}/nvibrant/target/ARCH` to speed up
updates. `make clean` removes Cargo output.

The default installation prefix is `~/.local`. Override with
`make install PREFIX=/absolute/path`. The executable and embedded assets are
installed under `lib/nvibrant`, with launcher symlinks in `bin`, a desktop entry
under `share/applications`, an app icon under `share/icons`, and a login restore
entry in the user's XDG autostart directory. GPL and third-party license notices
are included in the installed payload.

Upgrades stage a complete payload and restore previous files if replacement
fails. User preferences remain separate from installed files. Disabled startup
entries survive upgrades. See [USAGE.md](USAGE.md) for preferences and restoration.

## Checks

```sh
make test
make test-gui
cargo fmt --check
cargo clippy --all-targets -- -D warnings
shellcheck scripts/build.sh scripts/run-dev install.sh
```

Tests cover settings migration, validation, concurrent saves, restore retries,
automatic detection across multiple GPUs, install/upgrade/uninstall, the curl
bootstrap and private Rust fallback. GTK smoke checks use simulated hardware and
do not write preferences.

```sh
./scripts/run-dev --demo
NVIBRANT_SCREENSHOT="$PWD/docs/images/nvibrant.png" make test-gui
.build/bin/nvibrant --list-all-json
```

CI builds and tests natively on x86-64 and ARM64. Hardware behaviour must be
verified separately on NVIDIA; CPU-only runners cannot establish visible colour.
