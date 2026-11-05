use crate::{
    backend::{self, Port},
    runtime::{self, Display, Native},
    settings::{self, Settings},
};
use anyhow::{Result, anyhow, ensure};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

struct Card {
    gpu: u8,
    port: Port,
    scale: gtk::Scale,
}
struct State {
    displays: Vec<Display>,
    cards: Vec<Card>,
    settings: Settings,
}
struct Ui {
    window: gtk::ApplicationWindow,
    refresh: gtk::Button,
    apply: gtk::Button,
    reset: gtk::Button,
    cards: gtk::Box,
    empty: gtk::Label,
    state: RefCell<State>,
    busy: Cell<bool>,
    demo: bool,
}
fn label(text: &str, class: &str) -> gtk::Label {
    let widget = gtk::Label::new(Some(text));
    widget.set_xalign(0.0);
    widget.add_css_class(class);
    widget
}
fn wordmark(class: &str) -> gtk::Label {
    let title = label("", class);
    title.set_markup("<span foreground=\"#76b900\">N</span>Vibrant");
    title
}
fn row(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}
fn column(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}
fn spacer() -> gtk::Box {
    let widget = row(0);
    widget.set_hexpand(true);
    widget
}
impl Ui {
    fn new(app: &gtk::Application, demo: bool) -> Rc<Self> {
        let (settings, warning) = if demo {
            (Settings::default(), None)
        } else {
            match Settings::load(&settings::path()) {
                Ok(data) => (data, None),
                Err(e) => (Settings::default(), Some(format!("{e:#}"))),
            }
        };
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("NVibrant")
            .default_width(640)
            .default_height(560)
            .icon_name("io.github.nvibrant.Aero")
            .build();
        window.add_css_class("aero");
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&wordmark("wordmark")));
        window.set_titlebar(Some(&header));
        let root = column(24);
        for side in [
            gtk::PositionType::Top,
            gtk::PositionType::Bottom,
            gtk::PositionType::Left,
            gtk::PositionType::Right,
        ] {
            match side {
                gtk::PositionType::Top => root.set_margin_top(30),
                gtk::PositionType::Bottom => root.set_margin_bottom(30),
                gtk::PositionType::Left => root.set_margin_start(30),
                _ => root.set_margin_end(30),
            }
        }
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&root)
            .build();
        window.set_child(Some(&scroll));
        let hero = row(12);
        let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(include_bytes!(
            "../assets/nvidia.svg"
        )))
        .expect("Embedded NVIDIA SVG must be valid");
        let logo = gtk::Image::from_paintable(Some(&gdk::Texture::for_pixbuf(&pixbuf)));
        logo.set_pixel_size(40);
        logo.set_valign(gtk::Align::Center);
        logo.update_property(&[gtk::accessible::Property::Label("NVIDIA")]);
        hero.append(&logo);
        let title = wordmark("headline");
        hero.append(&title);
        hero.append(&spacer());
        let refresh = gtk::Button::with_label("Refresh");
        refresh.set_valign(gtk::Align::Center);
        hero.append(&refresh);
        root.append(&hero);
        let cards = column(14);
        root.append(&cards);
        let empty = label("Finding displays…", "muted");
        empty.set_wrap(true);
        root.append(&empty);
        let actions = row(12);
        let reset = gtk::Button::with_label("Reset");
        let apply = gtk::Button::with_label("Apply");
        apply.add_css_class("suggested-action");
        actions.append(&reset);
        actions.append(&spacer());
        actions.append(&apply);
        root.append(&actions);
        let ui = Rc::new(Self {
            window,
            refresh,
            apply,
            reset,
            cards,
            empty,
            state: RefCell::new(State {
                displays: vec![],
                cards: vec![],
                settings,
            }),
            busy: Cell::new(false),
            demo,
        });
        let weak = Rc::downgrade(&ui);
        ui.refresh.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.discover();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.apply.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.apply_colours();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.reset.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                for card in &ui.state.borrow().cards {
                    card.scale.set_value(0.0);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.window.connect_close_request(move |_| {
            if weak.upgrade().is_some_and(|ui| ui.busy.get()) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        ui.window.present();
        ui.discover();
        if let Some(warning) = warning {
            ui.error("Saved settings could not be loaded", &warning);
        }
        ui
    }
    fn clear(&self) {
        self.state.borrow_mut().cards.clear();
        self.state.borrow_mut().displays.clear();
        while let Some(child) = self.cards.first_child() {
            self.cards.remove(&child);
        }
    }
    fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        let has_cards = !self.state.borrow().cards.is_empty();
        self.refresh.set_sensitive(!busy);
        self.cards.set_sensitive(!busy);
        self.reset.set_sensitive(!busy && has_cards);
        self.apply.set_sensitive(!busy && has_cards);
    }
    fn error(&self, title: &str, details: &str) {
        let dialog = gtk::MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .message_type(gtk::MessageType::Error)
            .buttons(gtk::ButtonsType::Close)
            .text(title)
            .secondary_text(details)
            .build();
        dialog.add_css_class("aero");
        dialog.connect_response(|dialog, _| dialog.close());
        dialog.present();
    }
    fn task<T: Send + 'static>(
        self: &Rc<Self>,
        work: impl FnOnce() -> Result<T> + Send + 'static,
        done: impl FnOnce(&Rc<Self>, Result<T>) + 'static,
    ) {
        self.set_busy(true);
        let (send, recv) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = send.send(work());
        });
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let result = loop {
                match recv.try_recv() {
                    Ok(result) => break result,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        break Err(anyhow!("Background request stopped unexpectedly"));
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        glib::timeout_future(Duration::from_millis(20)).await
                    }
                }
            };
            done(&ui, result);
            ui.set_busy(false);
        });
    }
    fn discover(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let demo = self.demo;
        self.refresh.set_label("Finding…");
        self.task(
            move || {
                if demo {
                    Ok(backend::demo_ports()
                        .into_iter()
                        .map(|port| Display { gpu: 0, port })
                        .collect())
                } else {
                    runtime::discover_connected(&Native, &backend::gpu_indices()?)
                }
            },
            move |ui, result| {
                ui.refresh.set_label("Refresh");
                match result {
                    Ok(displays) => ui.show_displays(displays),
                    Err(error) => {
                        ui.clear();
                        ui.empty.set_text("No NVIDIA displays available.");
                        ui.empty.set_visible(true);
                        ui.error("Could not detect NVIDIA displays", &format!("{error:#}"));
                    }
                }
            },
        );
    }
    fn show_displays(self: &Rc<Self>, displays: Vec<Display>) {
        let previous = self
            .state
            .borrow()
            .cards
            .iter()
            .map(|card| ((card.gpu, card.port.key()), card.scale.value()))
            .collect::<BTreeMap<_, _>>();
        self.clear();
        let multiple_gpus = displays
            .iter()
            .map(|d| d.gpu)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1;
        for display in &displays {
            let value = previous
                .get(&(display.gpu, display.port.key()))
                .copied()
                .unwrap_or_else(|| {
                    f64::from(
                        self.state
                            .borrow()
                            .settings
                            .value(display.gpu, &display.port)
                            .unwrap_or(0),
                    )
                });
            self.add_card(display.clone(), value, multiple_gpus);
        }
        self.state.borrow_mut().displays = displays;
        self.empty
            .set_text("No displays are connected to an NVIDIA GPU.");
        self.empty.set_visible(self.state.borrow().cards.is_empty());
    }
    fn add_card(&self, display: Display, initial: f64, multiple_gpus: bool) {
        let Display { gpu, port } = display;
        let card = column(14);
        card.add_css_class("glass-card");
        let top = row(12);
        let titles = column(4);
        titles.set_hexpand(true);
        titles.append(&label(
            &format!("{} · Port {}", port.kind, port.connector + 1),
            "card-title",
        ));
        if multiple_gpus {
            titles.append(&label(&format!("NVIDIA GPU {gpu}"), "muted"));
        }
        top.append(&titles);
        let number = label(&format_value(initial), "value");
        top.append(&number);
        card.append(&top);
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, -1024.0, 1023.0, 1.0);
        scale.set_draw_value(false);
        scale.set_value(initial);
        scale.set_hexpand(true);
        scale.update_property(&[gtk::accessible::Property::Label(&format!(
            "Vibrance for {} output {}",
            port.kind,
            port.index + 1
        ))]);
        scale.set_tooltip_text(Some("−1024 grayscale · 0 natural · 1023 maximum"));
        scale.connect_value_changed(move |scale| number.set_text(&format_value(scale.value())));
        card.append(&scale);
        let ends = row(0);
        ends.append(&label("Grayscale", "muted"));
        ends.append(&spacer());
        ends.append(&label("Full bloom", "muted"));
        card.append(&ends);
        let presets = row(8);
        for (name, value) in [
            ("Natural", 0.0),
            ("Fresh", 256.0),
            ("Vivid", 512.0),
            ("Luminous", 768.0),
        ] {
            let button = gtk::Button::with_label(name);
            let scale = scale.clone();
            button.connect_clicked(move |_| scale.set_value(value));
            presets.append(&button);
        }
        card.append(&presets);
        self.cards.append(&card);
        self.state
            .borrow_mut()
            .cards
            .push(Card { gpu, port, scale });
    }
    fn apply_colours(self: &Rc<Self>) {
        if self.busy.get() || self.state.borrow().cards.is_empty() {
            return;
        }
        let mut updates: BTreeMap<u8, BTreeMap<String, i32>> = BTreeMap::new();
        for card in &self.state.borrow().cards {
            updates
                .entry(card.gpu)
                .or_default()
                .insert(card.port.key(), card.scale.value().round() as i32);
        }
        let demo = self.demo;
        self.apply.set_label("Applying…");
        self.task(
            move || {
                if demo {
                    Ok(runtime::ApplyOutcome {
                        saved: BTreeMap::new(),
                        errors: vec![],
                    })
                } else {
                    runtime::apply_displays(&Native, &settings::path(), &updates)
                }
            },
            move |ui, result| {
                ui.apply.set_label("Apply");
                match result {
                    Err(error) => ui.error("Could not apply colours", &format!("{error:#}")),
                    Ok(outcome) => {
                        for (gpu, values) in outcome.saved {
                            let mut state = ui.state.borrow_mut();
                            state
                                .settings
                                .gpus
                                .entry(gpu.to_string())
                                .or_default()
                                .extend(values);
                            state.settings.last_gpu = gpu;
                        }
                        if !outcome.errors.is_empty() {
                            ui.error(
                                "Some colours could not be applied or saved",
                                &outcome.errors.join("\n\n"),
                            );
                        } else {
                            ui.apply
                                .set_label(if demo { "Previewed" } else { "Applied" });
                            let weak = Rc::downgrade(ui);
                            glib::timeout_add_local_once(Duration::from_secs(2), move || {
                                if let Some(ui) = weak.upgrade().filter(|ui| !ui.busy.get()) {
                                    ui.apply.set_label("Apply");
                                }
                            });
                        }
                    }
                }
            },
        );
    }
}
fn format_value(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        format!("{:+}", value.round() as i32)
    }
}

