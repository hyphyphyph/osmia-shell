//! The settings window: pages on the left, settings on the right.
//! Every control writes one key to config.toml and the shell reloads it.

use egui::{pos2, vec2, Align, Align2, Layout, Rect, Sense, Ui};

use crate::config::Config;
use crate::theme::{self, fid};
use crate::ui::widgets::{id, pill, switch};
use crate::ui::{Action, ConfValue};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Appearance,
    Bar,
    Notifications,
    Launcher,
    DefaultApps,
    Capture,
    Power,
    Shortcuts,
    Session,
    About,
}

const PAGES: [(Page, &str); 10] = [
    (Page::Appearance, "Appearance"),
    (Page::Bar, "Bar"),
    (Page::Notifications, "Notifications"),
    (Page::Launcher, "Launcher"),
    (Page::DefaultApps, "Default applications"),
    (Page::Capture, "Screenshots and video"),
    (Page::Power, "Power and screen"),
    (Page::Shortcuts, "Keyboard shortcuts"),
    (Page::Session, "Session"),
    (Page::About, "About"),
];

const ACCENTS: [&str; 6] = ["#f0a35e", "#6cb6ff", "#7ee787", "#d2a8ff", "#ff7b72", "#e3b341"];

/// Bar items that can be placed, with the names shown in the editor.
const BAR_ITEMS: [(&str, &str); 16] = [
    ("windows", "Window menu"),
    ("attention", "Minimized windows"),
    ("media", "Now playing"),
    ("tray", "System tray"),
    ("cpu", "CPU"),
    ("ram", "Memory"),
    ("battery", "Battery"),
    ("network", "Wi-Fi"),
    ("volume", "Volume"),
    ("brightness", "Brightness"),
    ("notifications", "Notifications"),
    ("clock", "Clock"),
    ("capture", "Screenshot button"),
    ("settings", "Settings button"),
    ("power", "Power button"),
    ("|", "Divider"),
];

pub struct Env<'a> {
    pub screen_timeout: u32,
    pub lock_timeout: u32,
    /// Switch to this page (a lowercase page name or its first word).
    pub goto: Option<&'a str>,
    pub binds: &'a [crate::keybinds::Bind],
    pub binds_error: Option<&'a str>,
    /// The key the shortcut recorder heard. Empty means it was cancelled.
    pub captured: Option<&'a str>,
    pub defaults: &'a crate::defaults::DefaultApps,
}

pub fn window(ui: &mut Ui, cfg: &Config, env: &Env, actions: &mut Vec<Action>) {
    let page_id = id("settings-page");
    let mut page: Page = ui.data(|d| d.get_temp(page_id)).unwrap_or(Page::Appearance);
    if let Some(g) = env.goto {
        let g = g.to_lowercase();
        if let Some((p, _)) = PAGES.iter().find(|(_, n)| n.to_lowercase().starts_with(&g) || n.to_lowercase().split(' ').any(|w| w == g)) {
            page = *p;
        }
    }

    let nav = egui::Panel::left("settings-nav")
        .exact_size(200.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(theme::BAR))
        .show(ui, |ui| {
            ui.add_space(14.0);
            for (p, name) in PAGES {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 16.0, 32.0), Sense::click());
                let r = r.translate(vec2(8.0, 0.0));
                if p == page {
                    ui.painter().rect_filled(r, 4, theme::PLATE_SEL);
                    ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(2.0, r.height())), 0, theme::accent());
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 4, theme::PLATE);
                }
                ui.painter().text(pos2(r.min.x + 14.0, r.center().y), Align2::LEFT_CENTER, name, fid(13.0), if p == page { theme::TEXT } else { theme::DIM });
                if resp.clicked() {
                    page = p;
                }
            }
        });
    // Our own divider: a whole device pixel wide and fully opaque, so it
    // does not blur away at fractional scales.
    {
        let ppp = ui.ctx().pixels_per_point();
        let r = nav.response.rect;
        let x = ((r.max.x * ppp).round() - 0.5) / ppp;
        ui.painter().line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], egui::Stroke::new(1.0 / ppp, theme::CARD_LINE));
    }

    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::CARD)).show(ui, |ui| {
        let title = PAGES.iter().find(|(p, _)| *p == page).map(|(_, n)| *n).unwrap_or("");
        let (tr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
        ui.painter().text(pos2(tr.min.x + 28.0, tr.center().y + 4.0), Align2::LEFT_CENTER, title, fid(19.0), theme::TEXT);
        ui.painter().line_segment([tr.left_bottom(), tr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match page {
            Page::Appearance => appearance(ui, cfg, actions),
            Page::Bar => bar(ui, cfg, actions),
            Page::Notifications => notifications(ui, cfg, actions),
            Page::Launcher => launcher(ui, cfg, actions),
            Page::DefaultApps => default_apps(ui, cfg, env, actions),
            Page::Capture => capture(ui, cfg, actions),
            Page::Power => power(ui, env, actions),
            Page::Shortcuts => shortcuts(ui, env, actions),
            Page::Session => session(ui, cfg, actions),
            Page::About => about(ui, actions),
        });
    });

    ui.data_mut(|d| d.insert_temp(page_id, page));
}

// ---------- pages ----------

fn appearance(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    row(ui, "Accent color", "Used for highlights, the selected item and the focus mark.", |ui| {
        if let Some(v) = text_input(ui, "accent", &cfg.theme.accent, 96.0) {
            actions.push(set("theme.accent", s(&v)));
        }
        ui.add_space(8.0);
        for hex in ACCENTS.iter().rev() {
            let Some(col) = crate::config::parse_color(hex) else { continue };
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            ui.painter().rect_filled(r.shrink(2.0), 4, col);
            if cfg.theme.accent.eq_ignore_ascii_case(hex) {
                ui.painter().rect_stroke(r, 4, egui::Stroke::new(1.5, theme::TEXT), egui::StrokeKind::Inside);
            } else if resp.hovered() {
                ui.painter().rect_stroke(r, 4, egui::Stroke::new(1.0, theme::DIM), egui::StrokeKind::Inside);
            }
            if resp.clicked() {
                actions.push(set("theme.accent", s(hex)));
            }
        }
    });
    row(ui, "Font size", "Text size across the bar, panels and this window.", |ui| {
        let v = cfg.theme.font_size.round() as i32;
        match stepper(ui, "font", &format!("{v} pt")) {
            -1 if v > 9 => actions.push(set("theme.font_size", ConfValue::Float((v - 1) as f64))),
            1 if v < 18 => actions.push(set("theme.font_size", ConfValue::Float((v + 1) as f64))),
            _ => {}
        }
    });
    row(ui, "Icon theme", "Where app icons come from. System default follows your desktop.", |ui| {
        let themes: Vec<String> = ui.data_mut(|d| d.get_temp_mut_or_insert_with(id("icon-themes"), crate::appicon::installed_themes).clone());
        let current = cfg.theme.icon_theme.clone();
        let label = if current.is_empty() { "System default".to_owned() } else { current.clone() };
        egui::ComboBox::from_id_salt("icon-theme-combo").selected_text(label).width(190.0).height(320.0).show_ui(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if ui.selectable_label(current.is_empty(), "System default").clicked() {
                actions.push(set("theme.icon_theme", s("")));
            }
            for t in &themes {
                if ui.selectable_label(*t == current, t).clicked() {
                    actions.push(set("theme.icon_theme", s(t)));
                }
            }
        });
    });
}

