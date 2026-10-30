"""Validated per-GPU/output preferences, saved only after successful Apply."""
import fcntl
import json
import os
from pathlib import Path
import re
import tempfile


def config_path():
    root = Path(os.environ.get('XDG_CONFIG_HOME') or Path.home() / '.config')
    if not root.is_absolute():
        root = Path.home() / '.config'
    return root / 'nvibrant/settings.json'


def empty():
    return {'version': 1, 'last_gpu': 0, 'gpus': {}}


def port_key(port):
    return f"{port['display']}:{port['connector']}:{port['type']}"


class SettingsStore:
    def __init__(self, path=None):
        self.path = Path(path) if path is not None else config_path()
        self.data = empty()

    def _read(self):
        try:
            data = json.loads(self.path.read_text(encoding='utf-8'))
        except FileNotFoundError:
            return empty()
        except (ValueError, UnicodeError) as exc:
            raise ValueError(f'Cannot read saved settings at {self.path}: {exc}') from exc
        if (not isinstance(data, dict) or type(data.get('version')) is not int or data['version'] != 1
                or type(data.get('last_gpu')) is not int or not 0 <= data['last_gpu'] <= 15
                or not isinstance(data.get('gpus'), dict)):
            raise ValueError(f'Invalid settings format at {self.path}')
        for gpu, outputs in data['gpus'].items():
            if not re.fullmatch(r'(?:[0-9]|1[0-5])', gpu) or not isinstance(outputs, dict):
                raise ValueError(f'Invalid GPU settings at {self.path}')
            for key, value in outputs.items():
                if (not re.fullmatch(r'[0-9]+:[0-9]+:[A-Za-z0-9_-]+', key)
                        or type(value) is not int or not -1024 <= value <= 1023):
                    raise ValueError(f'Invalid display setting at {self.path}')
        return data

    def load(self):
        self.data = self._read()

    @property
    def last_gpu(self):
        return self.data['last_gpu']

    def value(self, gpu, port):
        return self.data['gpus'].get(str(gpu), {}).get(port_key(port))

    def save(self, gpu, ports, values):
        if type(gpu) is not int or not 0 <= gpu <= 15:
            raise ValueError('Invalid NVIDIA GPU index')
        updates = {}
        for port in ports:
            if port['connected'] and port['index'] in values:
                value = values[port['index']]
                if type(value) is not int or not -1024 <= value <= 1023:
                    raise ValueError('Invalid saved vibrance value')
                updates[port_key(port)] = value
        self.path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        # Serialize multiple app windows and merge with the latest saved GPUs.
        with (self.path.parent / '.settings.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            data = self._read()
            data['gpus'].setdefault(str(gpu), {}).update(updates)
            data['last_gpu'] = gpu
            temporary = None
            try:
                # Same directory means replace cannot cross filesystem boundaries.
                with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8',
                                                 prefix='.settings-', suffix='.tmp',
                                                 dir=self.path.parent, delete=False) as stream:
                    temporary = Path(stream.name)
                    json.dump(data, stream, indent=2, sort_keys=True)
                    stream.write('\n')
                    stream.flush()
                    os.fsync(stream.fileno())
                os.replace(temporary, self.path)
                self.data = data
            finally:
                if temporary is not None:
                    temporary.unlink(missing_ok=True)