pub fn run(demo: bool, smoke: bool) -> Result<()> {
    let app = gtk::Application::builder()
        .application_id("io.github.nvibrant.Aero")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let failure = Rc::new(RefCell::new(None));
    let failure_for_app = failure.clone();
    app.connect_activate(move |app| {
        let css = gtk::CssProvider::new();
        css.load_from_data(include_str!("../assets/style.css"));
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("GTK display"),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let ui = Ui::new(app, demo);
        // Window signal handlers use Weak references; keep the controller alive
        // for the window lifetime without a window/controller reference cycle.
        let holder = Rc::new(RefCell::new(Some(ui.clone())));
        let holder_for_close = holder.clone();
        ui.window.connect_destroy(move |_| {
            holder_for_close.borrow_mut().take();
        });
        if smoke {
            let app = app.clone();
            let failure = failure_for_app.clone();
            glib::spawn_future_local(async move {
                let result = smoke_test(&ui).await;
                if let Err(error) = result {
                    *failure.borrow_mut() = Some(format!("{error:#}"));
                }
                ui.window.close();
                app.quit();
            });
        }
    });
    let code = app.run_with_args::<&str>(&[]);
    if let Some(error) = failure.borrow_mut().take() {
        return Err(anyhow!(error));
    }
    ensure!(code == glib::ExitCode::SUCCESS, "GTK exited with {code:?}");
    Ok(())
}
async fn settle(ui: &Ui) -> Result<()> {
    for _ in 0..200 {
        if !ui.busy.get() {
            return Ok(());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Err(anyhow!("GTK operation did not complete"))
}
async fn smoke_test(ui: &Rc<Ui>) -> Result<()> {
    settle(ui).await?;
    ensure!(
        ui.demo && ui.state.borrow().cards.len() == 2,
        "Demo discovery failed"
    );
    let scale = ui.state.borrow().cards[0].scale.clone();
    scale.set_value(512.0);
    ui.discover();
    settle(ui).await?;
    ensure!(
        ui.state.borrow().cards[0].scale.value() == 512.0,
        "Refresh lost pending edits"
    );
    ui.apply_colours();
    settle(ui).await?;
    ensure!(
        ui.apply.label().as_deref() == Some("Previewed"),
        "Apply failed"
    );
    ui.reset.emit_clicked();
    ensure!(
        ui.state
            .borrow()
            .cards
            .iter()
            .all(|c| c.scale.value() == 0.0),
        "Reset failed"
    );
    ui.discover();
    settle(ui).await?;
    ensure!(
        ui.state.borrow().displays.len() == 2,
        "Auto-detection failed"
    );
    // A compact screenshot of the actual Rust/GTK interface.
    ui.window.set_default_size(640, 720);
    glib::timeout_future(Duration::from_millis(500)).await;
    ui.apply.set_label("Apply");
    glib::timeout_future(Duration::from_millis(200)).await;
    if let Ok(path) = std::env::var("NVIBRANT_SCREENSHOT") {
        let paintable = gtk::WidgetPaintable::new(Some(&ui.window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(
            &snapshot,
            f64::from(ui.window.width()),
            f64::from(ui.window.height()),
        );
        let node = snapshot
            .to_node()
            .ok_or_else(|| anyhow!("Empty window snapshot"))?;
        let renderer = ui
            .window
            .renderer()
            .ok_or_else(|| anyhow!("No GTK renderer"))?;
        let texture = renderer.render_texture(&node, None);
        texture.save_to_png(path)?;
    }
    println!("Rust GTK smoke checks passed: auto-detection, refresh, apply, reset, demo isolation");
    Ok(())
}