fn bar(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    row(ui, "Position", "Which screen edge the bar sits on.", |ui| {
        if let Some(i) = segmented(ui, "pos", &["Top", "Bottom"], index_of(&["top", "bottom"], &cfg.bar.edge)) {
            actions.push(set("bar.edge", s(["top", "bottom"][i])));
        }
    });
    row(ui, "Height", "In pixels.", |ui| {
        let v = cfg.bar.height as i64;
        match stepper(ui, "bar-h", &format!("{v} px")) {
            -1 if v > 24 => actions.push(set("bar.height", ConfValue::Int(v - 2))),
            1 if v < 48 => actions.push(set("bar.height", ConfValue::Int(v + 2))),
            _ => {}
        }
    });
    let clocks = ["%H:%M", "%-I:%M %p", "%H:%M:%S"];
    row(ui, "Clock", "24-hour, 12-hour, or with seconds.", |ui| {
        if let Some(i) = segmented(ui, "clock", &["24-hour", "12-hour", "Seconds"], index_of(&clocks, &cfg.bar.clock_format)) {
            actions.push(set("bar.clock_format", s(clocks[i])));
        }
    });
    row(ui, "Clock format", "Any strftime pattern, such as %H:%M.", |ui| {
        if let Some(v) = text_input(ui, "clock-fmt", &cfg.bar.clock_format, 190.0) {
            actions.push(set("bar.clock_format", s(&v)));
        }
    });
    let dates = ["", "%a %-d %b", "%-d %b", "%Y-%m-%d"];
    row(ui, "Date", "Shown before the time.", |ui| {
        if let Some(i) = segmented(ui, "date", &["Hidden", "Mon 3 Nov", "3 Nov", "2026-11-03"], index_of(&dates, &cfg.bar.date_format)) {
            actions.push(set("bar.date_format", s(dates[i])));
        }
    });
    subhead(ui, "Layout", "Choose what the bar shows and in what order.");
    layout_editor(ui, cfg, actions);
}

fn notifications(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    let corners = ["bottom-right", "bottom-left", "top-right", "top-left"];
    row(ui, "Popup corner", "Where new notifications appear.", |ui| {
        if let Some(i) = segmented(ui, "corner", &["Bottom right", "Bottom left", "Top right", "Top left"], index_of(&corners, &cfg.notifications.corner)) {
            actions.push(set("notifications.corner", s(corners[i])));
        }
    });
    row(ui, "Popup time", "How long a popup stays when the app does not say.", |ui| {
        let secs = (cfg.notifications.default_timeout_ms / 1000) as i64;
        match stepper(ui, "popup-t", &format!("{secs} s")) {
            -1 if secs > 1 => actions.push(set("notifications.default_timeout_ms", ConfValue::Int((secs - 1) * 1000))),
            1 if secs < 60 => actions.push(set("notifications.default_timeout_ms", ConfValue::Int((secs + 1) * 1000))),
            _ => {}
        }
    });
    let sizes = [50usize, 100, 200, 500];
    row(ui, "History size", "How many notifications the bell keeps.", |ui| {
        let sel = sizes.iter().position(|v| *v == cfg.notifications.history_size);
        if let Some(i) = segmented(ui, "hist", &["50", "100", "200", "500"], sel) {
            actions.push(set("notifications.history_size", ConfValue::Int(sizes[i] as i64)));
        }
    });
    row(ui, "Do not disturb at start", "Popups stay hidden until you turn it off in the bell menu.", |ui| {
        if switch(ui, cfg.notifications.dnd_on_start) {
            actions.push(set("notifications.dnd_on_start", ConfValue::Bool(!cfg.notifications.dnd_on_start)));
        }
    });
}

