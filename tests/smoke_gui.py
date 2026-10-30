"""Run under xvfb-run; exercise real GTK widgets with a fake backend."""
import sys
import os
import time
import threading
from pathlib import Path
source_root = Path(__file__).resolve().parents[1]
package_root = Path(os.environ.get('NVIBRANT_TEST_APP', source_root)).resolve()
sys.path.insert(0, str(package_root))
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
gi.require_version('Graphene', '1.0')
from gi.repository import Gtk, GLib, Gdk, Graphene
from nvibrant.gui import AeroWindow

app = Gtk.Application(application_id='io.github.nvibrant.Smoke')
app.register(None)
css = Gtk.CssProvider()
css.load_from_path(str(package_root / 'nvibrant/assets/aero.css'))
Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
app.connect('activate', lambda *_: None)
window = AeroWindow(app, demo=True)
window.set_size_request(820, 1000)
window.present()

def settle():
    deadline = time.monotonic() + 4
    while window.busy and time.monotonic() < deadline:
        GLib.MainContext.default().iteration(True)
    assert not window.busy, 'Operation timed out'

def exercise():
    settle()
    assert len(window.scales) == 2
    window.scales[0].set_value(512)
    assert 'Pending' in window.status.get_text()
    # A slow refresh must retain the same widgets, layout and pending values.
    card = window.cards.get_first_child()
    scale = window.scales[0]
    release = threading.Event()
    discover = window.backend.discover
    def slow_discover():
        assert release.wait(3), 'Test scan was never released'
        return discover()
    window.backend.discover = slow_discover
    window.refresh.emit('clicked')
    assert window.busy
    assert window.cards.get_first_child() is card
    assert window.cards.get_sensitive()
    assert not window.apply_button.get_sensitive()
    scale.set_value(768)
    release.set()
    settle()
    assert window.cards.get_first_child() is card
    assert window.scales[0] is scale
    assert scale.get_value() == 768
    assert window.dirty
    # A real topology change rebuilds once and retains unchanged output values.
    window.backend.discover = lambda: discover() + [
        dict(index=3, display=0, connector=3, type='DisplayPort', connected=True)]
    window.discover()
    settle()
    assert set(window.scales) == {0, 1, 3}
    assert window.scales[0].get_value() == 768
    assert window.scales[3].get_value() == 0
    window.backend.discover = discover
    window.discover()
    settle()
    assert set(window.scales) == {0, 1}
    assert window.scales[0].get_value() == 768
    card = window.cards.get_first_child()
    # Failed scans retain the layout but disable all stale display controls.
    window.backend.discover = lambda: (_ for _ in ()).throw(RuntimeError('Refresh failed'))
    window.discover()
    settle()
    assert window.cards.get_first_child() is card
    assert not window.cards.get_sensitive()
    assert not window.apply_button.get_sensitive()
    window.backend.discover = discover
    window.discover()
    settle()
    assert window.cards.get_sensitive()
    window.apply()
    settle()
    assert 'not changed' in window.status.get_text()
    window.reset_values()
    assert all(s.get_value() == 0 for s in window.scales.values())
    window.gpu.set_value(1)
    assert not window.apply_button.get_sensitive()
    window.backend.discover = lambda: (_ for _ in ()).throw(RuntimeError('Test driver failure'))
    window.discover()
    settle()
    assert not window.scales
    assert not window.apply_button.get_sensitive()
    assert 'Test driver failure' in window.log.get_text()
    window.backend = __import__('nvibrant.backend', fromlist=['Backend']).Backend(demo=True)
    window.gpu.set_value(0)
    window.discover()
    settle()
    assert len(window.scales) == 2
    window.scales[0].set_value(256)
    window.scales[1].set_value(512)
    window.status.set_text('DEMO MODE • No hardware changes will be made.')
    GLib.timeout_add(500, capture)
    return False

# GTK/GSK renders the actual window to an image, without desktop capture.
def capture():
    paintable = Gtk.WidgetPaintable.new(window)
    snapshot = Gtk.Snapshot.new()
    paintable.snapshot(snapshot, window.get_width(), window.get_height())
    node = snapshot.to_node()
    viewport = Graphene.Rect()
    viewport.init(0, 0, window.get_width(), window.get_height())
    texture = window.get_renderer().render_texture(node, viewport)
    texture.save_to_png(str(Path(__file__).resolve().parents[1] / 'screenshots/aero.png'))
    app.quit()
    return False
GLib.timeout_add(500, exercise)
app.run([])
print('GTK smoke checks passed')
