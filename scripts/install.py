#!/usr/bin/env python3
"""Install a self-contained native build; never touch system Python packages."""
import argparse
import json
import os
from pathlib import Path
import shlex
import shutil
import sys
import subprocess
import tempfile

MARKER = '.nvibrant-install.json'
APP_ID = 'io.github.nvibrant.Aero'
DESKTOP = f'{APP_ID}.desktop'


def locations(prefix):
    return (prefix / 'lib/nvibrant', prefix / 'bin/nvibrant-gtk',
            prefix / 'bin/nvibrant', prefix / 'share/applications' / DESKTOP,
            prefix / 'share/icons/hicolor/scalable/apps' / f'{APP_ID}.svg')


def exists(path):
    return path.exists() or path.is_symlink()


def check_owned(prefix):
    app, gui, cli, desktop, icon = locations(prefix)
    if app.is_symlink():
        raise RuntimeError(f'Refusing to replace a symlink installation directory: {app}')
    if exists(app):
        marker = app / MARKER
        if not marker.is_file() or json.loads(marker.read_text()).get('application') != 'nvibrant-glass':
            raise RuntimeError(f'Refusing to replace an unrecognized directory: {app}')
    elif any(exists(p) for p in (gui, cli, desktop, icon)):
        raise RuntimeError('An existing launcher or desktop file would be overwritten. Use another prefix.')
    return app, gui, cli, desktop, icon


def install(source, build_dir, prefix, driver):
    app, gui, cli, desktop, icon = check_owned(prefix)
    backend = build_dir / 'nvibrant'
    if not backend.is_file() or not os.access(backend, os.X_OK):
        raise RuntimeError(f'Backend is missing or not executable: {backend}')
    for directory in (app.parent, gui.parent, desktop.parent, icon.parent):
        directory.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix='.nvibrant-install-', dir=app.parent))
    backup = app.with_name('.nvibrant-previous')
    if exists(backup):
        shutil.rmtree(staging)
        raise RuntimeError(f'Previous interrupted upgrade exists: {backup}. Restore or move it before retrying.')
    try:
        (staging / 'build').mkdir()
        shutil.copy2(backend, staging / 'build/nvibrant')
        shutil.copytree(source / 'nvibrant', staging / 'nvibrant',
                        ignore=shutil.ignore_patterns('__pycache__', '*.pyc', '*.cpp', 'resources'))
        shutil.copy2(source / 'license.txt', staging / 'license.txt')
        (staging / MARKER).write_text(json.dumps({
            'application': 'nvibrant-glass', 'driver': driver, 'python': sys.executable,
        }) + '\n')
        # Use a private backend beside the Python package, independent of source
        # checkout, working directory, Python site-packages and the system CLI.
        launcher = '''#!/bin/sh
# nVibrant Glass launcher
app_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../lib/nvibrant" && pwd) || exit 1
cd -- "$app_dir" || exit 1
exec INTERPRETER -m nvibrant.gui "$@"
'''.replace('INTERPRETER', shlex.quote(sys.executable))
        entry = (source / 'data' / DESKTOP).read_text()
        # Desktop Exec has its own quoting rules, separate from shell quoting.
        executable = str(gui).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
        entry = entry.replace('Exec=nvibrant-gtk', f'Exec="{executable}"')
        if app.exists():
            app.rename(backup)
        try:
            staging.rename(app)
            gui.write_text(launcher)
            gui.chmod(0o755)
            if exists(cli):
                cli.unlink()
            cli.symlink_to('../lib/nvibrant/build/nvibrant')
            desktop.write_text(entry)
            shutil.copy2(app / 'nvibrant/assets' / f'{APP_ID}.svg', icon)
        except Exception:
            # Restore the previous complete payload on an interrupted upgrade.
            if backup.exists():
                shutil.rmtree(app, ignore_errors=True)
                backup.rename(app)
            raise
        if backup.exists():
            shutil.rmtree(backup)
    finally:
        if staging.exists():
            shutil.rmtree(staging)
    refresh_desktop(prefix)
    print(f'Installed nVibrant Glass into {prefix}')
    print(f'Launch: {gui}')
    print(f'Uninstall: bash build.sh uninstall --prefix {shlex.quote(str(prefix))}')
    if str(gui.parent) not in os.environ.get('PATH', '').split(os.pathsep):
        print(f'Add {gui.parent} to PATH to use nvibrant-gtk by name.')
    print('A desktop launcher is installed. No display settings were changed.')


def uninstall(prefix):
    app, gui, cli, desktop, icon = check_owned(prefix)
    if not app.exists():
        print(f'No nVibrant Glass installation at {prefix}')
        return
    for path in (gui, cli, desktop, icon):
        if exists(path):
            path.unlink()
    shutil.rmtree(app)
    refresh_desktop(prefix)
    print(f'Uninstalled nVibrant Glass from {prefix}')


def refresh_desktop(prefix):
    # These cache tools are optional; desktops can also discover files directly.
    for name, arguments in (
        ('gtk-update-icon-cache', ['--force', '--ignore-theme-index', str(prefix / 'share/icons/hicolor')]),
        ('update-desktop-database', [str(prefix / 'share/applications')]),
    ):
        command = shutil.which(name)
        if command:
            try:
                subprocess.run([command, *arguments], check=False, timeout=10,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            except (OSError, subprocess.TimeoutExpired):
                pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['install', 'uninstall'])
    parser.add_argument('--prefix', type=Path, required=True)
    parser.add_argument('--source', type=Path)
    parser.add_argument('--build-dir', type=Path)
    parser.add_argument('--driver-version', default='unknown')
    args = parser.parse_args()
    if not args.prefix.is_absolute() or args.prefix == Path('/'):
        parser.error('prefix must be absolute and cannot be /')
    try:
        if args.action == 'install':
            if args.source is None or args.build_dir is None:
                parser.error('install needs --source and --build-dir')
            install(args.source.resolve(), args.build_dir.resolve(), args.prefix, args.driver_version)
        else:
            uninstall(args.prefix)
    except (OSError, ValueError, RuntimeError) as exc:
        print(f'nVibrant install: {exc}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