fn launcher(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    overview_rows(ui, cfg, actions);
    subhead(ui, "Launcher", "The search box that opens with the Super key.");
    row(ui, "Width", "In pixels.", |ui| {
        let v = cfg.launcher.width as i64;
        match stepper(ui, "launch-w", &format!("{v} px")) {
            -1 if v > 400 => actions.push(set("launcher.width", ConfValue::Int(v - 20))),
            1 if v < 900 => actions.push(set("launcher.width", ConfValue::Int(v + 20))),
            _ => {}
        }
    });
    row(ui, "Visible results", "How many rows show before the list scrolls.", |ui| {
        let v = cfg.launcher.max_rows as i64;
        match stepper(ui, "launch-r", &v.to_string()) {
            -1 if v > 4 => actions.push(set("launcher.max_rows", ConfValue::Int(v - 1))),
            1 if v < 14 => actions.push(set("launcher.max_rows", ConfValue::Int(v + 1))),
            _ => {}
        }
    });
}

fn overview_rows(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    subhead(ui, "Overview", "Every open window as a card. Type to filter, click to switch, Escape to close.");
    row(ui, "Hot corner", "Rest the pointer in the top left corner of the screen to open it. Needs the bar on top. `osmia overview toggle` works from a key.", |ui| {
        let opts = ["Top left", "Off"];
        let cur = if cfg.overview.corner == "top-left" { 0 } else { 1 };
        if let Some(i) = segmented(ui, "ov-corner", &opts, Some(cur)) {
            actions.push(set("overview.corner", s(if i == 0 { "top-left" } else { "off" })));
        }
    });
}

fn default_apps(ui: &mut Ui, cfg: &Config, env: &Env, actions: &mut Vec<Action>) {
    for (i, cat) in crate::defaults::CATEGORIES.iter().enumerate() {
        let Some(r) = env.defaults.rows.get(i) else { continue };
        row(ui, cat.label, cat.help, |ui| {
            let label = r.current.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| "Not set".to_owned());
            egui::ComboBox::from_id_salt(("default-app", i)).selected_text(crate::ui::popups::clip(&label, 28)).width(220.0).height(320.0).show_ui(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if r.apps.is_empty() {
                    ui.label("No installed app says it opens these.");
                }
                for a in &r.apps {
                    let on = r.current.as_ref().is_some_and(|c| c.id == a.id);
                    if ui.selectable_label(on, &a.name).clicked() && !on {
                        actions.push(Action::SetDefaultApp(i, a.id.clone()));
                    }
                }
            });
        });
    }
    row(ui, "Terminal", "Apps that need a terminal run in this one. Leave it empty to use the TERMINAL variable.", |ui| {
        if let Some(v) = text_input(ui, "terminal", &cfg.launcher.terminal, 160.0) {
            actions.push(set("launcher.terminal", s(&v)));
        }
    });
}

