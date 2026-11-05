# Using NVibrant

## Controls

- **Refresh** finds displays connected to NVIDIA GPUs automatically.
- **Natural**, **Fresh**, **Vivid**, **Luminous** select 0, 256, 512 or 768.
- **Apply** changes your displays and saves the values after success.
- **Reset** selects zero on all displayed screens; Apply commits it.

Values run from −1024 (grayscale), through 0 (natural), to 1023 (maximum).
Opening the app and moving sliders do not change your display. Refresh preserves
pending choices for unchanged outputs. Errors appear in a dialog.

## Settings and startup

Settings live in `~/.config/nvibrant/settings.json`, or under an absolute
`XDG_CONFIG_HOME`. Upgrades read existing settings. Unapplied edits, demo previews
and failed driver calls are never saved.

The installer adds **NVibrant saved colours** to desktop startup. It restores
saved values after login without opening a window, retrying while displays become
ready. Disable that entry in Startup Applications to opt out.

For compositors without XDG autostart support, add `nvibrant-gtk --restore` to the
compositor's own startup configuration. After sleep or an in-session driver reset,
Apply again or run the restore command. Remove any competing vibrancy startup
commands you previously configured.

## CLI

```sh
nvibrant                       # Open the app
nvibrant --list-all-json        # Find connected NVIDIA screens
nvibrant --list-json --gpu 0    # Inspect all outputs on one GPU
nvibrant 512 512                # Set output values directly
nvibrant --skip 512             # Leave the first output unchanged
nvibrant-gtk --restore          # Restore saved values without a window
nvibrant-gtk --demo             # Preview without hardware or real settings
nvibrant --version
```

Positional values include disconnected ports. Inspect `--list-json` for their
order. Missing positional values default to natural vibrancy; use `--skip` to
leave an output untouched. Direct CLI changes do not save GUI preferences.
`NVIDIA_GPU` selects the default GPU, and `ATTRIBUTE` supports `dithering`,
`dithering-mode` and `dithering-depth`.

## Troubleshooting

**No screens found:** NVIDIA must drive the output directly. A laptop panel wired
to an integrated GPU is outside NVIDIA's controls. Check the loaded NVIDIA driver
and modesetting configuration, then Refresh.

**Driver version mismatch:** run the installer again after a driver upgrade so
it rebuilds against matching headers.

**Settings do not return after reboot:** check that the NVibrant startup entry is
enabled. Try `~/.local/bin/nvibrant-gtk --restore` in a terminal to see any errors.

[Install](../README.md) · [Build and development](BUILDING.md)
