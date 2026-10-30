# nVibrant Glass

A native GTK 4 interface for nVibrant, styled with glossy black glass panels, silver highlights,
soft spectral accents and Apple-inspired restraint.

![nVibrant Glass demo](screenshots/aero.png)

## Run on Arch Linux

Install the native tools and GTK Python bindings:

```sh
sudo pacman -S --needed gtk4 python-gobject python-cairo python-packaging meson ninja gcc git
```

Clone **this fork**, including NVIDIA's headers, and build the native backend:

```sh
git clone --branch codex/frutiger-aero-gtk --recurse-submodules https://github.com/Nirusaki-Malaal/nvibrant.git
cd nvibrant
meson setup build --buildtype=release
meson compile -C build
./run-gtk
```

This fork pins NVIDIA headers to **615.78.08**, the version used for hardware
verification. Use headers matching your NVIDIA driver if discovery reports an
ioctl/version mismatch. For example (substitute your installed driver version):

```sh
git -C open-gpu fetch origin tag "$(cat /sys/module/nvidia/version)"
git -C open-gpu checkout "$(cat /sys/module/nvidia/version)"
meson compile -C build
```

To preview the interface without building the backend or accessing hardware:

```sh
python3 -m nvibrant.gui --demo
```

Other distributions need GTK 4, PyGObject with Cairo integration, pycairo and
Python packaging; package names vary. Run with the distro Python that provides
`gi`, rather than an isolated Python environment without the GTK bindings.

## Use

1. The app discovers NVIDIA outputs **without changing any colours**.
2. Choose a preset or move each display's slider. Keyboard arrows work too.
3. Click **Apply colours**. Changes are explicit; moving a slider alone does
   not change your screen.
4. **Reset to natural**, then Apply, restores zero vibrance to connected outputs
   on the selected GPU.
5. For a second NVIDIA GPU, change the index and click **Refresh**. Refresh
   discards pending choices and disables stale controls if detection fails.

Values range from −1024 (grayscale) through 0 (natural) to 1023 (maximum).
Initial sliders show **pending zero values**, not a reading of current hardware
settings. Apply sends every connected output's displayed value together.
Output numbering uses NVIDIA's physical connector order, preserving gaps for
unplugged ports. Settings can reset after reboot, sleep or driver/display changes;
there is no automatic persistence in this first version.

On hybrid laptops, an internal screen driven by the integrated GPU is not
controllable through nVibrant. Expand Details for backend errors. The GUI never
asks for sudo or downloads code at launch. A request has a 15 second timeout and
runs off the GTK thread; the window waits for an active request before closing.

## Desktop launcher

The wheel exposes `nvibrant-gtk`. If using the source checkout instead, create a
small launcher at `~/.local/bin/nvibrant-gtk` that changes into your checkout and
executes `/usr/bin/python3 -m nvibrant.gui`. Then copy
`data/io.github.nvibrant.Aero.desktop` into `~/.local/share/applications/`.

## Backend changes

This fork adds `--list-json`, a read-only operation returning all ports with
zero-based `index`, `display`, `connector`, `type` and `connected` fields.
The global index is the positional CLI argument index, including disconnected
ports and multiple display groups. Discovery skips SET_DPY_ATTRIBUTE entirely.
Partial enumeration and per-output apply failures return a nonzero exit code.

The GUI deliberately never falls back to `/usr/bin/nvibrant`: upstream parses
unknown arguments as zero, so using its binary for discovery would reset colours.
Use this fork's local Meson build or this fork's bundled wheel binaries.
The original `nvibrant` CLI remains available and unchanged in usage.

## Checks

```sh
python3 -m unittest discover -s tests -v
xvfb-run -a env GDK_BACKEND=x11 GSK_RENDERER=cairo python3 tests/smoke_gui.py
```

The GTK smoke check exercises discovery, sliders, apply/reset, GPU discovery
failure and recovery using the demo backend; it renders the actual widgets to
`screenshots/aero.png`. Real hardware discovery can be checked separately:

```sh
./build/nvibrant --list-json
```