fn capture(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    let c = &cfg.capture;
    subhead(ui, "Screenshots", "Print opens the capture toolbar. Alt+Print captures the focused window, Ctrl+Print the whole screen.");
    row(ui, "Pictures folder", "Screenshots are saved here, named by date and time.", |ui| {
        if let Some(v) = text_input(ui, "cap-pics", &c.pictures_dir, 260.0) {
            actions.push(set("capture.pictures_dir", s(&v)));
        }
    });
    row(ui, "Copy to clipboard", "Every screenshot is also put on the clipboard.", |ui| {
        if switch(ui, c.copy) {
            actions.push(set("capture.copy", ConfValue::Bool(!c.copy)));
        }
    });
    row(ui, "Include the mouse cursor", "The starting choice. You can change it in the toolbar.", |ui| {
        if switch(ui, c.cursor) {
            actions.push(set("capture.cursor", ConfValue::Bool(!c.cursor)));
        }
    });
    let delays = [0i64, 3, 5, 10];
    row(ui, "Delay", "Wait before capturing, to set up the shot.", |ui| {
        let sel = delays.iter().position(|d| *d == c.delay as i64);
        if let Some(i) = segmented(ui, "cap-delay", &["None", "3 s", "5 s", "10 s"], sel) {
            actions.push(set("capture.delay", ConfValue::Int(delays[i])));
        }
    });
    subhead(ui, "Video", "Shift+Print records an area. Press any capture key, or the red timer in the bar, to stop.");
    row(ui, "Videos folder", "Recordings are saved here as MP4.", |ui| {
        if let Some(v) = text_input(ui, "cap-vids", &c.videos_dir, 260.0) {
            actions.push(set("capture.videos_dir", s(&v)));
        }
    });
    let fps = [24i64, 30, 60];
    row(ui, "Frame rate", "Frames per second.", |ui| {
        let sel = fps.iter().position(|f| *f == c.fps as i64);
        if let Some(i) = segmented(ui, "cap-fps", &["24", "30", "60"], sel) {
            actions.push(set("capture.fps", ConfValue::Int(fps[i])));
        }
    });
    let enc = ["auto", "hardware", "software"];
    row(ui, "Encoder", "Auto uses the graphics chip when it can, which is lighter on the processor.", |ui| {
        if let Some(i) = segmented(ui, "cap-enc", &["Auto", "Hardware", "Software"], index_of(&enc, &c.encoder)) {
            actions.push(set("capture.encoder", s(enc[i])));
        }
    });
    let q = ["small", "balanced", "high"];
    row(ui, "Quality", "Higher quality makes bigger files.", |ui| {
        if let Some(i) = segmented(ui, "cap-q", &["Small", "Balanced", "High"], index_of(&q, &c.quality)) {
            actions.push(set("capture.quality", s(q[i])));
        }
    });
    let au = ["none", "system", "mic", "both"];
    row(ui, "Sound", "The starting choice. You can change it in the toolbar.", |ui| {
        if let Some(i) = segmented(ui, "cap-au", &["Off", "System", "Mic", "Both"], index_of(&au, &c.audio)) {
            actions.push(set("capture.audio", s(au[i])));
        }
    });
    let ex = ["none", "gif", "webm"];
    row(ui, "Also make a copy as", "Made after each recording. The notification also has buttons for this.", |ui| {
        if let Some(i) = segmented(ui, "cap-ex", &["Nothing", "GIF", "WebM"], index_of(&ex, &c.export)) {
            actions.push(set("capture.export", s(ex[i])));
        }
    });
}

fn power(ui: &mut Ui, env: &Env, actions: &mut Vec<Action>) {
    row(ui, "Turn the screen off after", "When the machine has been idle. Also on the brightness menu.", |ui| {
        let label = crate::idle::label(env.screen_timeout);
        egui::ComboBox::from_id_salt("screen-off-combo").selected_text(label).width(150.0).show_ui(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for m in crate::idle::CHOICES {
                if ui.selectable_label(m == env.screen_timeout, crate::idle::label(m)).clicked() {
                    actions.push(Action::SetScreenTimeout(m));
                }
            }
        });
    });
    row(ui, "Lock after", "Locks with swaylock after this much idle time.", |ui| {
        let choices = [0u32, 1, 2, 5, 10, 15, 30, 60];
        let label = crate::idle::label(env.lock_timeout);
        egui::ComboBox::from_id_salt("lock-after-combo").selected_text(label).width(150.0).show_ui(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for m in choices {
                if ui.selectable_label(m == env.lock_timeout, crate::idle::label(m)).clicked() {
                    actions.push(Action::SetLockTimeout(m));
                }
            }
        });
    });
}

fn session(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    row(ui, "Lock command", "Run by the power menu.", |ui| {
        if let Some(v) = text_input(ui, "lock-cmd", &cfg.session.lock, 260.0) {
            actions.push(set("session.lock", s(&v)));
        }
    });
    row(ui, "Log out command", "Run by the power menu after you confirm.", |ui| {
        if let Some(v) = text_input(ui, "logout-cmd", &cfg.session.logout, 260.0) {
            actions.push(set("session.logout", s(&v)));
        }
    });
}

