"""Restore saved colours at desktop login, without opening a GTK window."""
import subprocess
import sys
import time

from nvibrant.backend import Backend
from nvibrant.settings import SettingsStore


def restore(settings=None, backend_factory=Backend, attempts=12, delay=5, sleep=time.sleep):
    settings = settings if settings is not None else SettingsStore()
    settings.load()  # Invalid preferences must never result in a driver write.
    pending = {int(gpu) for gpu, values in settings.data['gpus'].items() if values}
    errors = {}
    for attempt in range(attempts):
        for gpu in sorted(pending):
            try:
                backend = backend_factory(gpu=gpu)
                ports = backend.discover()
                values = {p['index']: settings.value(gpu, p) for p in ports
                          if p['connected'] and settings.value(gpu, p) is not None}
                if not values:
                    raise RuntimeError('No connected outputs with saved colours')
                backend.apply(ports, values, preserve_unspecified=True)
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
                errors[gpu] = str(exc)
            else:
                pending.remove(gpu)
        if not pending:
            return
        if attempt + 1 < attempts:
            sleep(delay)
    raise RuntimeError('; '.join(f'GPU {gpu}: {errors[gpu]}' for gpu in sorted(pending)))


def main():
    try:
        restore()
    except (OSError, ValueError, RuntimeError) as exc:
        print(f'nVibrant restore: {exc}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
