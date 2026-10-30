"""Run under xvfb-run; exercise real GTK widgets with a fake backend."""
import sys
import time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
gi.require_version('Graphene', '1.0')
from gi.repository import Gtk, GLib, Gdk, Graphene
from nvibrant.gui import AeroWindow

app = Gtk.Application(application_id='io.github.nvibrant.Smoke')
app.register(None)
css = Gtk.CssProvider()
css.load_from_path(str(Path(__file__).resolve().parents[1] / 'nvibrant/assets/aero.css'))
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