fn shortcuts(ui: &mut Ui, env: &Env, actions: &mut Vec<Action>) {
    use crate::keybinds::{command_title, conflicts, Act};
    const NEW: usize = usize::MAX;
    const FIND: usize = usize::MAX - 1;
    let rec_id = id("sc-rec");
    let find_id = id("sc-find");
    let mut find: String = ui.data(|d| d.get_temp(find_id)).unwrap_or_default();
    let newkey_id = id("sc-newkey");
    let newcmd_id = id("sc-newcmd");
    let confirm_id = id("sc-confirm");
    let mut rec: Option<usize> = ui.data(|d| d.get_temp(rec_id)).flatten();
    let mut new_key: String = ui.data(|d| d.get_temp(newkey_id)).unwrap_or_default();
    let mut new_cmd: String = ui.data(|d| d.get_temp(newcmd_id)).unwrap_or_default();
    let mut confirm: bool = ui.data(|d| d.get_temp(confirm_id)).unwrap_or(false);

    // The recorder heard a key.
    if let Some(k) = env.captured {
        if !k.is_empty() {
            match rec {
                Some(NEW) => new_key = k.to_owned(),
                Some(FIND) => find = k.to_owned(),
                Some(i) => actions.push(Action::ShortcutSetKey(i, k.to_owned())),
                None => {}
            }
        }
        rec = None;
        actions.push(Action::ShortcutTaken);
    }

    subhead(
        ui,
        "Keyboard shortcuts",
        "Saved to labwc's rc.xml and applied at once. Click a key to record a new one.",
    );
    if let Some(e) = env.binds_error {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
        ui.painter().text(pos2(r.min.x + 28.0, r.center().y), Align2::LEFT_CENTER, e, fid(12.0), theme::accent());
    }
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    ui.painter().text(
        pos2(r.min.x + 28.0, r.center().y),
        Align2::LEFT_CENTER,
        "Click a row to type the keys or edit the command. Keys already in use must be typed.",
        fid(11.0),
        theme::DIM,
    );

    let open_id = id("sc-open");
    let mut open: Option<usize> = ui.data(|d| d.get_temp(open_id)).flatten();
    let mut added = false;

    // Search and add.
    {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
        let pad = 28.0;
        let new_w = 140.0;
        let rec_w = 118.0;
        let search_r = Rect::from_min_max(
            pos2(r.min.x + pad, r.center().y - 14.0),
            pos2(r.max.x - pad - new_w - rec_w - 16.0, r.center().y + 14.0),
        );
        ui.painter().rect(search_r, 4, theme::BTN, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
        let mut sc = ui.new_child(egui::UiBuilder::new().max_rect(search_r.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center)));
        sc.add(
            egui::TextEdit::singleline(&mut find)
                .id(id("sc-find-edit"))
                .hint_text("Search by keys, name or command")
                .frame(egui::Frame::NONE)
                .font(fid(12.0))
                .desired_width(search_r.width() - 16.0),
        );
        let btn = |ui: &mut Ui, rect: Rect, label: &str, on: bool, key: &str| -> bool {
            let resp = ui.interact(rect, id(("sc-tb", key)), Sense::click());
            let (fill, stroke, col) = if on {
                (theme::accent_bg(), theme::accent(), theme::accent())
            } else if resp.hovered() {
                (theme::PLATE_SEL, theme::CARD_LINE, theme::TEXT)
            } else {
                (theme::BTN, theme::CARD_LINE, theme::TEXT)
            };
            ui.painter().rect(rect, 4, fill, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, fid(12.0), col);
            resp.clicked()
        };
        let rec_r = Rect::from_min_size(pos2(search_r.max.x + 8.0, search_r.min.y), vec2(rec_w, 28.0));
        let new_r = Rect::from_min_size(pos2(rec_r.max.x + 8.0, search_r.min.y), vec2(new_w, 28.0));
        let finding = rec == Some(FIND);
        if btn(ui, rec_r, if finding { "Press keys\u{2026}" } else { "Find by keys" }, finding, "find") {
            if finding {
                rec = None;
                actions.push(Action::ShortcutRecord(false));
            } else {
                rec = Some(FIND);
                actions.push(Action::ShortcutRecord(true));
            }
        }
        let adding = open == Some(NEW) || rec == Some(NEW);
        if btn(ui, new_r, "+ New shortcut", adding, "new") {
            open = if adding { None } else { Some(NEW) };
            if rec == Some(NEW) {
                rec = None;
                actions.push(Action::ShortcutRecord(false));
            }
        }
    }

    if open == Some(NEW) || rec == Some(NEW) {
        let chip = if rec == Some(NEW) { "Press keys\u{2026}".to_owned() } else if new_key.is_empty() { "Record keys".to_owned() } else { new_key.clone() };
        let mut ed = |ui: &mut Ui, w: f32| {
            if let Some(v) = text_input(ui, "sc-new-key", &new_key, 130.0) {
                new_key = v;
            }
            let cw = (w - 130.0 - 8.0 - 64.0 - 8.0).max(80.0);
            if let Some(v) = text_input(ui, "sc-new-cmd", &new_cmd, cw) {
                new_cmd = v;
            }
            let ok = crate::keybinds::valid_key(&new_key) && !new_cmd.trim().is_empty();
            if pill(ui, "Add", ok) && ok {
                actions.push(Action::ShortcutAdd(new_key.clone(), new_cmd.clone()));
                new_key.clear();
                new_cmd.clear();
                added = true;
            }
        };
        let (chip_click, _) = sc_card(
            ui,
            "new",
            "New shortcut",
            "Record the keys, then type the command to run.",
            false,
            &chip,
            rec == Some(NEW),
            Some(&mut ed as &mut dyn FnMut(&mut Ui, f32)),
        );
        if added {
            open = None;
        }
        if chip_click {
            if rec == Some(NEW) {
                rec = None;
                actions.push(Action::ShortcutRecord(false));
            } else {
                rec = Some(NEW);
                actions.push(Action::ShortcutRecord(true));
            }
        }
    }

    let clash = conflicts(env.binds);
    let mut shown = 0;
    for (i, b) in env.binds.iter().enumerate() {
        let (title, mut sub) = match &b.act {
            Act::Exec(c) => (command_title(c), c.clone()),
            Act::Other(l) => (l.clone(), "Window manager action".to_owned()),
        };
        let q = find.trim().to_lowercase();
        if !q.is_empty() {
            let hay = format!("{} {} {}", b.key, title, sub).to_lowercase();
            let qn = crate::keybinds::canonical(&find).to_lowercase();
            if !hay.contains(&q) && !(qn.len() > 1 && b.key.to_lowercase() == qn) {
                continue;
            }
            shown += 1;
        } else {
            shown += 1;
        }
        let warn = clash[i].is_some();
        if let Some(j) = clash[i] {
            sub = format!("Same keys as \u{201c}{}\u{201d} above, which wins.", env.binds[j].key);
        }
        let chip = if rec == Some(i) { "Press keys\u{2026}".to_owned() } else { b.key.clone() };
        let is_open = open == Some(i);
        let mut removed = false;
        let mut ed = |ui: &mut Ui, w: f32| {
            if let Some(v) = text_input(ui, &format!("sc-key-{i}"), &b.key, 130.0) {
                if v != b.key {
                    actions.push(Action::ShortcutSetKey(i, v));
                }
            }
            let cw = (w - 130.0 - 8.0 - 72.0 - 8.0).max(80.0);
            if let Act::Exec(c) = &b.act {
                if let Some(v) = text_input(ui, &format!("sc-cmd-{i}"), c, cw) {
                    if v != *c {
                        actions.push(Action::ShortcutSetCommand(i, v));
                    }
                }
            } else {
                ui.add_space(cw + 8.0);
            }
            if pill(ui, "Remove", false) {
                actions.push(Action::ShortcutDelete(i));
                removed = true;
            }
        };
        let (chip_click, row_click) = sc_card(
            ui,
            &format!("b{i}"),
            &title,
            &sub,
            warn,
            &chip,
            rec == Some(i),
            is_open.then_some(&mut ed as &mut dyn FnMut(&mut Ui, f32)),
        );
        if removed {
            open = None;
        }
        if chip_click {
            if rec == Some(i) {
                rec = None;
                actions.push(Action::ShortcutRecord(false));
            } else {
                rec = Some(i);
                actions.push(Action::ShortcutRecord(true));
            }
        }
        if row_click {
            open = if open == Some(i) { None } else { Some(i) };
        }
    }

    if shown == 0 {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
        ui.painter().text(pos2(r.min.x + 28.0, r.center().y), Align2::LEFT_CENTER, "No shortcut matches.", fid(12.0), theme::DIM);
    }
    row(ui, "Restore the default shortcuts", "Replaces your list with the one that came with Osmia. Your file is kept as rc.xml.bak-osmia.", |ui| {
        if confirm {
            if pill(ui, "Yes, restore", true) {
                actions.push(Action::ShortcutsRestore);
                confirm = false;
            }
            if pill(ui, "Cancel", false) {
                confirm = false;
            }
        } else if pill(ui, "Restore\u{2026}", false) {
            confirm = true;
        }
    });
    ui.add_space(20.0);

    ui.data_mut(|d| {
        d.insert_temp(rec_id, rec);
        d.insert_temp(find_id, find);
        d.insert_temp(newkey_id, new_key);
        d.insert_temp(newcmd_id, new_cmd);
        d.insert_temp(confirm_id, confirm);
        d.insert_temp(open_id, open);
    });
}

