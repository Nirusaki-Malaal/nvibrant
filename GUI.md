# nVibrant Glass

A native GTK 4 interface for nVibrant, styled with glossy black glass panels, silver highlights,
soft spectral accents and Apple-inspired restraint.

![nVibrant Glass demo](screenshots/aero.png)

## Build and install

Use the [curl installer or Makefile](readme.md#install-with-curl). Both compile
natively on x86-64 and ARM64 Linux and install the GUI, native backend and desktop
launcher without modifying Python site-packages.

From this checkout:

```sh
make check
make
make install
~/.local/bin/nvibrant-gtk
```

`make deps` installs distro packages if needed. The universal `build.sh` selects
headers matching the running NVIDIA driver automatically; use
`make DRIVER_VERSION=615.78.08` to target a particular driver. On a machine without
a running NVIDIA driver it falls back to `data/nvidia-driver-version`.

Preview without building or accessing hardware:

```sh
./run-gtk --demo
```

See [BUILDING.md](BUILDING.md) for custom prefixes, dependencies, uninstalling and
how the installed payload is laid out.

## Use

1. The app discovers NVIDIA outputs **without changing any colours**.
2. Choose a preset or move each display's slider. Keyboard arrows work too.
3. Click **Apply colours**. Changes are explicit; moving a slider alone does
   not change your screen.
4. **Reset to natural**, then Apply, restores zero vibrance to connected outputs
   on the selected GPU.
5. For a second NVIDIA GPU, change the index and click **Refresh**. Refresh
   preserves pending choices for unchanged outputs, keeps cards visible during
   detection, and disables stale controls if detection fails.

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

`make install` and the curl installer create a `nvibrant-gtk` launcher and a
desktop entry automatically. The default prefix is `~/.local`; add
`~/.local/bin` to your PATH if you want to launch `nvibrant-gtk` by name.
Custom prefixes may need their `share` directory added to `XDG_DATA_DIRS` for
the desktop menu to find the entry.

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
