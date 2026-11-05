<p align="center">
  <img src="assets/io.github.nvibrant.Aero.svg" alt="NVibrant" width="88">
</p>
<h1 align="center">NVibrant</h1>
<p align="center">Tool for Wayland to change vibrancy on NVIDIA GPUs.</p>
<p align="center">
  <a href="https://github.com/Nirusaki-Malaal/nvibrant/actions/workflows/native-build.yml"><img alt="Build" src="https://img.shields.io/github/actions/workflow/status/Nirusaki-Malaal/nvibrant/native-build.yml?branch=main&amp;label=build"></a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-native-dea584">
  <img alt="Linux Wayland" src="https://img.shields.io/badge/Linux-Wayland-76b900">
  <a href="license.txt"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-white"></a>
</p>

NVibrant gives NVIDIA-driven displays a little more colour—or a lot less.
A native Rust app with a minimal black interface, square controls, an Ubuntu
wordmark, and automatic screen detection. Maintained by **Nirusaki-Malaal**.

![NVibrant interface](screenshots/rust.png)

## Install

Run this in a terminal as your normal user:

```sh
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/main/install.sh | sh
```

The installer detects your Linux architecture, installs build dependencies,
selects headers matching your NVIDIA driver, builds NVibrant and adds its app
menu entry. It supports **x86-64 and ARM64**, with dependency setup for
Debian/Ubuntu, Arch, Fedora, openSUSE, Alpine and Void Linux.

System package installation may ask for sudo. If a recent Rust toolchain is
missing, the installer prepares one in a private user cache. Your shell profile
and existing toolchain settings are left alone. The app installs into `~/.local`.

Launch **NVibrant** from your app menu, or run:

```sh
~/.local/bin/nvibrant-gtk
```

Already have the dependencies? Skip their installation:

```sh
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/nvibrant/main/install.sh | sh -s -- --no-deps
```

For a custom location, append `--prefix /absolute/path`. Run the same install
command to **update** NVibrant; saved colour preferences are retained.

## What it does

- **Finds NVIDIA screens automatically.** Connected outputs from multiple NVIDIA
  GPUs appear together, with no GPU selector to configure.
- **Controls each screen separately.** Choose Natural, Fresh, Vivid or Luminous,
  or adjust the slider from grayscale to maximum vibrancy.
- **Remembers your colours.** Apply saves preferences per GPU and output connector.
- **Restores them after reboot.** A background login task restores saved colours
  when the desktop starts, without opening a window.

## Use

Move a slider or choose a preset, then click **Apply**. Changes take effect only
when applied. **Reset** selects natural colours; Apply commits that choice.
**Refresh** rescans screens while keeping pending changes for unchanged outputs.

Opening the app reads saved preferences without changing the display. Driver
and save errors appear in a dialog. For a hardware-free preview, run
`nvibrant-gtk --demo`.

## Requirements

- Linux with a Wayland desktop and a loaded NVIDIA driver.
- A display driven directly by NVIDIA. A laptop panel wired to an integrated GPU
  cannot be adjusted through NVIDIA's controls.
- GTK 4.8 or newer. The installer handles development packages on supported distros.
- Internet access and disk space for the first source build.

The universal installer builds for the host architecture and driver rather than
assuming one prebuilt binary works everywhere. On other Linux distributions,
prepare the dependencies listed in [BUILDING.md](BUILDING.md), then use `--no-deps`.
Re-run the installer after a driver update if NVibrant reports a version mismatch.

## CLI

```sh
nvibrant                       # Open the app
nvibrant --list-all-json        # Discover connected NVIDIA screens
nvibrant --list-json --gpu 0    # Inspect all outputs on one GPU
nvibrant 512 512                # Set positional output values directly
nvibrant --skip 512             # Leave the first output unchanged
nvibrant-gtk --restore          # Restore saved preferences without a window
nvibrant-gtk --demo             # Preview without touching hardware
nvibrant --version
```

Vibrancy ranges from **−1024** (grayscale), through **0** (natural), to
**1023** (maximum). Positional CLI values include disconnected ports; inspect
`--list-json` to see their order. CLI changes do not overwrite GUI preferences.
`NVIDIA_GPU` and the `ATTRIBUTE` dithering controls are also supported.

## Preferences and startup

Preferences live in `~/.config/nvibrant/settings.json`, or beneath an absolute
`XDG_CONFIG_HOME`. Existing settings are carried forward during upgrades.
Unapplied changes and failed driver requests are never saved.

The login task restores connected outputs that have saved values, retrying while
the driver becomes ready. Disable **NVibrant saved colours** in your desktop's
Startup Applications settings to opt out. Desktops that do not run XDG autostart
entries can run `nvibrant-gtk --restore` from their own startup configuration.
After sleep or an in-session driver reset, use that command or Apply again.

## Build from source

```sh
git clone https://github.com/Nirusaki-Malaal/nvibrant.git
cd nvibrant
make deps
make
make install
```

The application, GTK interface, NVIDIA backend, settings store, login restore and
installer are written in Rust. Theme and SVG assets are embedded in the executable.
See [BUILDING.md](BUILDING.md) for dependencies, driver headers and build options,
and [GUI.md](GUI.md) for detailed controls.

```sh
make test
make test-gui
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Uninstall

```sh
~/.local/bin/nvibrant --uninstall "$HOME/.local"
```

This removes the app, menu entry, icon and managed login task. Your saved
preferences remain available for a later reinstall.

## License

[MIT](license.txt). See [NOTICE.md](NOTICE.md) for acknowledgements.
NVIDIA is a trademark of NVIDIA Corporation.