fn about(ui: &mut Ui, actions: &mut Vec<Action>) {
    row(ui, "osmia", concat!("Version ", env!("CARGO_PKG_VERSION")), |_| {});
    let path = crate::config::path();
    row(ui, "Config file", &path.display().to_string(), |ui| {
        if pill(ui, "Open", false) {
            actions.push(Action::OpenConfigFile);
        }
    });
    row(ui, "Reload from file", "Use after editing the file by hand.", |ui| {
        if pill(ui, "Reload", false) {
            actions.push(Action::ReloadConfig);
        }
    });
    let armed_id = id("reset-armed");
    let armed: bool = ui.data(|d| d.get_temp(armed_id)).unwrap_or(false);
    row(ui, "Reset all settings", "Goes back to the defaults. Your file is kept as config.toml.bak.", |ui| {
        let label = if armed { "Click again to confirm" } else { "Reset" };
        if pill(ui, label, armed) {
            if armed {
                actions.push(Action::ResetConfig);
            }
            ui.data_mut(|d| d.insert_temp(armed_id, !armed));
        }
    });
}

// ---------- the bar layout editor ----------

fn layout_editor(ui: &mut Ui, cfg: &Config, actions: &mut Vec<Action>) {
    let regions: [(&str, &str, &Vec<String>); 3] = [
        ("Left", "bar.layout.left", &cfg.bar.layout.left),
        ("Center", "bar.layout.center", &cfg.bar.layout.center),
        ("Right", "bar.layout.right", &cfg.bar.layout.right),
    ];
    let used: Vec<&String> = regions.iter().flat_map(|r| r.2.iter()).collect();
    for (name, key, list) in regions {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
        ui.painter().text(pos2(r.min.x + 28.0, r.center().y + 4.0), Align2::LEFT_CENTER, name, fid(12.0), theme::DIM);
        for (i, item) in list.iter().enumerate() {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width() - 56.0, 30.0), Sense::hover());
            let r = r.translate(vec2(28.0, 0.0));
            ui.painter().rect_filled(r, 4, theme::PLATE);
            let label = BAR_ITEMS.iter().find(|(k, _)| *k == item).map(|(_, n)| *n).unwrap_or(item.as_str());
            ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, label, fid(12.0), theme::TEXT);
            let mut x = r.max.x - 8.0;
            let mut btn = |ui: &mut Ui, glyph: &str, tag: &str, enabled: bool| -> bool {
                let b = Rect::from_center_size(pos2(x - 12.0, r.center().y), vec2(24.0, 22.0));
                x -= 28.0;
                let resp = ui.interact(b, id(("lay", key, i, tag)), if enabled { Sense::click() } else { Sense::hover() });
                if resp.hovered() && enabled {
                    ui.painter().rect_filled(b, 4, theme::PLATE_SEL);
                }
                ui.painter().text(b.center(), Align2::CENTER_CENTER, glyph, fid(13.0), if enabled { theme::TEXT } else { theme::LINE });
                enabled && resp.clicked()
            };
            let mut next: Option<Vec<String>> = None;
            if btn(ui, "\u{d7}", "x", true) {
                let mut l = list.clone();
                l.remove(i);
                next = Some(l);
            }
            if btn(ui, "\u{2193}", "d", i + 1 < list.len()) {
                let mut l = list.clone();
                l.swap(i, i + 1);
                next = Some(l);
            }
            if btn(ui, "\u{2191}", "u", i > 0) {
                let mut l = list.clone();
                l.swap(i, i - 1);
                next = Some(l);
            }
            if let Some(l) = next {
                actions.push(set(key, ConfValue::List(l)));
            }
            ui.add_space(3.0);
        }
        // Add an item.
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width() - 56.0, 30.0), Sense::hover());
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r.translate(vec2(28.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
        egui::ComboBox::from_id_salt(("add", key)).selected_text("Add item").width(170.0).show_ui(&mut c, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (k, label) in BAR_ITEMS {
                if k != "|" && used.iter().any(|u| u.as_str() == k) {
                    continue;
                }
                if ui.selectable_label(false, label).clicked() {
                    let mut l = list.clone();
                    l.push(k.to_owned());
                    actions.push(set(key, ConfValue::List(l)));
                }
            }
        });
        ui.add_space(6.0);
    }
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(28.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
    if pill(&mut c, "Reset layout", false) {
        let d = crate::config::Layout::default();
        actions.push(set("bar.layout.left", ConfValue::List(d.left)));
        actions.push(set("bar.layout.center", ConfValue::List(d.center)));
        actions.push(set("bar.layout.right", ConfValue::List(d.right)));
    }
}

