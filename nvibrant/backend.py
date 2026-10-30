"""Small, GTK-independent boundary around the fork's native backend."""
import json
import os
import subprocess
from pathlib import Path


class Backend:
    def __init__(self, demo=False, gpu=0):
        self.demo = demo
        self.gpu = gpu

    def run(self, *args):
        binary = Path(__file__).resolve().parent.parent / 'build' / 'nvibrant'
        if not binary.is_file():
            # Only this fork's bundled binaries are eligible; an upstream system
            # binary interprets --list-json as zero and would reset the displays.
            from nvibrant import get_versions, get_best
            if not get_versions():
                raise RuntimeError('Backend missing. Follow GUI.md to build this fork first.')
            try:
                _, binary = get_best()
            except (SystemExit, ValueError) as exc:
                raise RuntimeError('No compatible NVIDIA driver/backend found. See GUI.md.') from exc
            binary.chmod(binary.stat().st_mode | 0o100)
        env = dict(os.environ, NVIDIA_GPU=str(self.gpu), ATTRIBUTE='vibrance')
        result = subprocess.run([str(binary), *map(str, args)], env=env,
                                text=True, capture_output=True, timeout=15)
        output = (result.stdout + result.stderr).strip()
        if result.returncode or 'Failed' in output or 'failed' in output:
            raise RuntimeError(output or f'Backend exited with {result.returncode}')
        return output

    def discover(self):
        if self.demo:
            return [dict(index=0, display=0, connector=0, type='HDMI', connected=True),
                    dict(index=1, display=0, connector=1, type='DisplayPort', connected=True)]
        ports = json.loads(self.run('--list-json'))['ports']
        if not isinstance(ports, list):
            raise RuntimeError('Invalid display information from backend')
        return ports

    def apply(self, ports, values):
        if not ports:
            raise RuntimeError('No NVIDIA displays detected')
        args = [0] * (max(p['index'] for p in ports) + 1)
        for index, value in values.items():
            if index < 0 or index >= len(args) or not -1024 <= value <= 1023:
                raise ValueError('Invalid output or vibrance value')
            args[index] = value
        if self.demo:
            return 'Demo only - no display settings changed.'
        output = self.run(*args)
        if 'Success' not in output:
            raise RuntimeError(output or 'No display accepted the setting')
        return output
