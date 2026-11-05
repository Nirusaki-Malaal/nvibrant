---
title: Autostart
icon: octicons/rocket-16
description: Configure the automatic startup of nvibrant using a Systemd user service,
  including instructions for settings that require environment variables.
tags:
- Documentation
- Quickstart
- Autostart
---

## NVibrant Rust

This fork's installer adds a desktop login entry that restores the GUI's last
successfully applied colours in the background, including after a reboot. No
additional service is needed. It retries while the NVIDIA displays become ready
and leaves outputs without saved preferences untouched.

To restore manually, run `nvibrant-gtk --restore`. Disable the entry in your
desktop's Startup Applications settings to stop automatic restoration. Remove
older vibrance startup commands if you previously configured them, so they do
not overwrite the saved colours. See [the GUI guide](https://github.com/Nirusaki-Malaal/nvibrant/blob/main/GUI.md).

## Legacy upstream CLI systemd service

For simplicity, a Systemd user service running either [`uvx`](https://docs.astral.sh/uv/concepts/tools/) for the latest releases (in case of driver updates), or a [prebuilt binary](../get/actions.md) directly should cover most users, and integrates well with dotfiles.

Create a file at `~/.config/systemd/user/nvibrant.service` with the content:

```ini
[Unit]
Description=Apply nvibrant
After=graphical.target

[Service]
Type=oneshot
ExecStartPre=/bin/sleep 5
ExecStart=uvx nvibrant 1023 1023

[Install]
WantedBy=default.target
```

Change values, enable the service with `systemctl --user enable --now nvibrant.service`

!!! tip "Tips"
    - You can pin it to a specific version with `uvx nvibrant==1.2.1 (args)` to have more control/security
    - Local binary at `~/.local/bin/nvibrant` and use `ExecStart=%h/.local/bin/nvibrant (args)`
    - Sleeping for a few seconds can prevent racing conditions with the display server starting up

### Environment variables

For autostarting [dithering](./dithering.md) or [multigpus](./multigpu.md), which are done via environment variables, systemd syntax does not run commands in a shell - a few options are:

=== ":simple-gnu: Using GNU coreutils"
    ```ini linenums="1"
    [Service]
    ExecStart=/usr/bin/env ATTRIBUTE=dithering uvx nvibrant
    ```

=== ":simple-gnubash: Using bash"
    ```ini linenums="1"
    [Service]
    ExecStart=/bin/bash -c "ATTRIBUTE=dithering uvx nvibrant"
    ```

You may add two `ExecStart` to the service, a combined setup may look like:

=== ":simple-gnu: Using GNU coreutils"
    ```ini linenums="1"
    [Service]
    Type=oneshot
    ExecStartPre=/bin/sleep 5
    ExecStart=/usr/bin/env ATTRIBUTE=dithering uvx nvibrant
    ExecStart=/usr/bin/env ATTRIBUTE=vibrance  uvx nvibrant 1023 1023
    ```

=== ":simple-gnubash: Using bash"
    ```ini linenums="1"
    [Service]
    Type=oneshot
    ExecStartPre=/bin/sleep 5
    ExecStart=/bin/bash -c "ATTRIBUTE=dithering uvx nvibrant"
    ExecStart=/bin/bash -c "ATTRIBUTE=vibrance  uvx nvibrant 1023 1023"
    ```
