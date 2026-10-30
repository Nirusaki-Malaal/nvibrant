"""Check real GTK save/reopen behavior with a fake driver and temporary config."""
import os
from pathlib import Path
import sys
import tempfile
import time
from unittest.mock import patch

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(os.environ.get('NVIBRANT_TEST_APP', root))))
import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, GLib
from nvibrant.gui import AeroWindow
from nvibrant.settings import SettingsStore


class FakeBackend:
    def __init__(self):
        self.gpu = 0
        self.calls = []
        self.fail = False

    def discover(self):
        return [dict(index=3, display=0, connector=3, type='DisplayPort', connected=True)]

    def apply(self, ports, values):
        self.calls.append(dict(values))
        if self.fail:
            raise RuntimeError('Test driver failure')
        return 'Success'


def settle(window):
    end = time.monotonic() + 4
    while window.busy and time.monotonic() < end:
        GLib.MainContext.default().iteration(True)
    assert not window.busy


app = Gtk.Application(application_id='io.github.nvibrant.SettingsSmoke')
app.register(None)
with tempfile.TemporaryDirectory(prefix='nvibrant-settings-gtk-') as temporary:
    path = Path(temporary) / 'settings.json'
    def open_window(demo=False):
        window = AeroWindow(app, demo=demo, settings=SettingsStore(path), backend=FakeBackend())
        settle(window)
        return window
    window = open_window()
    assert not path.exists()
    assert window.scales[3].get_value() == 0
    window.gpu.set_value(1)
    window.discover()
    settle(window)
    window.scales[3].set_value(512)
    assert not path.exists(), 'Moving a slider must not save'
    window.apply()
    settle(window)
    assert 'applied and saved' in window.status.get_text()
    original = path.read_bytes()
    window.scales[3].set_value(768)  # Unsaved edit must not survive close.
    window.destroy()

    reopened = open_window()
    assert reopened.gpu.get_value_as_int() == 1
    assert reopened.scales[3].get_value() == 512
    assert reopened.backend.calls == [], 'Opening must never apply colours'
    assert 'Saved colours loaded' in reopened.status.get_text()
    reopened.backend.fail = True
    reopened.scales[3].set_value(768)
    reopened.apply()
    settle(reopened)
    assert path.read_bytes() == original, 'Failed hardware apply must not save'
    reopened.backend.fail = False
    with patch.object(reopened.settings, 'save', side_effect=OSError('Test disk failure')):
        reopened.apply()
        settle(reopened)
        assert 'applied, but settings could not be saved' in reopened.status.get_text()
        assert path.read_bytes() == original
    reopened.reset_values()
    reopened.apply()
    settle(reopened)
    reopened.destroy()
    natural = open_window()
    assert natural.scales[3].get_value() == 0, 'Successful natural reset must be saved'
    natural.destroy()

    original = path.read_bytes()
    demo = open_window(demo=True)
    assert demo.gpu.get_value_as_int() == 0, 'Demo must not load real preferences'
    demo.scales[3].set_value(768)
    demo.apply()
    settle(demo)
    demo.destroy()
    assert path.read_bytes() == original, 'Demo must not overwrite real preferences'

    path.write_text('{corrupt')
    corrupt = open_window()
    assert corrupt.scales[3].get_value() == 0
    assert 'Saved settings could not be loaded' in corrupt.status.get_text()
    assert path.read_text() == '{corrupt'
    corrupt.destroy()
print('GTK persistence checks passed: save, reopen, GPU restore, errors, reset and demo isolation')
