"""nVibrant Glass - native GTK 4 display controls."""
import argparse
import math
import sys
import threading
from pathlib import Path

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
from gi.repository import Gdk, Gio, GLib, Gtk
from nvibrant.backend import Backend


def label(text, css=None):
    widget = Gtk.Label(label=text, xalign=0)
    if css:
        widget.add_css_class(css)
    return widget


def box(vertical=False, spacing=12):
    return Gtk.Box(orientation=Gtk.Orientation.VERTICAL if vertical else Gtk.Orientation.HORIZONTAL,
                   spacing=spacing)


class AeroWindow(Gtk.ApplicationWindow):
    def __init__(self, app, demo=False):
        super().__init__(application=app, title='nVibrant Glass')
        self.set_default_size(820, 780)
        Gtk.IconTheme.get_for_display(self.get_display()).add_search_path(
            str(Path(__file__).with_name('assets')))
        self.set_icon_name('io.github.nvibrant.Aero')
        self.backend = Backend(demo)
        self.ports = []
        self.scales = {}
        self.busy = False
        self.dirty = False
        self.connect('close-request', self.on_close)
        header = Gtk.HeaderBar()
        header.set_title_widget(label('nVibrant  /  Glass', 'wordmark'))
        self.set_titlebar(header)
        self.add_css_class('aero')
        scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER)
        self.set_child(scroll)
        root = box(True, 22)
        for side in ('top', 'bottom', 'start', 'end'):
            getattr(root, f'set_margin_{side}')(30)
        scroll.set_child(root)
        hero = box(False, 22)
        orb = Gtk.DrawingArea(content_width=104, content_height=104)
        orb.set_draw_func(self.draw_orb)
        hero.append(orb)
        titles = box(True, 4)
        titles.set_valign(Gtk.Align.CENTER)
        titles.append(label('COLOUR, REFINED', 'eyebrow'))
        titles.append(label('Your display. More vivid.', 'headline'))
        titles.append(label('Fine-tune your colours. Find your balance.', 'subtitle'))
        hero.append(titles)
        root.append(hero)
        toolbar = box()
        toolbar.append(label('YOUR DISPLAYS', 'eyebrow'))
        spacer = box(); spacer.set_hexpand(True); toolbar.append(spacer)
        toolbar.append(label('NVIDIA GPU', 'muted'))
        self.gpu = Gtk.SpinButton.new_with_range(0, 15, 1)
        self.gpu.set_tooltip_text('NVIDIA GPU index; change it, then click Refresh')
        self.gpu.connect('value-changed', self.invalidate_gpu)
        toolbar.append(self.gpu)
        self.refresh = Gtk.Button(label='↻  Refresh')
        self.refresh.connect('clicked', lambda *_: self.discover())
        toolbar.append(self.refresh)
        root.append(toolbar)
        self.cards = box(True, 14)
        root.append(self.cards)
        hint = label('Choose a look, then Apply. Opening or refreshing never changes your display.', 'muted')
        hint.set_wrap(True)
        root.append(hint)
        actions = box()
        self.reset = Gtk.Button(label='Reset to natural')
        self.reset.connect('clicked', self.reset_values)
        actions.append(self.reset)
        stretch = box(); stretch.set_hexpand(True); actions.append(stretch)
        self.apply_button = Gtk.Button(label='Apply colours  →')
        self.apply_button.add_css_class('suggested-action')
        self.apply_button.connect('clicked', self.apply)
        actions.append(self.apply_button)
        root.append(actions)
        self.status = label('Finding your displays…', 'status')
        self.status.set_wrap(True)
        self.status.set_selectable(True)
        root.append(self.status)
        details = Gtk.Expander(label='Details & compatibility')
        self.log = label('NVIDIA must drive the display directly. Hybrid laptop screens may use the iGPU.\n'
                         'Values start at natural (0); they are pending choices, not a reading of current settings.\n'
                         'Settings last until the driver resets; this app does not install an autostart service.', 'muted')
        self.log.set_wrap(True)
        self.log.set_selectable(True)
        details.set_child(self.log)
        root.append(details)
        self.demo = demo
        self.discover()

    @staticmethod
    def draw_orb(area, cr, w, h):
        import cairo
        cr.save(); cr.scale(w / 104, h / 104)
        gradient = cairo.RadialGradient(34, 23, 2, 54, 57, 51)
        gradient.add_color_stop_rgb(0, .56, .60, .69)
        gradient.add_color_stop_rgb(.34, .22, .25, .32)
        gradient.add_color_stop_rgb(.72, .06, .07, .10)
        gradient.add_color_stop_rgb(1, .15, .18, .25)
        cr.arc(52, 52, 47, 0, 2 * math.pi); cr.set_source(gradient); cr.fill()
        cr.set_line_width(1)
        cr.set_source_rgba(.83, .88, 1, .45)
        cr.arc(52, 52, 47, 0, 2 * math.pi); cr.stroke()
        sheen = cairo.LinearGradient(0, 12, 0, 58)
        sheen.add_color_stop_rgba(0, 1, 1, 1, .7)
        sheen.add_color_stop_rgba(1, 1, 1, 1, 0)
        cr.save(); cr.translate(47, 30); cr.rotate(-.3); cr.scale(1, .42)
        cr.arc(0, 0, 33, 0, 2 * math.pi); cr.set_source(sheen); cr.fill(); cr.restore()
        # A quiet spectral wave suspended inside the dark glass sphere.
        wave = cairo.LinearGradient(24, 0, 81, 0)
        wave.add_color_stop_rgb(0, .55, .74, 1)
        wave.add_color_stop_rgb(.5, .91, .9, 1)
        wave.add_color_stop_rgb(1, .66, .63, .92)
        cr.set_source(wave); cr.set_line_width(2.5)
        cr.set_line_cap(cairo.LINE_CAP_ROUND)
        cr.move_to(25, 61); cr.curve_to(39, 76, 61, 35, 80, 47); cr.stroke()
        cr.set_source_rgba(.83, .89, 1, .35); cr.set_line_width(1.2)
        cr.move_to(26, 68); cr.curve_to(46, 81, 64, 46, 80, 54); cr.stroke()
        cr.restore()

    def on_close(self, *_):
        # Let an in-flight driver request report its result before closing.
        return self.busy

    def set_busy(self, busy, lock_cards=True):
        self.busy = busy
        for widget in (self.refresh, self.gpu, self.apply_button, self.reset):
            widget.set_sensitive(not busy)
        self.cards.set_sensitive(bool(self.scales) and (not busy or not lock_cards))
        self.apply_button.set_sensitive(not busy and bool(self.scales))
        self.reset.set_sensitive(not busy and bool(self.scales))

    def task(self, work, done, on_error=None, lock_cards=True):
        self.set_busy(True, lock_cards=lock_cards)
        def worker():
            try:
                result, error = work(), None
            except Exception as exc:
                result, error = None, str(exc)
            GLib.idle_add(finish, result, error)
        def finish(result, error):
            if error:
                if on_error:
                    on_error()
                self.status.set_text('Could not complete the request. Expand Details below.')
                self.status.add_css_class('error')
                self.log.set_text(error)
            else:
                self.status.remove_css_class('error')
                done(result)
            self.set_busy(False)
            return GLib.SOURCE_REMOVE
        threading.Thread(target=worker, daemon=True).start()

    def invalidate_gpu(self, *_):
        if self.busy:
            return
        self.scales = {}
        self.ports = []
        while child := self.cards.get_first_child():
            self.cards.remove(child)
        self.set_busy(False)
        self.status.set_text('GPU changed • Click Refresh to detect its displays.')

    def discover(self):
        if self.busy:
            return
        # Keep the visible cards (and their pending values) during read-only scans.
        self.backend.gpu = self.gpu.get_value_as_int()
        self.status.set_text('Finding your displays…')
        def failed():
            # Preserve layout, but make stale controls unusable until a good scan.
            self.scales = {}
            self.ports = []
        self.task(self.backend.discover, self.show_ports, on_error=failed, lock_cards=False)

    def show_ports(self, ports):
        if ports != self.ports or not self.scales:
            previous = {p['index']: p for p in self.ports}
            values = {index: round(scale.get_value()) for index, scale in self.scales.items()}
            was_dirty = self.dirty
            self.scales = {}
            while child := self.cards.get_first_child():
                self.cards.remove(child)
            retained = False
            for port in ports:
                if port['connected']:
                    keep = previous.get(port['index']) == port and port['index'] in values
                    self.add_card(port, values[port['index']] if keep else 0)
                    retained |= keep
            self.dirty = was_dirty and retained
        self.ports = ports
        self.status.set_text('Pending changes preserved • Click Apply colours when ready.' if self.dirty else
                             ('DEMO MODE • No hardware changes will be made.' if self.demo else
                              ('Ready. Choose your colours below.' if self.scales else
                               'No connected NVIDIA displays. Try another GPU index or check hybrid graphics.')))

    def add_card(self, port, initial_value=0):
        card = box(True, 14); card.add_css_class('glass-card')
        top = box()
        title = box(True, 3)
        title.append(label(f"▣  {port['type']} · Port {port['connector'] + 1}", 'card-title'))
        title.append(label(f"GPU {self.backend.gpu}  /  Display {port['display']}  /  Output {port['index'] + 1}", 'muted'))
        title.set_hexpand(True); top.append(title)
        value = label(f'{initial_value:+d}' if initial_value else '0', 'value'); top.append(value); card.append(top)
        scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, -1024, 1023, 1)
        scale.set_value(initial_value); scale.set_draw_value(False); scale.set_hexpand(True)
        scale.set_tooltip_text('Digital vibrance: −1024 grayscale, 0 natural, 1023 maximum')
        scale.update_property([Gtk.AccessibleProperty.LABEL],
                              [f"Vibrance for {port['type']} output {port['index'] + 1}"])
        scale.connect('value-changed', self.changed, value)
        self.scales[port['index']] = scale
        card.append(scale)
        ends = box()
        ends.append(label('Grayscale', 'muted'))
        spacer = box(); spacer.set_hexpand(True); ends.append(spacer)
        ends.append(label('Full bloom', 'muted')); card.append(ends)
        presets = box(False, 8)
        for name, amount in [('Natural', 0), ('Fresh', 256), ('Vivid', 512), ('Luminous', 768)]:
            button = Gtk.Button(label=name)
            button.connect('clicked', lambda _, s=scale, v=amount: s.set_value(v))
            presets.append(button)
        card.append(presets); self.cards.append(card)

    def changed(self, scale, value_label):
        amount = round(scale.get_value())
        value_label.set_text(f'{amount:+d}' if amount else '0')
        self.dirty = True
        self.status.set_text('Pending changes • Click Apply colours to update your display.')

    def reset_values(self, *_):
        for scale in self.scales.values():
            scale.set_value(0)
        self.status.set_text('Natural colours selected • Click Apply colours to reset.')

    def apply(self, *_):
        if self.busy or not self.scales:
            return
        values = {index: round(scale.get_value()) for index, scale in self.scales.items()}
        self.status.set_text('Applying colours…')
        def done(output):
            self.dirty = False
            self.log.set_text(output)
            self.status.set_text('Demo preview updated • Your displays were not changed.' if self.demo else
                                 'Colours applied. Enjoy the view.')
        self.task(lambda: self.backend.apply(self.ports, values), done)


def main():
    parser = argparse.ArgumentParser(description='nVibrant Glass GTK 4 interface')
    parser.add_argument('--demo', action='store_true', help='Preview without accessing NVIDIA hardware')
    args = parser.parse_args()
    app = Gtk.Application(application_id='io.github.nvibrant.Aero',
                          flags=Gio.ApplicationFlags.NON_UNIQUE)
    def activate(application):
        css = Gtk.CssProvider()
        css.load_from_path(str(Path(__file__).with_name('assets') / 'aero.css'))
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css,
                                                  Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        AeroWindow(application, args.demo).present()
    app.connect('activate', activate)
    return app.run([sys.argv[0]])


if __name__ == '__main__':
    raise SystemExit(main())