// ---------- building blocks ----------

fn set(key: &str, v: ConfValue) -> Action {
    Action::SetConfig(key.to_owned(), v)
}

fn s(v: &str) -> ConfValue {
    ConfValue::Str(v.to_owned())
}

fn index_of(options: &[&str], current: &str) -> Option<usize> {
    options.iter().position(|o| *o == current)
}

fn subhead(ui: &mut Ui, title: &str, desc: &str) {
    ui.add_space(14.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
    ui.painter().text(pos2(r.min.x + 28.0, r.min.y + 14.0), Align2::LEFT_CENTER, title, fid(15.0), theme::TEXT);
    ui.painter().text(pos2(r.min.x + 28.0, r.min.y + 33.0), Align2::LEFT_CENTER, desc, fid(11.0), theme::DIM);
}

/// A setting: name and help on the left, the control on the right.
fn row(ui: &mut Ui, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    let pad = 28.0;
    let text_w = ((w - pad * 2.0) * 0.46).max(160.0);
    let tg = ui.painter().layout(title.to_owned(), fid(13.0), theme::TEXT, text_w);
    let dg = (!desc.is_empty()).then(|| ui.painter().layout(desc.to_owned(), fid(11.0), theme::DIM, text_w));
    let text_h = tg.size().y + dg.as_ref().map(|g| g.size().y + 3.0).unwrap_or(0.0);
    let h = (text_h + 24.0).max(52.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let top = r.min.y + (h - text_h) / 2.0;
    let title_h = tg.size().y;
    ui.painter().galley(pos2(r.min.x + pad, top), tg, theme::TEXT);
    if let Some(g) = dg {
        ui.painter().galley(pos2(r.min.x + pad, top + title_h + 3.0), g, theme::DIM);
    }
    let area = Rect::from_min_max(pos2(r.min.x + pad + text_w + 12.0, r.min.y), pos2(r.max.x - pad, r.max.y));
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(Layout::right_to_left(Align::Center)));
    control(&mut c);
    ui.painter().line_segment([pos2(r.min.x + pad, r.max.y), pos2(r.max.x - pad, r.max.y)], egui::Stroke::new(1.0, theme::LINE));
}

/// A shortcut row: name and what it runs on the left, a key chip on the right.
/// Click the chip to record new keys. Click anywhere else to open the editor
/// under the row. Returns (chip clicked, row clicked).
fn sc_card(
    ui: &mut Ui,
    key: &str,
    title: &str,
    sub: &str,
    sub_warn: bool,
    chip: &str,
    recording: bool,
    editor: Option<&mut dyn FnMut(&mut Ui, f32)>,
) -> (bool, bool) {
    let w = ui.available_width();
    let pad = 28.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
    let chip_w = (ui.painter().layout_no_wrap(chip.to_owned(), fid(12.0), theme::TEXT).size().x + 26.0).clamp(90.0, 190.0);
    let chip_r = Rect::from_center_size(pos2(r.max.x - pad - chip_w / 2.0, r.center().y), vec2(chip_w, 28.0));
    let chip_resp = ui.interact(chip_r, id(("sc-chip", key)), Sense::click());
    if resp.hovered() && !chip_resp.hovered() {
        ui.painter().rect_filled(r.shrink2(vec2(10.0, 3.0)), 4, theme::PLATE);
    }
    let text_w = chip_r.min.x - (r.min.x + pad) - 14.0;
    let max_chars = (text_w / 6.6).max(8.0) as usize;
    ui.painter().text(pos2(r.min.x + pad, r.center().y - 9.0), Align2::LEFT_CENTER, crate::ui::popups::clip(title, max_chars), fid(13.0), theme::TEXT);
    ui.painter().text(
        pos2(r.min.x + pad, r.center().y + 9.0),
        Align2::LEFT_CENTER,
        crate::ui::popups::clip(sub, max_chars + 6),
        fid(11.0),
        if sub_warn { theme::accent() } else { theme::DIM },
    );
    let (fill, stroke, col) = if recording {
        (theme::accent_bg(), theme::accent(), theme::accent())
    } else if chip_resp.hovered() {
        (theme::PLATE_SEL, theme::CARD_LINE, theme::TEXT)
    } else {
        (theme::BTN, theme::CARD_LINE, theme::TEXT)
    };
    ui.painter().rect(chip_r, 4, fill, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    ui.painter().text(chip_r.center(), Align2::CENTER_CENTER, chip, fid(12.0), col);
    if let Some(ed) = editor {
        let (er, _) = ui.allocate_exact_size(vec2(w, 48.0), Sense::hover());
        let area = Rect::from_min_max(pos2(er.min.x + pad, er.min.y), pos2(er.max.x - pad, er.max.y - 4.0));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(Layout::left_to_right(Align::Center)));
        ed(&mut c, area.width());
    }
    ui.painter().line_segment([pos2(r.min.x + pad, ui.cursor().min.y), pos2(r.max.x - pad, ui.cursor().min.y)], egui::Stroke::new(1.0, theme::LINE));
    (chip_resp.clicked(), resp.clicked() && !chip_resp.hovered())
}

/// Joined buttons, one lit. Returns the index clicked.
pub(crate) fn segmented(ui: &mut Ui, key: &str, opts: &[&str], selected: Option<usize>) -> Option<usize> {
    let widths: Vec<f32> = opts
        .iter()
        .map(|o| ui.painter().layout_no_wrap((*o).to_owned(), fid(12.0), theme::TEXT).size().x + 22.0)
        .collect();
    let total: f32 = widths.iter().sum();
    let (rect, _) = ui.allocate_exact_size(vec2(total, 26.0), Sense::hover());
    ui.painter().rect(rect, 4, theme::BTN, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
    let mut x = rect.min.x;
    let mut clicked = None;
    for (i, o) in opts.iter().enumerate() {
        let seg = Rect::from_min_size(pos2(x, rect.min.y), vec2(widths[i], 26.0));
        let resp = ui.interact(seg, id(("seg", key, i)), Sense::click());
        let on = selected == Some(i);
        if on {
            ui.painter().rect(seg.shrink(1.0), 3, theme::accent_bg(), egui::Stroke::new(1.0, theme::accent()), egui::StrokeKind::Inside);
        } else if resp.hovered() {
            ui.painter().rect_filled(seg.shrink(1.0), 3, theme::PLATE_SEL);
        }
        ui.painter().text(seg.center(), Align2::CENTER_CENTER, *o, fid(12.0), if on { theme::accent() } else { theme::TEXT });
        if resp.clicked() {
            clicked = Some(i);
        }
        x += widths[i];
    }
    clicked
}

/// [-] value [+]. Returns -1, 0 or 1.
pub(crate) fn stepper(ui: &mut Ui, key: &str, value: &str) -> i32 {
    let (rect, _) = ui.allocate_exact_size(vec2(150.0, 26.0), Sense::hover());
    let minus = Rect::from_min_size(rect.min, vec2(26.0, 26.0));
    let plus = Rect::from_min_size(pos2(rect.max.x - 26.0, rect.min.y), vec2(26.0, 26.0));
    let mut out = 0;
    for (r, glyph, delta) in [(minus, "\u{2212}", -1), (plus, "+", 1)] {
        let resp = ui.interact(r, id(("step", key, delta)), Sense::click());
        ui.painter().rect(r, 4, if resp.hovered() { theme::PLATE_SEL } else { theme::BTN }, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph, fid(14.0), theme::TEXT);
        if resp.clicked() {
            out = delta;
        }
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, value, fid(12.0), theme::TEXT);
    out
}

/// A text box that saves when you press Enter or click away.
fn text_input(ui: &mut Ui, key: &str, current: &str, width: f32) -> Option<String> {
    let (r, _) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    ui.painter().rect(r, 4, theme::BTN, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
    let eid = id(("cfgtext", key));
    let focused = ui.memory(|m| m.has_focus(eid));
    let mut draft: String = if focused { ui.data_mut(|d| d.get_temp(eid)).unwrap_or_else(|| current.to_owned()) } else { current.to_owned() };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center)));
    let resp = child.add(
        egui::TextEdit::singleline(&mut draft)
            .id(eid)
            .frame(egui::Frame::NONE)
            .desired_width(r.width() - 16.0)
            .font(fid(12.0)),
    );
    ui.data_mut(|d| d.insert_temp(eid, draft.clone()));
    (resp.lost_focus() && draft != current && !draft.trim().is_empty()).then(|| draft.trim().to_owned())
}
