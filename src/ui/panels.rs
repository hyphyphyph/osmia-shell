//! Panels that open from the bar: history, calendar, power menu.
//! Each is a floating card, 6 px from the bar, over a full-screen click catcher.

use chrono::{Datelike, Local, NaiveDate};
use egui::{pos2, vec2, Align2, Rect, Sense, Ui};

use crate::icons::{self, Icon};
use crate::notif::{strip_markup, Notification, Notifs, URGENCY_CRITICAL};
use crate::theme;
use crate::ui::popups::clip;
use crate::ui::widgets::{age, card_frame, pill, switch, tile};
use crate::ui::{Action, Overlay, PowerAction};

pub struct Placement {
    /// Right edge of the bar item that opened the panel.
    pub anchor_x: f32,
    pub bar_height: f32,
    pub bar_on_top: bool,
}

pub fn show(
    ui: &mut Ui,
    overlay: &Overlay,
    place: &Placement,
    notifs: &Notifs,
    tray: &[crate::tray::Item],
    tray_menu: &[crate::tray::MenuNode],
    tray_menu_loaded: bool,
    wifi: &[crate::sys::WifiNet],
    wifi_scanning: bool,
    screen_timeout: u32,
    brightness: Option<u32>,
    audio: &crate::sys::AudioInfo,
    battery: Option<&crate::sys::BatteryDetail>,
    profiles: &crate::sys::PowerProfiles,
    media: &crate::mpris::State,
    windows: &crate::ui::WinList,
    procs: &crate::sys::ProcList,
    actions: &mut Vec<Action>,
) {
    let full = ui.max_rect();
    // Click outside the card closes the panel.
    let bg = ui.interact(full, crate::ui::widgets::id("overlay-bg"), Sense::click());
    if bg.clicked() {
        actions.push(Action::CloseOverlay);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        actions.push(Action::CloseOverlay);
    }
    // egui sizes scroll areas and new windows from the previous frame, so a
    // panel that has just opened or just got its content is drawn too small until
    // something else triggers a redraw. Draw a few extra frames to let it settle.
    {
        let key = (format!("{overlay:?}"), tray_menu.len(), tray_menu_loaded, notifs.history.len());
        let sid = crate::ui::widgets::id("panel-settle");
        let prev: Option<((String, usize, bool, usize), u8)> = ui.data(|d| d.get_temp(sid));
        let left = match prev {
            Some((last, left)) if last == key => left,
            _ => 4,
        };
        if left > 0 {
            ui.ctx().request_repaint();
        }
        ui.data_mut(|d| d.insert_temp(sid, (key, left.saturating_sub(1))));
    }
    // A long tray menu gets more columns, then scrolls. See tray_layout.
    let tray_cols = match overlay {
        Overlay::TrayMenu(id) => Some(tray_layout(full, place, tray.iter().find(|i| i.id == *id), tray_menu)),
        _ => None,
    };
    // The most height a panel's list can take: the screen less the bar and edges.
    let avail = (full.height() - place.bar_height - 6.0 - 16.0).max(200.0);
    ui.ctx().data_mut(|d| d.insert_temp(crate::ui::widgets::id("panel-avail"), avail));
    let (width, pivot_x) = match overlay {
        Overlay::History => (420.0, place.anchor_x),
        Overlay::Calendar(_) => (260.0, place.anchor_x),
        Overlay::Power(_) => (250.0, place.anchor_x),
        Overlay::TrayMenu(_) => {
            let cols = tray_cols.map(|t| t.0).unwrap_or(1);
            let w = (cols as f32 * MENU_COL_W).max(270.0);
            (w, place.anchor_x)
        }
        Overlay::Network => (260.0, place.anchor_x),
        Overlay::Screen => (230.0, place.anchor_x),
        Overlay::Audio => (320.0, place.anchor_x),
        Overlay::Battery => (300.0, place.anchor_x),
        Overlay::Media => (340.0, place.anchor_x),
        Overlay::Windows => (360.0, place.anchor_x),
        Overlay::Cpu | Overlay::Ram => {
            // Wide enough for the longest name. It only grows while the card is open,
            // so the card does not shrink and grow as the numbers change.
            let wid = crate::ui::widgets::id("proc-width");
            let open: Option<String> = ui.ctx().data(|d| d.get_temp(crate::ui::widgets::id("proc-open"))).flatten();
            let need = proc_width(ui, procs, matches!(overlay, Overlay::Cpu), open.as_deref(), (full.width() - 16.0).min(PROC_MAX_W));
            let w = ui.ctx().data(|d| d.get_temp::<f32>(wid)).unwrap_or(0.0).max(need).min(full.width() - 16.0);
            ui.ctx().data_mut(|d| d.insert_temp(wid, w));
            (w, place.anchor_x)
        }
        Overlay::Launcher => return,
    };
    // Centred under the item that opened it, kept on screen.
    let left = (pivot_x - width / 2.0).min(full.max.x - 8.0 - width).max(8.0);
    let (pos, pivot) = if place.bar_on_top {
        (pos2(left, place.bar_height + 6.0), Align2::LEFT_TOP)
    } else {
        (pos2(left, full.max.y - place.bar_height - 6.0), Align2::LEFT_BOTTOM)
    };
    // A tray panel changes size when its menu arrives. A fresh Area per shape
    // lets egui place it from its real size instead of the old one.
    let shape = match overlay {
        Overlay::TrayMenu(_) => (tray_cols.map(|t| t.0).unwrap_or(1), tray_menu_loaded, tray_menu.len()),
        Overlay::Cpu | Overlay::Ram => {
            let cpu = matches!(overlay, Overlay::Cpu);
            let open: Option<String> = ui.ctx().data(|d| d.get_temp(crate::ui::widgets::id("proc-open"))).flatten();
            (proc_scroll_h(&proc_rows(procs, cpu), open.as_deref(), avail - PROC_CHROME) as usize, procs.ready, procs.lock.is_some() as usize + 2 * width as usize)
        }
        _ => (0, true, 0),
    };
    egui::Area::new(crate::ui::widgets::id(("panel", shape)))
        .fixed_pos(pos)
        .pivot(pivot)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            card_frame().show(ui, |ui| {
                ui.set_width(width);
                match overlay {
                    Overlay::History => history(ui, notifs, actions),
                    Overlay::Calendar(off) => calendar(ui, *off, actions, overlay),
                    Overlay::Power(confirm) => power(ui, *confirm, actions),
                    Overlay::TrayMenu(id) => tray_panel(ui, id, tray, tray_menu, tray_menu_loaded, tray_cols.unwrap_or((1, 400.0)), media, actions),
                    Overlay::Network => network_panel(ui, wifi, wifi_scanning, actions),
                    Overlay::Screen => screen_panel(ui, screen_timeout, brightness, actions),
                    Overlay::Audio => audio_panel(ui, audio, actions),
                    Overlay::Battery => battery_panel(ui, battery, profiles, actions),
                    Overlay::Media => media_panel(ui, media, actions),
                    Overlay::Windows => windows_panel(ui, windows, actions),
                    Overlay::Cpu => proc_panel(ui, procs, true, actions),
                    Overlay::Ram => proc_panel(ui, procs, false, actions),
                    Overlay::Launcher => {}
                }
            });
        });
}

fn history(ui: &mut Ui, notifs: &Notifs, actions: &mut Vec<Action>) {
    // Header.
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    let p = ui.painter();
    p.text(pos2(hr.min.x + 14.0, hr.center().y), Align2::LEFT_CENTER, "Notifications", crate::theme::fid(13.0), theme::TEXT);
    p.text(
        pos2(hr.max.x - 14.0, hr.center().y),
        Align2::RIGHT_CENTER,
        format!("{} kept of {}", notifs.history.len(), notifs.history_cap),
        crate::theme::fid(11.0),
        theme::DIM,
    );
    p.line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));

    if notifs.history.is_empty() {
        let (er, _) = ui.allocate_exact_size(vec2(ui.available_width(), 130.0), Sense::hover());
        let p = ui.painter();
        let ic = Rect::from_center_size(pos2(er.center().x, er.min.y + 40.0), vec2(30.0, 30.0));
        icons::draw(p, if notifs.dnd { Icon::BellOff } else { Icon::Bell }, ic, theme::DIM);
        p.text(pos2(er.center().x, er.min.y + 70.0), Align2::CENTER_CENTER, "Nothing missed", crate::theme::fid(13.0), theme::TEXT);
        let sub = if notifs.dnd {
            "Do not disturb is on. Popups are hidden. Critical ones still show."
        } else {
            "Notifications that you miss will wait here."
        };
        p.text(pos2(er.center().x, er.min.y + 90.0), Align2::CENTER_CENTER, sub, crate::theme::fid(11.0), theme::DIM);
    } else {
        egui::ScrollArea::vertical().max_height((panel_avail(ui) - 110.0).max(120.0)).auto_shrink([false, true]).show(ui, |ui| {
            for n in &notifs.history {
                entry(ui, n, actions);
            }
        });
    }

    // Footer.
    let (fr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    ui.painter().line_segment([fr.left_top(), fr.right_top()], egui::Stroke::new(1.0, theme::LINE));
    let mut left = ui.new_child(egui::UiBuilder::new().max_rect(fr.shrink2(vec2(14.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    if switch(&mut left, notifs.dnd) {
        actions.push(Action::ToggleDnd);
    }
    left.add_space(8.0);
    left.label(egui::RichText::new("Do not disturb").size(crate::theme::scaled(12.0)).color(theme::TEXT));
    let mut right = ui.new_child(egui::UiBuilder::new().max_rect(fr.shrink2(vec2(14.0, 0.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
    if pill(&mut right, "Clear all", false) {
        actions.push(Action::ClearHistory);
    }
}

fn entry(ui: &mut Ui, n: &Notification, actions: &mut Vec<Action>) {
    let body = strip_markup(&n.body);
    let lines = (body.chars().count() as f32 / 52.0).ceil().clamp(0.0, 3.0);
    let has_buttons = n.buttons().next().is_some();
    let h = 50.0 + lines * 15.0 + if has_buttons { 32.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    // Not resp.hovered(): that goes false while the pointer is over the close button,
    // which would hide the button under the pointer and flicker.
    let row_hot = ui.rect_contains_pointer(r);
    let p = ui.painter();
    if row_hot {
        p.rect_filled(r, 0, theme::PLATE);
    }
    p.line_segment([r.left_bottom(), r.right_bottom()], egui::Stroke::new(1.0, egui::Color32::from_rgb(0x2b, 0x30, 0x37)));
    if n.urgency == URGENCY_CRITICAL {
        p.rect_filled(Rect::from_min_max(r.min, pos2(r.min.x + 3.0, r.max.y)), 0, theme::accent());
    }
    if n.unread {
        p.rect_filled(Rect::from_center_size(pos2(r.min.x + 6.0, r.min.y + 18.0), vec2(5.0, 5.0)), 2, theme::accent());
    }
    tile(p, pos2(r.min.x + 26.0, r.min.y + 20.0), 16.0, &n.icon_key());
    p.text(pos2(r.min.x + 40.0, r.min.y + 20.0), Align2::LEFT_CENTER, &n.app_name, crate::theme::fid(11.0), theme::TEXT);
    p.text(pos2(r.max.x - 14.0, r.min.y + 20.0), Align2::RIGHT_CENTER, age(n.born), crate::theme::fid(11.0), theme::DIM);
    p.text(pos2(r.min.x + 40.0, r.min.y + 39.0), Align2::LEFT_CENTER, clip(&n.summary, 46), crate::theme::fid(13.0), theme::TEXT);
    if lines > 0.0 {
        let g = p.layout(body, crate::theme::fid(12.0), theme::DIM, r.width() - 60.0);
        p.with_clip_rect(Rect::from_min_size(r.min + vec2(40.0, 48.0), vec2(r.width() - 54.0, lines * 15.0 + 2.0)))
            .galley(pos2(r.min.x + 40.0, r.min.y + 48.0), g, theme::DIM);
    }
    if has_buttons {
        let mut bx = r.min.x + 40.0;
        let by = r.max.y - 30.0;
        for (key, label) in n.buttons().take(4) {
            let rect = Rect::from_min_size(pos2(bx, by), vec2(label.chars().count() as f32 * 6.5 + 22.0, 22.0));
            let b = ui.interact(rect, crate::ui::widgets::id(("hbtn", n.id, key.as_str())), Sense::click());
            let fill = if b.hovered() { theme::PLATE_SEL } else { theme::BTN };
            ui.painter().rect(rect, 4, fill, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, crate::theme::fid(12.0), theme::TEXT);
            if b.clicked() {
                actions.push(Action::NotifButton(n.id, key.clone()));
            }
            bx += rect.width() + 6.0;
        }
    }
    if row_hot {
        let x = Rect::from_center_size(pos2(r.max.x - 22.0, r.min.y + 38.0), vec2(18.0, 18.0));
        let xr = ui.interact(x, crate::ui::widgets::id(("hx", n.id)), Sense::click());
        ui.painter().rect_filled(x, 4, if xr.hovered() { theme::PLATE_SEL } else { theme::BTN });
        icons::draw(ui.painter(), Icon::Close, x.shrink(3.0), theme::DIM);
        if xr.clicked() {
            actions.push(Action::HistoryRemove(n.id));
            return;
        }
    }
    if resp.clicked() {
        actions.push(Action::NotifClick(n.id));
    }
}

fn calendar(ui: &mut Ui, offset: i32, actions: &mut Vec<Action>, overlay: &Overlay) {
    let _ = overlay;
    let today = Local::now().date_naive();
    let total = today.year() * 12 + today.month0() as i32 + offset;
    let (year, month) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let days = {
        let next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) }.unwrap();
        (next - first).num_days() as u32
    };
    let lead = first.weekday().num_days_from_monday();

    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    let title = first.format("%B %Y").to_string();
    ui.painter().text(hr.center(), Align2::CENTER_CENTER, title, crate::theme::fid(13.0), theme::TEXT);
    for (dx, delta, ch) in [(-1.0, -1, "\u{2039}"), (1.0, 1, "\u{203a}")] {
        let b = Rect::from_center_size(pos2(hr.center().x + dx * (hr.width() / 2.0 - 22.0), hr.center().y), vec2(24.0, 24.0));
        let r = ui.interact(b, crate::ui::widgets::id(("calnav", delta)), Sense::click());
        if r.hovered() {
            ui.painter().rect_filled(b, 4, theme::PLATE);
        }
        ui.painter().text(b.center(), Align2::CENTER_CENTER, ch, crate::theme::fid(16.0), theme::DIM);
        if r.clicked() {
            actions.push(Action::ToggleOverlay(Overlay::Calendar(offset + delta), f32::NAN));
        }
    }
    let cell = (ui.available_width() - 16.0) / 7.0;
    let rows = (lead + days).div_ceil(7);
    let (gr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0 + rows as f32 * 32.0 + 10.0), Sense::hover());
    let x0 = gr.min.x + 8.0;
    for (i, d) in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"].iter().enumerate() {
        ui.painter().text(pos2(x0 + cell * (i as f32 + 0.5), gr.min.y + 8.0), Align2::CENTER_CENTER, d, crate::theme::fid(10.0), theme::DIM);
    }
    for day in 1..=days {
        let idx = lead + day - 1;
        let (c, r) = (idx % 7, idx / 7);
        let center = pos2(x0 + cell * (c as f32 + 0.5), gr.min.y + 28.0 + r as f32 * 32.0 + 14.0);
        let is_today = year == today.year() && month == today.month() && day == today.day();
        if is_today {
            ui.painter().rect_filled(Rect::from_center_size(center, vec2(28.0, 26.0)), 4, theme::accent());
        }
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            day.to_string(),
            crate::theme::fid(12.0),
            if is_today { egui::Color32::from_rgb(0x14, 0x16, 0x1a) } else { theme::TEXT },
        );
    }
}

fn power(ui: &mut Ui, confirm: Option<PowerAction>, actions: &mut Vec<Action>) {
    ui.add_space(6.0);
    let mut list = vec![PowerAction::Lock, PowerAction::Suspend];
    if crate::sys::can_hibernate() {
        list.push(PowerAction::Hibernate);
    }
    let split = list.len();
    list.extend([PowerAction::Logout, PowerAction::Reboot, PowerAction::PowerOff]);
    for (i, a) in list.into_iter().enumerate() {
        if i == split {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
            ui.painter().line_segment([pos2(r.min.x + 8.0, r.center().y), pos2(r.max.x - 8.0, r.center().y)], egui::Stroke::new(1.0, theme::LINE));
        }
        if confirm == Some(a) {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 64.0), Sense::hover());
            let r = r.translate(vec2(6.0, 0.0));
            ui.painter().rect(r, 4, theme::accent_bg(), egui::Stroke::new(1.0, theme::accent()), egui::StrokeKind::Inside);
            ui.painter().text(pos2(r.min.x + 12.0, r.min.y + 18.0), Align2::LEFT_CENTER, format!("{} now?", a.label()), crate::theme::fid(12.0), theme::TEXT);
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(r.min.x + 10.0, r.max.y - 28.0), pos2(r.max.x - 10.0, r.max.y - 6.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
            if pill(&mut c, a.label(), true) {
                actions.push(Action::Power(a));
            }
            c.add_space(6.0);
            if pill(&mut c, "Cancel", false) {
                actions.push(Action::ToggleOverlay(Overlay::Power(None), f32::NAN));
            }
            continue;
        }
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 32.0), Sense::click());
        let r = r.translate(vec2(6.0, 0.0));
        if resp.hovered() {
            ui.painter().rect_filled(r, 4, theme::PLATE);
        }
        let ic = match a {
            PowerAction::Lock => Icon::Lock,
            _ => Icon::Power,
        };
        icons::draw(ui.painter(), ic, Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(14.0, 14.0)), theme::DIM);
        ui.painter().text(pos2(r.min.x + 34.0, r.center().y), Align2::LEFT_CENTER, a.label(), crate::theme::fid(12.0), theme::TEXT);
        if resp.clicked() {
            if a.needs_confirm() {
                actions.push(Action::ToggleOverlay(Overlay::Power(Some(a)), f32::NAN));
            } else {
                actions.push(Action::Power(a));
            }
        }
    }
    ui.add_space(6.0);
}

fn tray_panel(
    ui: &mut Ui,
    id: &str,
    items: &[crate::tray::Item],
    nodes: &[crate::tray::MenuNode],
    loaded: bool,
    (cols, rows_h): (usize, f32),
    media: &crate::mpris::State,
    actions: &mut Vec<Action>,
) {
    let Some(item) = items.iter().find(|i| i.id == id) else {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::hover());
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "This item is gone", crate::theme::fid(12.0), theme::DIM);
        return;
    };
    // Header: icon, name, state, and what the app says about itself.
    let status = match item.status.as_str() {
        "NeedsAttention" => "Needs attention",
        "Passive" => "Idle",
        "Active" => "Active",
        _ => "",
    };
    let mut lines: Vec<String> = Vec::new();
    if !item.tip_title.is_empty() && item.tip_title != item.title {
        lines.push(item.tip_title.clone());
    }
    if !item.tip_text.is_empty() {
        lines.push(item.tip_text.clone());
    }
    let tip = lines.join(" \u{b7} ");
    let galley = (!tip.is_empty()).then(|| ui.painter().layout(tip, crate::theme::fid(11.0), theme::DIM, ui.available_width() - 68.0));
    let h = 56.0 + galley.as_ref().map(|g| g.size().y + 4.0).unwrap_or(0.0);
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let ic = pos2(hr.min.x + 32.0, hr.min.y + 30.0);
    match crate::tray::icon_texture(ui.ctx(), item) {
        Some(tex) => {
            ui.painter().image(
                tex.id(),
                Rect::from_center_size(ic, vec2(28.0, 28.0)),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => crate::ui::widgets::tile(ui.painter(), ic, 28.0, &format!("{}\n{}", item.id, item.title)),
    }
    ui.painter().text(pos2(hr.min.x + 56.0, hr.min.y + 22.0), Align2::LEFT_CENTER, clip(&item.title, 26), crate::theme::fid(13.0), theme::TEXT);
    let col = if item.status == "NeedsAttention" { theme::accent() } else { theme::DIM };
    ui.painter().text(pos2(hr.min.x + 56.0, hr.min.y + 40.0), Align2::LEFT_CENTER, status, crate::theme::fid(11.0), col);
    if let Some(g) = galley {
        ui.painter().galley(pos2(hr.min.x + 56.0, hr.min.y + 52.0), g, theme::DIM);
    }
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));
    // A media player that belongs to this app gets its controls right here.
    let names: Vec<String> = item
        .app_key
        .split('\n')
        .chain(std::iter::once(item.title.as_str()))
        .chain(std::iter::once(item.id.rsplit('/').next().unwrap_or("")))
        .map(str::to_owned)
        .collect();
    if let Some(p) = media.for_app(&names) {
        media_card(ui, p, media.art.get(&p.art_url).map(|a| a.as_ref()), actions);
        let (s, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().line_segment([s.left_center(), s.right_center()], egui::Stroke::new(1.0, theme::LINE));
    }
    ui.add_space(6.0);

    // Open: one named action, only when the app has an "open" to call.
    if !item.item_is_menu {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 32.0), Sense::click());
        let r = r.translate(vec2(6.0, 0.0));
        if resp.hovered() {
            ui.painter().rect_filled(r, 4, theme::PLATE);
        }
        ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, format!("Open {}", clip(&item.title, 22)), crate::theme::fid(12.0), theme::TEXT);
        if resp.clicked() {
            actions.push(Action::TrayActivate(item.id.clone()));
        }
        if item.menu.is_some() {
            let (s, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
            ui.painter().line_segment([pos2(s.min.x + 8.0, s.center().y), pos2(s.max.x - 8.0, s.center().y)], egui::Stroke::new(1.0, theme::LINE));
        }
    }
    // The app's menu, drawn here.
    if item.menu.is_some() {
        if !loaded {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
            ui.painter().text(pos2(r.min.x + 18.0, r.center().y), Align2::LEFT_CENTER, "Loading menu", crate::theme::fid(12.0), theme::DIM);
        } else if nodes.is_empty() {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
            ui.painter().text(pos2(r.min.x + 18.0, r.center().y), Align2::LEFT_CENTER, "The app sent an empty menu", crate::theme::fid(12.0), theme::DIM);
        }
        menu_columns(ui, id, nodes, cols, rows_h, actions);
        // Escape hatch for menus that do not draw well here.
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 28.0), Sense::click());
        let r = r.translate(vec2(6.0, 0.0));
        if resp.hovered() {
            ui.painter().rect_filled(r, 4, theme::PLATE);
        }
        ui.painter().text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, "Show the app's own menu", crate::theme::fid(11.0), theme::DIM);
        if resp.clicked() {
            actions.push(Action::TrayNativeMenu(item.id.clone()));
        }
    }
    ui.add_space(6.0);
}

const MENU_COL_W: f32 = 250.0;
const SEP_H: f32 = 9.0;
const ROW_H: f32 = 32.0;

struct FlatRow<'a> {
    node: &'a crate::tray::MenuNode,
    depth: usize,
}

impl FlatRow<'_> {
    fn height(&self) -> f32 {
        if self.node.separator { SEP_H } else { ROW_H }
    }
}

fn flatten<'a>(nodes: &'a [crate::tray::MenuNode], depth: usize, out: &mut Vec<FlatRow<'a>>) {
    for n in nodes {
        out.push(FlatRow { node: n, depth });
        flatten(&n.children, depth + 1, out);
    }
}

/// Split rows into `cols` columns of about equal height, keeping their order.
fn split_columns<'a, 'b>(rows: &'b [FlatRow<'a>], cols: usize) -> Vec<&'b [FlatRow<'a>]> {
    let total: f32 = rows.iter().map(FlatRow::height).sum();
    let target = total / cols as f32;
    let mut out = Vec::new();
    let (mut start, mut acc) = (0, 0.0);
    for (i, r) in rows.iter().enumerate() {
        let h = r.height();
        // Start a new column when this row would push the current one past the target,
        // unless it is the last column.
        if out.len() + 1 < cols && acc > 0.0 && acc + h / 2.0 > target {
            out.push(&rows[start..i]);
            start = i;
            acc = 0.0;
        }
        acc += h;
    }
    out.push(&rows[start..]);
    out
}

/// How a tray panel is laid out: (columns, height available for the menu rows).
/// One column if the menu fits on screen. Otherwise more columns, as many as needed,
/// as long as the panel still fits across the screen. If it is still too tall,
/// the columns scroll. The panel stays on screen sideways by sliding toward the
/// side with more room, which `show` does when it places the card.
fn tray_layout(full: Rect, place: &Placement, item: Option<&crate::tray::Item>, nodes: &[crate::tray::MenuNode]) -> (usize, f32) {
    let mut flat = Vec::new();
    flatten(nodes, 0, &mut flat);
    // Header, open row, footer, and the card's margins.
    let tip_lines = item.map(|i| usize::from(!i.tip_text.is_empty()) + usize::from(!i.tip_title.is_empty() && i.tip_title != i.title)).unwrap_or(0);
    let fixed = 56.0 + 18.0 * tip_lines as f32 + 6.0 + ROW_H + SEP_H + 28.0 + 6.0 + 16.0;
    let avail = (full.height() - place.bar_height - 6.0 - 16.0 - fixed).max(120.0);
    let max_cols = (((full.width() - 16.0) / MENU_COL_W).floor() as usize).clamp(1, 4);
    let total: f32 = flat.iter().map(FlatRow::height).sum();
    for cols in 1..=max_cols {
        let tallest = split_columns(&flat, cols)
            .iter()
            .map(|c| c.iter().map(FlatRow::height).sum::<f32>())
            .fold(0.0, f32::max);
        if tallest <= avail {
            return (cols, avail);
        }
    }
    let _ = total;
    (max_cols, avail)
}

fn menu_columns(ui: &mut Ui, id: &str, nodes: &[crate::tray::MenuNode], cols: usize, max_h: f32, actions: &mut Vec<Action>) {
    let mut flat = Vec::new();
    flatten(nodes, 0, &mut flat);
    let groups = split_columns(&flat, cols.max(1));
    // Size the scroll area from the rows themselves. Left to egui it sizes from
    // the previous frame, which was drawn before the menu arrived.
    let tallest = groups.iter().map(|g| g.iter().map(FlatRow::height).sum::<f32>()).fold(0.0, f32::max);
    let h = tallest.min(max_h);
    egui::ScrollArea::vertical().min_scrolled_height(h).max_height(h).auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for g in groups {
                ui.allocate_ui_with_layout(vec2(MENU_COL_W, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(MENU_COL_W);
                    for r in g {
                        menu_row(ui, id, r, actions);
                    }
                });
            }
        });
    });
}

fn menu_row(ui: &mut Ui, id: &str, row: &FlatRow, actions: &mut Vec<Action>) {
    let (n, depth) = (row.node, row.depth);
    if n.separator {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), SEP_H), Sense::hover());
        ui.painter().line_segment([pos2(r.min.x + 8.0, r.center().y), pos2(r.max.x - 8.0, r.center().y)], egui::Stroke::new(1.0, theme::LINE));
        return;
    }
    let sense = if n.enabled { Sense::click() } else { Sense::hover() };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, ROW_H), sense);
    let r = r.translate(vec2(6.0, 0.0));
    if resp.hovered() && n.enabled {
        ui.painter().rect_filled(r, 4, theme::PLATE);
    }
    let x = r.min.x + 10.0 + depth as f32 * 14.0;
    if n.checked == Some(true) {
        icons::draw(ui.painter(), Icon::Check, Rect::from_center_size(pos2(x + 6.0, r.center().y), vec2(12.0, 12.0)), theme::accent());
    }
    let col = if n.enabled { theme::TEXT } else { egui::Color32::from_rgb(0x5e, 0x64, 0x6d) };
    ui.painter().text(pos2(x + 20.0, r.center().y), Align2::LEFT_CENTER, clip(&n.label, 26), crate::theme::fid(12.0), col);
    if !n.children.is_empty() {
        icons::draw(ui.painter(), Icon::Chevron, Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y), vec2(12.0, 12.0)), theme::DIM);
    }
    if resp.clicked() && n.children.is_empty() {
        actions.push(Action::TrayMenuClick(id.to_owned(), n.id));
    }
}

/// Every open window, by workspace and then by application.
fn windows_panel(ui: &mut Ui, list: &crate::ui::WinList, actions: &mut Vec<Action>) {
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y), Align2::LEFT_CENTER, "Windows", crate::theme::fid(13.0), theme::TEXT);
    ui.painter().text(pos2(hr.max.x - 14.0, hr.center().y), Align2::RIGHT_CENTER, list.wins.len().to_string(), crate::theme::fid(11.0), theme::DIM);
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));
    if list.wins.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 70.0), Sense::hover());
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "No windows are open", crate::theme::fid(12.0), theme::DIM);
        return;
    }
    // Sections: each workspace that has windows, then windows whose place is not known.
    let several = list.spaces.len() > 1;
    let mut sections: Vec<(Option<&crate::ui::WsEntry>, Vec<&crate::ui::WinEntry>)> = vec![];
    if several {
        for sp in &list.spaces {
            let mine: Vec<_> = list.wins.iter().filter(|w| w.ws.as_deref() == Some(sp.name.as_str())).collect();
            if !mine.is_empty() || sp.active {
                sections.push((Some(sp), mine));
            }
        }
        let known: Vec<&str> = list.spaces.iter().map(|s| s.name.as_str()).collect();
        let rest: Vec<_> = list.wins.iter().filter(|w| !w.ws.as_deref().is_some_and(|n| known.contains(&n))).collect();
        if !rest.is_empty() {
            sections.push((None, rest));
        }
    } else {
        sections.push((list.spaces.first(), list.wins.iter().collect()));
    }
    egui::ScrollArea::vertical().max_height((panel_avail(ui) - 90.0).max(120.0)).auto_shrink([false, true]).show(ui, |ui| {
        for (sp, wins) in &sections {
            if several {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), if sp.is_some() { Sense::click() } else { Sense::hover() });
                if resp.hovered() && sp.is_some() {
                    ui.painter().rect_filled(r.shrink2(vec2(6.0, 1.0)), 4, theme::PLATE);
                }
                let (name, active) = match sp {
                    Some(s) => (s.name.clone(), s.active),
                    None => ("Other workspaces".to_owned(), false),
                };
                ui.painter().text(pos2(r.min.x + 16.0, r.center().y + 2.0), Align2::LEFT_CENTER, name.to_uppercase(), crate::theme::fid(10.5), if active { theme::accent() } else { theme::DIM });
                if active {
                    ui.painter().text(pos2(r.max.x - 14.0, r.center().y + 2.0), Align2::RIGHT_CENTER, "here", crate::theme::fid(10.5), theme::accent());
                } else if resp.hovered() {
                    ui.painter().text(pos2(r.max.x - 14.0, r.center().y + 2.0), Align2::RIGHT_CENTER, "Go", crate::theme::fid(10.5), theme::TEXT);
                }
                if let (Some(s), true) = (sp, resp.clicked()) {
                    actions.push(Action::GotoWorkspace(s.index));
                }
                if wins.is_empty() {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
                    ui.painter().text(pos2(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, "No windows", crate::theme::fid(11.0), theme::DIM);
                }
            }
            // Group by application, in the order they first appear.
            let mut groups: Vec<(&str, Vec<&crate::ui::WinEntry>)> = vec![];
            for w in wins {
                match groups.iter_mut().find(|g| g.0 == w.app_id) {
                    Some(g) => g.1.push(w),
                    None => groups.push((w.app_id.as_str(), vec![w])),
                }
            }
            for (_, ws) in &groups {
                if ws.len() > 1 {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
                    tile(ui.painter(), pos2(r.min.x + 24.0, r.center().y), 16.0, &format!("{}\n{}", ws[0].app_id, ws[0].title));
                    ui.painter().text(pos2(r.min.x + 40.0, r.center().y), Align2::LEFT_CENTER, &ws[0].app_name, crate::theme::fid(12.0), theme::TEXT);
                    ui.painter().text(pos2(r.max.x - 14.0, r.center().y), Align2::RIGHT_CENTER, ws.len().to_string(), crate::theme::fid(11.0), theme::DIM);
                }
                for w in ws {
                    window_row(ui, w, ws.len() > 1, actions);
                }
            }
            ui.add_space(4.0);
        }
    });
}

fn window_row(ui: &mut Ui, w: &crate::ui::WinEntry, nested: bool, actions: &mut Vec<Action>) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 28.0), Sense::click());
    let r = r.translate(vec2(6.0, 0.0));
    if resp.hovered() {
        ui.painter().rect_filled(r, 4, theme::PLATE);
    } else if w.active {
        ui.painter().rect_filled(r, 4, theme::accent_bg());
    }
    let x = r.min.x + if nested { 34.0 } else { 18.0 };
    if !nested {
        tile(ui.painter(), pos2(x, r.center().y), 16.0, &format!("{}\n{}", w.app_id, w.title));
    }
    let tx = if nested { x } else { x + 14.0 };
    let col = if w.minimized { theme::DIM } else { theme::TEXT };
    let right = if w.minimized { "minimized" } else if nested { "" } else { w.app_name.as_str() };
    let max = if right.is_empty() { 40 } else { 28 };
    ui.painter().text(pos2(tx, r.center().y), Align2::LEFT_CENTER, clip(&w.title, max), crate::theme::fid(12.0), col);
    let mut rx = r.max.x - 10.0;
    if w.alert {
        ui.painter().circle_filled(pos2(rx - 3.0, r.center().y), 3.5, theme::accent());
        rx -= 14.0;
    }
    if !right.is_empty() {
        ui.painter().text(pos2(rx, r.center().y), Align2::RIGHT_CENTER, clip(right, 12), crate::theme::fid(10.5), theme::DIM);
    }
    if resp.clicked() {
        actions.push(Action::FocusWindow(w.index));
    }
}

fn clock(us: i64) -> String {
    let s = (us / 1_000_000).max(0);
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn media_panel(ui: &mut Ui, media: &crate::mpris::State, actions: &mut Vec<Action>) {
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y), Align2::LEFT_CENTER, "Now playing", crate::theme::fid(13.0), theme::TEXT);
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));
    if media.players.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 80.0), Sense::hover());
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "Nothing is playing", crate::theme::fid(12.0), theme::DIM);
        return;
    }
    for (i, p) in media.players.iter().enumerate() {
        if i > 0 {
            let (s, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().line_segment([s.left_center(), s.right_center()], egui::Stroke::new(1.0, theme::LINE));
        }
        let (lr, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        ui.painter().text(pos2(lr.min.x + 14.0, lr.center().y + 2.0), Align2::LEFT_CENTER, &p.identity, crate::theme::fid(11.0), theme::DIM);
        let open = pos2(lr.max.x - 14.0, lr.center().y + 2.0);
        ui.painter().text(open, Align2::RIGHT_CENTER, "Open", crate::theme::fid(11.0), if resp.hovered() { theme::TEXT } else { theme::DIM });
        if resp.clicked() {
            actions.push(Action::MediaRaise(p.bus.clone()));
        }
        media_card(ui, p, media.art.get(&p.art_url).map(|a| a.as_ref()), actions);
    }
    ui.add_space(4.0);
}

/// Cover, title, a bar to seek with, and the three buttons.
fn media_card(ui: &mut Ui, p: &crate::mpris::Player, art: Option<&crate::capture::grab::Image>, actions: &mut Vec<Action>) {
    use crate::mpris::Status;
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 164.0), Sense::hover());
    // The cover.
    let cover = Rect::from_min_size(pos2(r.min.x + 14.0, r.min.y + 8.0), vec2(72.0, 72.0));
    match art {
        Some(img) => {
            let tid = crate::ui::widgets::id(("mpris-art", &p.art_url));
            let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(tid)).unwrap_or_else(|| {
                let ci = egui::ColorImage::from_rgba_unmultiplied([img.w as usize, img.h as usize], &img.rgba);
                let t = ui.ctx().load_texture(format!("art-{}", p.art_url), ci, egui::TextureOptions::LINEAR);
                ui.ctx().data_mut(|d| d.insert_temp(tid, t.clone()));
                t
            });
            ui.painter().image(tex.id(), cover, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        None => crate::ui::widgets::tile(ui.painter(), cover.center(), 72.0, &format!("{}\n{}", p.desktop_entry, p.identity)),
    }
    let tx = cover.max.x + 14.0;
    let tw = (r.max.x - 14.0 - tx).max(60.0);
    let wrap = |ui: &Ui, text: &str, size: f32, color: egui::Color32| ui.painter().layout(text.to_owned(), crate::theme::fid(size), color, tw);
    let title = if p.title.is_empty() { p.identity.clone() } else { p.title.clone() };
    let mut y = r.min.y + 10.0;
    for (text, size, color, lines) in [(title, 13.5, theme::TEXT, 2usize), (p.artist.clone(), 11.5, theme::DIM, 1), (p.album.clone(), 11.0, theme::DIM, 1)] {
        if text.is_empty() {
            continue;
        }
        let g = wrap(ui, &text, size, color);
        let line_h = g.size().y / g.rows.len().max(1) as f32;
        let h = line_h * g.rows.len().min(lines) as f32;
        ui.painter().with_clip_rect(Rect::from_min_size(pos2(tx, y), vec2(tw, h + 1.0))).galley(pos2(tx, y), g, color);
        y += h + 3.0;
    }
    // The progress bar. Click or drag to jump.
    let pos = p.position_now();
    let bar = Rect::from_min_max(pos2(r.min.x + 14.0, r.min.y + 96.0), pos2(r.max.x - 14.0, r.min.y + 100.0));
    ui.painter().rect_filled(bar, 2, theme::BTN);
    let frac = if p.length_us > 0 { (pos as f32 / p.length_us as f32).clamp(0.0, 1.0) } else { 0.0 };
    ui.painter().rect_filled(Rect::from_min_max(bar.min, pos2(bar.min.x + bar.width() * frac, bar.max.y)), 2, theme::accent());
    if p.length_us > 0 && p.can_seek {
        let hit = bar.expand2(vec2(0.0, 8.0));
        let resp = ui.interact(hit, crate::ui::widgets::id(("mpris-seek", &p.bus)), Sense::click_and_drag());
        if resp.hovered() || resp.dragged() {
            ui.painter().rect_filled(Rect::from_center_size(pos2(bar.min.x + bar.width() * frac, bar.center().y), vec2(10.0, 10.0)), 3, theme::TEXT);
        }
        if resp.clicked() || resp.drag_stopped() {
            if let Some(pp) = resp.interact_pointer_pos() {
                let f = ((pp.x - bar.min.x) / bar.width()).clamp(0.0, 1.0);
                actions.push(Action::MediaSeek(p.bus.clone(), (f * p.length_us as f32) as i64));
            }
        }
    }
    ui.painter().text(pos2(bar.min.x, bar.max.y + 10.0), Align2::LEFT_CENTER, clock(pos), crate::theme::fid(10.5), theme::DIM);
    if p.length_us > 0 {
        ui.painter().text(pos2(bar.max.x, bar.max.y + 10.0), Align2::RIGHT_CENTER, clock(p.length_us), crate::theme::fid(10.5), theme::DIM);
    }
    // Previous, play or pause, next.
    let cy = r.min.y + 138.0;
    let playing = p.status == Status::Playing;
    let specs = [
        (Icon::Prev, p.can_prev, -50.0, false),
        (if playing { Icon::Pause } else { Icon::Play }, p.can_toggle, 0.0, true),
        (Icon::Next, p.can_next, 50.0, false),
    ];
    for (icon, enabled, dx, main) in specs {
        let b = Rect::from_center_size(pos2(r.center().x + dx, cy), vec2(if main { 46.0 } else { 38.0 }, 28.0));
        let resp = ui.interact(b, crate::ui::widgets::id(("mpris-btn", &p.bus, dx as i32)), if enabled { Sense::click() } else { Sense::hover() });
        let fill = if main { theme::accent() } else if resp.hovered() && enabled { theme::PLATE_SEL } else { theme::BTN };
        ui.painter().rect(b, 4, fill, egui::Stroke::new(1.0, if main { theme::accent() } else { theme::CARD_LINE }), egui::StrokeKind::Inside);
        let col = if !enabled { theme::LINE } else if main { egui::Color32::from_rgb(0x14, 0x16, 0x1a) } else { theme::TEXT };
        icons::draw(ui.painter(), icon, Rect::from_center_size(b.center(), vec2(14.0, 14.0)), col);
        if resp.clicked() {
            actions.push(match dx as i32 {
                -50 => Action::MediaPrev(p.bus.clone()),
                50 => Action::MediaNext(p.bus.clone()),
                _ => Action::MediaPlayPause(p.bus.clone()),
            });
        }
    }
    if playing {
        // Keep the bar moving between reads.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
}

fn duration_text(minutes: u32) -> String {
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

fn kv_row(ui: &mut Ui, key: &str, value: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    ui.painter().text(pos2(r.min.x + 14.0, r.center().y), Align2::LEFT_CENTER, key, crate::theme::fid(12.0), theme::DIM);
    ui.painter().text(pos2(r.max.x - 14.0, r.center().y), Align2::RIGHT_CENTER, value, crate::theme::fid(12.0), theme::TEXT);
}

fn profile_label(name: &str) -> String {
    match name {
        "power-saver" => "Power saver".into(),
        "balanced" => "Balanced".into(),
        "performance" => "Performance".into(),
        other => other.replace('-', " "),
    }
}

fn battery_panel(ui: &mut Ui, bat: Option<&crate::sys::BatteryDetail>, profiles: &crate::sys::PowerProfiles, actions: &mut Vec<Action>) {
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y), Align2::LEFT_CENTER, "Power", crate::theme::fid(13.0), theme::TEXT);
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));
    if let Some(b) = bat {
        // Big percentage and a status line with the time estimate.
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::hover());
        ui.painter().text(pos2(r.min.x + 14.0, r.min.y + 26.0), Align2::LEFT_CENTER, format!("{}%", b.percent), crate::theme::fid(26.0), theme::TEXT);
        let status = match (b.status.as_str(), b.eta_minutes) {
            ("Discharging", Some(m)) => format!("{} left", duration_text(m)),
            ("Charging", Some(m)) => format!("{} until full", duration_text(m)),
            ("Full", _) => "Fully charged".into(),
            ("Not charging", _) if b.ac_online => "Plugged in, not charging".into(),
            ("Discharging", None) => "Discharging".into(),
            ("Charging", None) => "Charging".into(),
            (s, _) => s.to_owned(),
        };
        ui.painter().text(pos2(r.max.x - 14.0, r.min.y + 26.0), Align2::RIGHT_CENTER, status, crate::theme::fid(12.0), theme::TEXT);
        let bar = Rect::from_min_max(pos2(r.min.x + 14.0, r.max.y - 18.0), pos2(r.max.x - 14.0, r.max.y - 12.0));
        ui.painter().rect_filled(bar, 3, theme::BTN);
        let col = if b.percent < 15 && b.status == "Discharging" { theme::accent() } else { theme::DIM };
        ui.painter().rect_filled(Rect::from_min_max(bar.min, pos2(bar.min.x + bar.width() * b.percent.min(100) as f32 / 100.0, bar.max.y)), 3, col);

        let source = if b.ac_online { "Mains" } else { "Battery" };
        kv_row(ui, "Power source", source);
        if b.power_w > 0.05 {
            let label = match b.status.as_str() {
                "Charging" => "Charging at",
                "Discharging" => "Drawing",
                _ => "Power flow",
            };
            kv_row(ui, label, &format!("{:.1} W", b.power_w));
        }
        if b.full_wh > 0.0 {
            kv_row(ui, "Energy", &format!("{:.1} of {:.1} Wh", b.energy_wh, b.full_wh));
        }
        if let Some(h) = b.health_percent() {
            kv_row(ui, "Battery health", &format!("{h}%  ({:.1} of {:.1} Wh new)", b.full_wh, b.design_wh));
        }
        if let Some(c) = b.cycles {
            kv_row(ui, "Charge cycles", &c.to_string());
        }
        if b.voltage_v > 0.0 {
            kv_row(ui, "Voltage", &format!("{:.2} V", b.voltage_v));
        }
        if let Some(l) = b.charge_limit {
            kv_row(ui, "Charge limit", &format!("{l}%"));
        }
        if !b.model.is_empty() {
            kv_row(ui, "Battery", &clip(&b.model, 30));
        }
        ui.add_space(6.0);
    } else {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::hover());
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "No battery found", crate::theme::fid(12.0), theme::DIM);
    }
    // Power profiles.
    let (sr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([sr.left_center(), sr.right_center()], egui::Stroke::new(1.0, theme::LINE));
    let (lr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    ui.painter().text(pos2(lr.min.x + 14.0, lr.center().y + 2.0), Align2::LEFT_CENTER, "Power profile", crate::theme::fid(13.0), theme::TEXT);
    if profiles.available.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "Power profiles are not available", crate::theme::fid(11.0), theme::DIM);
    }
    for name in &profiles.available {
        let active = *name == profiles.active;
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 30.0), Sense::click());
        let r = r.translate(vec2(6.0, 0.0));
        if resp.hovered() {
            ui.painter().rect_filled(r, 4, theme::PLATE);
        }
        if active {
            icons::draw(ui.painter(), Icon::Check, Rect::from_center_size(pos2(r.min.x + 16.0, r.center().y), vec2(12.0, 12.0)), theme::accent());
        }
        ui.painter().text(pos2(r.min.x + 34.0, r.center().y), Align2::LEFT_CENTER, profile_label(name), crate::theme::fid(12.0), theme::TEXT);
        if resp.clicked() && !active {
            actions.push(Action::SetPowerProfile(name.clone()));
        }
    }
    ui.add_space(6.0);
}

fn audio_panel(ui: &mut Ui, audio: &crate::sys::AudioInfo, actions: &mut Vec<Action>) {
    audio_section(ui, "Output", false, &audio.sinks, actions);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], egui::Stroke::new(1.0, theme::LINE));
    audio_section(ui, "Input", true, &audio.sources, actions);
}

fn audio_section(ui: &mut Ui, title: &str, input: bool, devs: &[crate::sys::AudioDev], actions: &mut Vec<Action>) {
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y + 2.0), Align2::LEFT_CENTER, title, crate::theme::fid(13.0), theme::TEXT);
    let Some(cur) = devs.iter().find(|d| d.default) else {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
        let what = if input { "No input devices" } else { "No output devices" };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, what, crate::theme::fid(12.0), theme::DIM);
        return;
    };
    // Mute button, slider, percent.
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    let mb = Rect::from_center_size(pos2(row.min.x + 24.0, row.center().y), vec2(24.0, 24.0));
    let mr = ui.interact(mb, crate::ui::widgets::id(("amute", input)), Sense::click());
    if mr.hovered() {
        ui.painter().rect_filled(mb, 4, theme::PLATE);
    }
    let ic = match (input, cur.muted) {
        (false, true) => Icon::VolumeMuted,
        _ => Icon::Volume,
    };
    let col = if cur.muted { theme::accent() } else { theme::DIM };
    icons::draw(ui.painter(), ic, mb.shrink(4.0), col);
    if mr.clicked() {
        actions.push(Action::AudioMute(input, cur.name.clone()));
    }
    let track = Rect::from_min_max(pos2(row.min.x + 44.0, row.center().y - 10.0), pos2(row.max.x - 52.0, row.center().y + 10.0));
    if let Some(p) = slider(ui, track, cur.percent, cur.muted, crate::ui::widgets::id(("avol", input))) {
        if p != cur.percent {
            actions.push(Action::AudioVolume(input, cur.name.clone(), p));
        }
    }
    ui.painter().text(
        pos2(row.max.x - 14.0, row.center().y),
        Align2::RIGHT_CENTER,
        format!("{}%", cur.percent),
        crate::theme::fid(11.0),
        theme::DIM,
    );
    ui.add_space(6.0);
    // Devices and their ports.
    for d in devs {
        let entries: Vec<(Option<&str>, String, Option<String>, bool)> = if d.ports.len() > 1 {
            d.ports
                .iter()
                .map(|p| (Some(p.name.as_str()), p.desc.clone(), Some(d.desc.clone()), d.default && d.active_port.as_deref() == Some(p.name.as_str())))
                .collect()
        } else {
            vec![(None, d.desc.clone(), None, d.default)]
        };
        for (port, label, sub, active) in entries {
            let h = if sub.is_some() { 36.0 } else { 30.0 };
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, h), Sense::click());
            let r = r.translate(vec2(6.0, 0.0));
            if resp.hovered() {
                ui.painter().rect_filled(r, 4, theme::PLATE);
            }
            if active {
                icons::draw(ui.painter(), Icon::Check, Rect::from_center_size(pos2(r.min.x + 16.0, r.center().y), vec2(12.0, 12.0)), theme::accent());
            }
            let (ly, sy) = if sub.is_some() { (r.min.y + 12.0, r.min.y + 26.0) } else { (r.center().y, 0.0) };
            ui.painter().text(pos2(r.min.x + 34.0, ly), Align2::LEFT_CENTER, clip(&label, 38), crate::theme::fid(12.0), theme::TEXT);
            if let Some(s) = &sub {
                ui.painter().text(pos2(r.min.x + 34.0, sy), Align2::LEFT_CENTER, clip(s, 44), crate::theme::fid(10.5), theme::DIM);
            }
            if resp.clicked() && !active {
                actions.push(Action::AudioSelect(input, d.name.clone(), port.map(str::to_owned)));
            }
        }
    }
    ui.add_space(6.0);
}

/// A horizontal slider drawn into `track`. Returns the new percent while the user drags or clicks.
fn slider(ui: &mut Ui, track: Rect, percent: u32, dim: bool, id: egui::Id) -> Option<u32> {
    let resp = ui.interact(track, id, Sense::click_and_drag());
    let bar = Rect::from_center_size(track.center(), vec2(track.width(), 4.0));
    let frac = (percent.min(100) as f32) / 100.0;
    let fill_col = if dim { theme::DIM } else { theme::accent() };
    let p = ui.painter();
    p.rect_filled(bar, 2, theme::BTN);
    p.rect_filled(Rect::from_min_max(bar.min, pos2(bar.min.x + bar.width() * frac, bar.max.y)), 2, fill_col);
    let hx = bar.min.x + bar.width() * frac;
    let big = resp.hovered() || resp.dragged();
    let k = if big { 14.0 } else { 12.0 };
    p.rect_filled(Rect::from_center_size(pos2(hx, track.center().y), vec2(k, k)), 4, theme::TEXT);
    if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let f = ((pos.x - bar.min.x) / bar.width()).clamp(0.0, 1.0);
            return Some((f * 100.0).round() as u32);
        }
    }
    None
}

fn screen_panel(ui: &mut Ui, current: u32, brightness: Option<u32>, actions: &mut Vec<Action>) {
    if let Some(b) = brightness {
        let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
        ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y + 2.0), Align2::LEFT_CENTER, "Brightness", crate::theme::fid(13.0), theme::TEXT);
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
        icons::draw(ui.painter(), Icon::Sun, Rect::from_center_size(pos2(row.min.x + 24.0, row.center().y), vec2(16.0, 16.0)), theme::DIM);
        let track = Rect::from_min_max(pos2(row.min.x + 44.0, row.center().y - 10.0), pos2(row.max.x - 52.0, row.center().y + 10.0));
        if let Some(p) = slider(ui, track, b, false, crate::ui::widgets::id("bright")) {
            if p != b {
                // The backlight never goes fully dark.
                actions.push(Action::SetBrightness(p.max(1)));
            }
        }
        ui.painter().text(pos2(row.max.x - 14.0, row.center().y), Align2::RIGHT_CENTER, format!("{b}%"), crate::theme::fid(11.0), theme::DIM);
        ui.add_space(6.0);
    }
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y), Align2::LEFT_CENTER, "Turn screen off after", crate::theme::fid(13.0), theme::TEXT);
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));
    ui.add_space(6.0);
    for m in crate::idle::CHOICES {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 30.0), Sense::click());
        let r = r.translate(vec2(6.0, 0.0));
        if resp.hovered() {
            ui.painter().rect_filled(r, 4, theme::PLATE);
        }
        if m == current {
            icons::draw(ui.painter(), Icon::Check, Rect::from_center_size(pos2(r.min.x + 16.0, r.center().y), vec2(12.0, 12.0)), theme::accent());
        }
        ui.painter().text(pos2(r.min.x + 34.0, r.center().y), Align2::LEFT_CENTER, crate::idle::label(m), crate::theme::fid(12.0), theme::TEXT);
        if resp.clicked() {
            actions.push(Action::SetScreenTimeout(m));
        }
    }
    ui.add_space(6.0);
}

fn network_panel(ui: &mut Ui, nets: &[crate::sys::WifiNet], scanning: bool, actions: &mut Vec<Action>) {
    let join_id = crate::ui::widgets::id("wifi-join");
    let pw_id = crate::ui::widgets::id("wifi-pw");
    let mut joining: Option<String> = ui.data(|d| d.get_temp(join_id));
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y), Align2::LEFT_CENTER, "Wi-Fi", crate::theme::fid(13.0), theme::TEXT);
    ui.painter().text(
        pos2(hr.max.x - 14.0, hr.center().y),
        Align2::RIGHT_CENTER,
        if scanning { "Scanning" } else { "" },
        crate::theme::fid(11.0),
        theme::DIM,
    );
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));
    if nets.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 60.0), Sense::hover());
        let msg = if scanning { "Looking for networks" } else { "No networks found" };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, msg, crate::theme::fid(12.0), theme::DIM);
    }
    for (title, saved_group) in [("Saved", true), ("Nearby", false)] {
        let group: Vec<_> = nets.iter().filter(|n| (n.saved || n.active) == saved_group).collect();
        if group.is_empty() {
            continue;
        }
        let (lr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
        ui.painter().text(pos2(lr.min.x + 14.0, lr.center().y + 4.0), Align2::LEFT_CENTER, title, crate::theme::fid(11.0), theme::DIM);
        for n in group.into_iter().take(10) {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 32.0), Sense::click());
            let r = r.translate(vec2(6.0, 0.0));
            let open = joining.as_deref() == Some(n.ssid.as_str());
            if (resp.hovered() || open) && !n.active {
                ui.painter().rect_filled(r, 4, theme::PLATE);
            }
            if n.active {
                icons::draw(ui.painter(), Icon::Check, Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(12.0, 12.0)), theme::accent());
            }
            ui.painter().text(pos2(r.min.x + 28.0, r.center().y), Align2::LEFT_CENTER, clip(&n.ssid, 20), crate::theme::fid(12.0), theme::TEXT);
            let mut x = r.max.x - 12.0;
            // Signal bars.
            for i in 0..3 {
                let h = 4.0 + i as f32 * 3.0;
                let on = n.signal >= [10, 40, 70][i];
                let bar = Rect::from_min_max(pos2(x - 3.0 - (2 - i) as f32 * 4.0, r.center().y + 5.0 - h), pos2(x - (2 - i) as f32 * 4.0, r.center().y + 5.0));
                ui.painter().rect_filled(bar, 0, if on { theme::DIM } else { theme::CARD_LINE });
            }
            x -= 18.0;
            if n.secure {
                icons::draw(ui.painter(), Icon::Lock, Rect::from_center_size(pos2(x - 6.0, r.center().y), vec2(12.0, 12.0)), theme::DIM);
            }
            if resp.clicked() && !n.active {
                if n.secure && !n.saved {
                    // A new secured network needs its password first.
                    joining = if open { None } else { Some(n.ssid.clone()) };
                    ui.data_mut(|d| d.insert_temp(pw_id, String::new()));
                } else {
                    actions.push(Action::NetConnect(n.ssid.clone(), n.saved));
                }
            }
            if open {
                let (pr, _) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 40.0), Sense::hover());
                let pr = pr.translate(vec2(6.0, 0.0));
                let mut pw: String = ui.data(|d| d.get_temp(pw_id)).unwrap_or_default();
                let field = Rect::from_min_max(pos2(pr.min.x + 8.0, pr.min.y + 4.0), pos2(pr.max.x - 74.0, pr.max.y - 4.0));
                ui.painter().rect(field, 4, theme::BTN, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field.shrink2(vec2(8.0, 2.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
                let te = child.add(
                    egui::TextEdit::singleline(&mut pw)
                        .id(crate::ui::widgets::id("wifi-pw-field"))
                        .password(true)
                        .hint_text("Password")
                        .frame(egui::Frame::NONE)
                        .desired_width(field.width() - 16.0)
                        .font(crate::theme::fid(12.0)),
                );
                if !ui.data(|d| d.get_temp::<bool>(crate::ui::widgets::id("wifi-pw-focused"))).unwrap_or(false) {
                    te.request_focus();
                    ui.data_mut(|d| d.insert_temp(crate::ui::widgets::id("wifi-pw-focused"), true));
                }
                let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let ready = pw.len() >= 8;
                let btn = Rect::from_min_max(pos2(pr.max.x - 66.0, pr.min.y + 6.0), pos2(pr.max.x - 8.0, pr.max.y - 6.0));
                let mut bc = ui.new_child(egui::UiBuilder::new().max_rect(btn).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)));
                let join = pill(&mut bc, "Join", ready);
                ui.data_mut(|d| d.insert_temp(pw_id, pw.clone()));
                if (join || enter) && ready {
                    actions.push(Action::NetJoin(n.ssid.clone(), pw));
                    joining = None;
                } else if (enter || join) && !ready {
                    te.request_focus();
                }
            }
        }
    }
    if joining.is_none() {
        ui.data_mut(|d| d.insert_temp(crate::ui::widgets::id("wifi-pw-focused"), false));
    }
    match &joining {
        Some(j) => ui.data_mut(|d| {
            d.insert_temp(join_id, j.clone());
        }),
        None => ui.data_mut(|d| d.remove::<String>(join_id)),
    }
    let (rr, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 30.0), Sense::click());
    let rr = rr.translate(vec2(6.0, 0.0));
    if resp.hovered() {
        ui.painter().rect_filled(rr, 4, theme::PLATE);
    }
    ui.painter().text(pos2(rr.min.x + 14.0, rr.center().y), Align2::LEFT_CENTER, "Scan again", crate::theme::fid(11.0), theme::DIM);
    if resp.clicked() {
        actions.push(Action::NetRescan);
    }
    ui.add_space(4.0);
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    use crate::tray::MenuNode;

    fn nodes(n: usize) -> Vec<MenuNode> {
        (0..n).map(|i| MenuNode { id: i as i32, label: format!("item {i}"), enabled: true, ..Default::default() }).collect()
    }
    fn place() -> Placement {
        Placement { anchor_x: 1800.0, bar_height: 30.0, bar_on_top: true }
    }

    #[test]
    fn short_menu_is_one_column() {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0));
        assert_eq!(tray_layout(full, &place(), None, &nodes(5)).0, 1);
    }

    #[test]
    fn long_menu_gets_more_columns_then_scrolls() {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 600.0));
        let (c, avail) = tray_layout(full, &place(), None, &nodes(30));
        assert!(c >= 2, "30 rows do not fit one column of {avail}");
        // So many rows that even the widest layout cannot hold them: scroll.
        let (c, avail) = tray_layout(Rect::from_min_size(pos2(0.0, 0.0), vec2(520.0, 600.0)), &place(), None, &nodes(200));
        assert_eq!(c, 2);
        let mut flat = Vec::new();
        let n = nodes(200);
        flatten(&n, 0, &mut flat);
        let tallest = split_columns(&flat, c).iter().map(|g| g.iter().map(FlatRow::height).sum::<f32>()).fold(0.0, f32::max);
        assert!(tallest > avail);
    }
}

fn mem_text(kb: u64) -> String {
    if kb >= 1024 * 1024 {
        format!("{:.1} GB", kb as f64 / 1024.0 / 1024.0)
    } else {
        format!("{} MB", kb / 1024)
    }
}

/// The programs using the most CPU or memory, with programs that run as many
/// processes added together. Lock holds the order still. The x stops a program.
fn proc_panel(ui: &mut Ui, list: &crate::sys::ProcList, cpu: bool, actions: &mut Vec<Action>) {
    use crate::sys::Proc;
    let confirm_id = crate::ui::widgets::id("proc-kill");
    let mut confirm: Option<String> = ui.data(|d| d.get_temp(confirm_id)).flatten();
    let edit_id = crate::ui::widgets::id("proc-limit");
    let mut edit: Option<(String, String)> = ui.data(|d| d.get_temp(edit_id)).flatten();
    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    let title = if cpu { "Processor" } else { "Memory" };
    let tw = ui.painter().layout_no_wrap(title.to_owned(), crate::theme::fid(13.0), theme::TEXT).size().x;
    ui.painter().text(pos2(hr.min.x + 14.0, hr.center().y), Align2::LEFT_CENTER, title, crate::theme::fid(13.0), theme::TEXT);
    // Lock chip.
    let locked = list.lock.is_some();
    let lr = Rect::from_min_size(pos2(hr.min.x + 14.0 + tw + 10.0, hr.center().y - 11.0), vec2(if locked { 66.0 } else { 78.0 }, 22.0));
    let lresp = ui.interact(lr, crate::ui::widgets::id("proc-lock"), Sense::click());
    let (fill, stroke, col) = if locked {
        (theme::accent_bg(), theme::accent(), theme::accent())
    } else if lresp.hovered() {
        (theme::PLATE_SEL, theme::CARD_LINE, theme::TEXT)
    } else {
        (theme::BTN, theme::CARD_LINE, theme::DIM)
    };
    ui.painter().rect(lr, 4, fill, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    ui.painter().text(lr.center(), Align2::CENTER_CENTER, if locked { "Locked" } else { "Lock order" }, crate::theme::fid(11.0), col);

    let total = if cpu {
        let used: f32 = list.all.iter().map(|p| p.cpu).sum();
        format!("{:.0}% of {} cores", (used / list.cores.max(1) as f32).min(100.0), list.cores)
    } else {
        format!("{} of {}", mem_text(list.mem_used_kb), mem_text(list.mem_total_kb))
    };
    let right = if cpu && !list.ready { "Measuring\u{2026}".to_owned() } else { total };
    ui.painter().text(pos2(hr.max.x - 14.0, hr.center().y), Align2::RIGHT_CENTER, right, crate::theme::fid(11.0), theme::DIM);
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));

    let key = |p: &Proc| if cpu { p.cpu } else { p.mem_kb as f32 };
    let ranked = proc_ranked(list, cpu);
    let rows = proc_rows(list, cpu);
    if lresp.clicked() {
        actions.push(Action::ProcLock(if locked { None } else { Some(ranked.iter().map(|p| p.name.clone()).collect()) }));
    }
    if rows.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 70.0), Sense::hover());
        let msg = if cpu && !list.ready { "Measuring\u{2026}" } else { "Nothing is using much" };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, msg, crate::theme::fid(12.0), theme::DIM);
        return;
    }
    ui.add_space(4.0);
    let max = rows.iter().map(|p| key(p)).fold(1.0, f32::max);
    let open_id = crate::ui::widgets::id("proc-open");
    let mut open: Option<String> = ui.data(|d| d.get_temp(open_id)).flatten();
    // Sized from the rows, not from last frame, so it opens at its real height.
    let sh = proc_scroll_h(&rows, open.as_deref(), panel_avail(ui) - PROC_CHROME);
    egui::ScrollArea::vertical().min_scrolled_height(sh).max_height(sh).auto_shrink([false, false]).show(ui, |ui| {
    for p in rows.iter().map(|r| r.as_ref()) {
        let label = proc_label(p);
        let cur = list.limits.get(&p.name).and_then(|l| if cpu { l.cpu.map(f64::from) } else { l.mem_mb.map(|m| m as f64) });
        let value = proc_value(list, p, cpu);
        let is_open = p.count > 1 && open.as_deref() == Some(p.name.as_str());
        let pids: Vec<u32> = p.members.iter().map(|m| m.0).collect();
        let lim = LimitCtx { cpu, cur, all: list.limits.get(&p.name).copied().unwrap_or_default(), name: &p.name };
        let clicked = proc_row(ui, &p.name, &label, &value, key(p) / max, p.count > 1, is_open, 0.0, &pids, &mut confirm, Some((&lim, &mut edit)), actions);
        if clicked {
            open = if is_open { None } else { Some(p.name.clone()) };
        }
        if is_open {
            let mut mem: Vec<&(u32, f32, u64)> = p.members.iter().collect();
            let mk = |m: &(u32, f32, u64)| if cpu { m.1 } else { m.2 as f32 };
            if list.lock.is_some() {
                mem.sort_by_key(|m| m.0);
            } else {
                mem.sort_by(|a, b| mk(b).total_cmp(&mk(a)).then(a.0.cmp(&b.0)));
            }
            let top = mem.iter().map(|m| mk(m)).fold(1.0, f32::max);
            for m in mem.into_iter().take(30) {
                let cmd = crate::sys::cmdline(m.0);
                let label = member_label(m.0, &cmd, &p.name);
                let value = if cpu { cpu_text((m.1 / list.cores.max(1) as f32).min(100.0)) } else { mem_text(m.2) };
                proc_row(ui, &format!("#{}", m.0), &label, &value, mk(m) / top, false, false, 18.0, &[m.0], &mut confirm, None, actions);
            }
        }
    }
    });
    if open.as_deref().is_some_and(|o| !list.all.iter().any(|p| p.name == o)) {
        open = None;
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
    ui.add_space(6.0);
    ui.data_mut(|d| d.insert_temp(confirm_id, confirm));
    ui.data_mut(|d| d.insert_temp(edit_id, edit));
}

/// Header, spacing and card margins around the process list.
const PROC_CHROME: f32 = 38.0 + 4.0 + 6.0 + 24.0;

/// Height the panel may use, set by `overlay` each frame.
fn panel_avail(ui: &Ui) -> f32 {
    ui.data(|d| d.get_temp(crate::ui::widgets::id("panel-avail"))).unwrap_or(520.0)
}

/// What the + and - buttons step through. Any value can also be typed.
/// The widest the process card gets, when a name is very long.
const PROC_MAX_W: f32 = 760.0;

fn proc_label(p: &crate::sys::Proc) -> String {
    if p.count > 1 {
        format!("{} \u{00d7}{}", p.name, p.count)
    } else if p.members.is_empty() {
        format!("{} (not running)", p.name)
    } else {
        p.name.clone()
    }
}

fn member_label(pid: u32, cmd: &str, name: &str) -> String {
    format!("{pid}  {}", if cmd.is_empty() { name } else { cmd })
}

/// The figure on a program row, with its limit when it has one.
fn proc_value(list: &crate::sys::ProcList, p: &crate::sys::Proc, cpu: bool) -> String {
    let value = if cpu { cpu_text((p.cpu / list.cores.max(1) as f32).min(100.0)) } else { mem_text(p.mem_kb) };
    let cur = list.limits.get(&p.name).and_then(|l| if cpu { l.cpu.map(f64::from) } else { l.mem_mb.map(|m| m as f64) });
    match cur {
        Some(c) => format!("{value} / {}", limit_text(cpu, c)),
        None => value,
    }
}

/// Width that shows every name in full, between the usual 320 and `max`.
fn proc_width(ui: &Ui, list: &crate::sys::ProcList, cpu: bool, open: Option<&str>, max: f32) -> f32 {
    let text_w = |t: &str, size: f32| ui.painter().layout_no_wrap(t.to_owned(), crate::theme::fid(size), theme::TEXT).size().x;
    let mut need = 320.0f32;
    for p in proc_rows(list, cpu).iter().map(|r| r.as_ref()) {
        // Pad, triangle, label, gap, value, then the limit and stop buttons.
        need = need.max(14.0 + 14.0 + text_w(&proc_label(p), 12.0) + 16.0 + text_w(&proc_value(list, p, cpu), 12.0) + 66.0);
        if p.count > 1 && open == Some(p.name.as_str()) {
            for m in p.members.iter().take(30) {
                let v = if cpu { cpu_text((m.1 / list.cores.max(1) as f32).min(100.0)) } else { mem_text(m.2) };
                need = need.max(18.0 + 14.0 + text_w(&member_label(m.0, &crate::sys::cmdline(m.0), &p.name), 11.0) + 16.0 + text_w(&v, 12.0) + 40.0);
            }
        }
    }
    need.min(max).ceil()
}

/// Cut text to fit a width in points, ending in an ellipsis. Only used when the
/// card is at its widest and a name is still longer.
fn clip_px(ui: &Ui, text: &str, size: f32, max_w: f32) -> String {
    let w = |t: &str| ui.painter().layout_no_wrap(t.to_owned(), crate::theme::fid(size), theme::TEXT).size().x;
    if w(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let t: String = chars[..mid].iter().collect::<String>() + "\u{2026}";
        if w(&t) <= max_w { lo = mid } else { hi = mid - 1 }
    }
    chars[..lo].iter().collect::<String>() + "\u{2026}"
}

const CPU_STEPS: [f64; 12] = [0.5, 1.0, 2.0, 3.0, 5.0, 10.0, 15.0, 25.0, 40.0, 50.0, 75.0, 100.0];
const MEM_STEPS: [f64; 11] = [128.0, 256.0, 512.0, 1024.0, 2048.0, 3072.0, 4096.0, 6144.0, 8192.0, 12288.0, 16384.0];

fn fmt_num(v: f64) -> String {
    if v.fract().abs() < 1e-9 { format!("{}", v as u64) } else { format!("{v:.1}") }
}

/// A typed limit, or None when it is not a usable number.
/// CPU is 0.1 to 100 (percent of the processor); memory is 16 MB or more.
fn parse_limit(text: &str, cpu: bool) -> Option<f64> {
    let v: f64 = text.trim().trim_end_matches('%').trim().parse().ok()?;
    let ok = if cpu { (0.1..=100.0).contains(&v) } else { v >= 16.0 };
    ok.then_some(v)
}

/// Share of the whole processor.
fn cpu_text(v: f32) -> String {
    if v < 10.0 { format!("{v:.1}%") } else { format!("{v:.0}%") }
}

fn limit_text(cpu: bool, v: f64) -> String {
    if cpu {
        format!("{}%", fmt_num(v))
    } else if v >= 1024.0 {
        format!("{:.1} GB", v / 1024.0)
    } else {
        format!("{} MB", fmt_num(v))
    }
}

/// What the limit editor of one row needs to know.
struct LimitCtx<'a> {
    cpu: bool,
    /// The current limit for this measure.
    cur: Option<f64>,
    /// Both limits, so changing one keeps the other.
    all: crate::limits::Limit,
    name: &'a str,
}

/// One row of the process list. Returns true when the row (not its x) was clicked.
#[allow(clippy::too_many_arguments)]
fn proc_row(
    ui: &mut Ui,
    key: &str,
    label: &str,
    value: &str,
    frac: f32,
    expandable: bool,
    is_open: bool,
    indent: f32,
    pids: &[u32],
    confirm: &mut Option<String>,
    limit: Option<(&LimitCtx, &mut Option<(String, String)>)>,
    actions: &mut Vec<Action>,
) -> bool {
    let h = if indent > 0.0 { 30.0 } else { 38.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), if expandable { Sense::click() } else { Sense::hover() });
    let ty = r.min.y + if indent > 0.0 { 11.0 } else { 13.0 };
    let mut limit = limit;
    if let Some((ctx, edit)) = limit.as_mut() {
        if let Some((_, text)) = edit.as_mut().filter(|(k, _)| k == key) {
            ui.painter().rect_filled(r.shrink2(vec2(6.0, 1.0)), 4, theme::PLATE);
            ui.painter().text(pos2(r.min.x + 14.0, r.center().y), Align2::LEFT_CENTER, "Limit", crate::theme::fid(12.0), theme::TEXT);
            let btn = |ui: &mut Ui, rect: Rect, text: &str, id: &str, hot: bool| -> bool {
                let rs = ui.interact(rect, crate::ui::widgets::id((id, key)), Sense::click());
                let (fill, stroke, col) = if hot { (theme::accent_bg(), theme::accent(), theme::accent()) } else if rs.hovered() { (theme::PLATE_SEL, theme::CARD_LINE, theme::TEXT) } else { (theme::BTN, theme::CARD_LINE, theme::TEXT) };
                ui.painter().rect(rect, 4, fill, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, crate::theme::fid(11.0), col);
                rs.clicked()
            };
            let cy = r.center().y;
            let steps = if ctx.cpu { &CPU_STEPS[..] } else { &MEM_STEPS[..] };
            let minus = Rect::from_center_size(pos2(r.min.x + 66.0, cy), vec2(20.0, 24.0));
            let field = Rect::from_min_size(pos2(r.min.x + 80.0, cy - 12.0), vec2(60.0, 24.0));
            let plus = Rect::from_center_size(pos2(field.max.x + 14.0, cy), vec2(20.0, 24.0));
            let parsed = parse_limit(text, ctx.cpu);
            if btn(ui, minus, "\u{2212}", "lim-minus", false) {
                let v = parsed.unwrap_or(steps[0]);
                *text = fmt_num(steps.iter().rev().find(|s| **s < v - 1e-9).copied().unwrap_or(steps[0]));
            }
            if btn(ui, plus, "+", "lim-plus", false) {
                let v = parsed.unwrap_or(steps[0]);
                *text = fmt_num(steps.iter().find(|s| **s > v + 1e-9).copied().unwrap_or(*steps.last().unwrap()));
            }
            ui.painter().rect(field, 4, theme::BTN, egui::Stroke::new(1.0, if parsed.is_some() { theme::CARD_LINE } else { theme::accent() }), egui::StrokeKind::Inside);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field.shrink2(vec2(6.0, 1.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let te = child.add(
                egui::TextEdit::singleline(text)
                    .id(crate::ui::widgets::id(("lim-field", key)))
                    .frame(egui::Frame::NONE)
                    .align(egui::Align2::CENTER_CENTER)
                    .desired_width(field.width() - 12.0)
                    .font(crate::theme::fid(12.0)),
            );
            let fid = crate::ui::widgets::id(("lim-focused", key));
            if !ui.data(|d| d.get_temp::<bool>(fid)).unwrap_or(false) {
                te.request_focus();
                ui.data_mut(|d| d.insert_temp(fid, true));
            }
            let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.painter().text(pos2(plus.max.x + 6.0, cy), Align2::LEFT_CENTER, if ctx.cpu { "%" } else { "MB" }, crate::theme::fid(11.0), theme::DIM);
            let set = Rect::from_center_size(pos2(r.max.x - 82.0, cy), vec2(50.0, 24.0));
            let other = Rect::from_center_size(pos2(r.max.x - 32.0, cy), vec2(52.0, 24.0));
            let mut done = false;
            let mut new = ctx.all;
            let clicked_set = btn(ui, set, "Set", "lim-set", parsed.is_some());
            if (clicked_set || enter) && parsed.is_some() {
                let v = parsed.unwrap();
                if ctx.cpu { new.cpu = Some(v as f32) } else { new.mem_mb = Some(v as u64) }
                actions.push(Action::SetProcLimit(ctx.name.to_owned(), new));
                done = true;
            } else if enter {
                te.request_focus();
            }
            if btn(ui, other, if ctx.cur.is_some() { "Clear" } else { "Cancel" }, "lim-other", false) {
                if ctx.cur.is_some() {
                    if ctx.cpu { new.cpu = None } else { new.mem_mb = None }
                    actions.push(Action::SetProcLimit(ctx.name.to_owned(), new));
                }
                done = true;
            }
            if done {
                ui.data_mut(|d| d.insert_temp(fid, false));
                **edit = None;
            }
            return false;
        }
    }
    if confirm.as_deref() == Some(key) {
        ui.painter().rect_filled(r.shrink2(vec2(6.0, 1.0)), 4, theme::PLATE);
        let what = if pids.len() > 1 { format!("Stop all {} processes?", pids.len()) } else { "Stop this process?".to_owned() };
        ui.painter().text(pos2(r.min.x + 14.0 + indent, r.center().y), Align2::LEFT_CENTER, what, crate::theme::fid(12.0), theme::TEXT);
        let yes = Rect::from_center_size(pos2(r.max.x - 82.0, r.center().y), vec2(50.0, 24.0));
        let no = Rect::from_center_size(pos2(r.max.x - 32.0, r.center().y), vec2(52.0, 24.0));
        for (rect, text, id, hot) in [(yes, "Stop", "proc-yes", true), (no, "Cancel", "proc-no", false)] {
            let rs = ui.interact(rect, crate::ui::widgets::id((id, key)), Sense::click());
            let (fill, stroke, col) = if hot { (theme::accent_bg(), theme::accent(), theme::accent()) } else if rs.hovered() { (theme::PLATE_SEL, theme::CARD_LINE, theme::TEXT) } else { (theme::BTN, theme::CARD_LINE, theme::TEXT) };
            ui.painter().rect(rect, 4, fill, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, crate::theme::fid(11.0), col);
            if rs.clicked() {
                if hot {
                    actions.push(Action::KillProcs(pids.to_vec()));
                }
                *confirm = None;
            }
        }
        return false;
    }
    let hover = ui.rect_contains_pointer(r);
    if hover && expandable {
        ui.painter().rect_filled(r.shrink2(vec2(6.0, 1.0)), 4, theme::PLATE);
    }
    let mut tx = r.min.x + 14.0 + indent;
    if expandable {
        // A small triangle: right when closed, down when open.
        let c = pos2(tx + 3.0, ty);
        let pts = if is_open {
            vec![c + vec2(-4.0, -2.5), c + vec2(4.0, -2.5), c + vec2(0.0, 3.0)]
        } else {
            vec![c + vec2(-2.5, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.5, 4.0)]
        };
        ui.painter().add(egui::Shape::convex_polygon(pts, theme::DIM, egui::Stroke::NONE));
        tx += 14.0;
    }
    let vx = if limit.is_some() { 66.0 } else { 40.0 };
    let vw = ui.painter().layout_no_wrap(value.to_owned(), crate::theme::fid(12.0), theme::DIM).size().x;
    let size = if indent > 0.0 { 11.0 } else { 12.0 };
    let label = clip_px(ui, label, size, r.max.x - vx - vw - 12.0 - tx);
    ui.painter().text(pos2(tx, ty), Align2::LEFT_CENTER, label, crate::theme::fid(size), if indent > 0.0 { theme::DIM } else { theme::TEXT });
    ui.painter().text(pos2(r.max.x - vx, ty), Align2::RIGHT_CENTER, value, crate::theme::fid(12.0), if limit.as_ref().is_some_and(|(c, _)| c.cur.is_some()) { theme::accent() } else { theme::DIM });
    if let Some((ctx, edit)) = limit.as_mut() {
        let g = Rect::from_center_size(pos2(r.max.x - 46.0, ty), vec2(20.0, 20.0));
        let gr = ui.interact(g, crate::ui::widgets::id(("proc-lim", key)), Sense::click());
        if hover || gr.hovered() || ctx.cur.is_some() {
            if gr.hovered() {
                ui.painter().rect_filled(g, 4, theme::PLATE_SEL);
            }
            let col = if ctx.cur.is_some() || gr.hovered() { theme::accent() } else { theme::DIM };
            crate::icons::draw(ui.painter(), crate::icons::Icon::Gauge, g.shrink(4.0), col);
        }
        if gr.clicked() {
            let want = ctx.cur.unwrap_or(if ctx.cpu { 25.0 } else { 1024.0 });
            **edit = Some((key.to_owned(), fmt_num(want)));
        }
    }
    let bar = Rect::from_min_max(pos2(r.min.x + 14.0 + indent, r.max.y - 11.0), pos2(r.max.x - 14.0, r.max.y - 7.0));
    ui.painter().rect_filled(bar, 2, theme::BTN);
    ui.painter().rect_filled(Rect::from_min_max(bar.min, pos2(bar.min.x + bar.width() * frac.clamp(0.02, 1.0), bar.max.y)), 2, theme::accent());
    // A program that is not running has nothing to stop, so no x.
    let x = Rect::from_center_size(pos2(r.max.x - 22.0, ty), vec2(20.0, 20.0));
    let xr = ui.interact(x, crate::ui::widgets::id(("proc-x", key)), if pids.is_empty() { Sense::hover() } else { Sense::click() });
    if !pids.is_empty() && (hover || xr.hovered()) {
        ui.painter().rect_filled(x, 4, if xr.hovered() { theme::PLATE_SEL } else { theme::BTN });
        crate::icons::draw(ui.painter(), crate::icons::Icon::Close, x.shrink(5.0), if xr.hovered() { theme::accent() } else { theme::DIM });
    }
    if xr.clicked() {
        *confirm = Some(key.to_owned());
    }
    resp.clicked() && !xr.hovered()
}

/// Every program by the current measure, with limited programs first. A limited
/// program that is not running still gets a row, so its limit can be cleared.
fn proc_ranked<'a>(list: &'a crate::sys::ProcList, cpu: bool) -> Vec<std::borrow::Cow<'a, crate::sys::Proc>> {
    use std::borrow::Cow;
    let key = |p: &crate::sys::Proc| if cpu { p.cpu } else { p.mem_kb as f32 };
    let limited = |p: &crate::sys::Proc| list.limits.contains_key(&p.name);
    let mut ranked: Vec<Cow<crate::sys::Proc>> = list.all.iter().filter(|p| limited(p) || if cpu { p.cpu >= 0.05 } else { true }).map(Cow::Borrowed).collect();
    for n in list.limits.keys() {
        if !list.all.iter().any(|p| &p.name == n) {
            ranked.push(Cow::Owned(crate::sys::Proc { name: n.clone(), ..Default::default() }));
        }
    }
    ranked.sort_by(|a, b| limited(b).cmp(&limited(a)).then(key(b).total_cmp(&key(a))).then(a.name.cmp(&b.name)));
    ranked
}

/// The rows to show: the ranking, or the locked order.
fn proc_rows<'a>(list: &'a crate::sys::ProcList, cpu: bool) -> Vec<std::borrow::Cow<'a, crate::sys::Proc>> {
    match &list.lock {
        Some(names) => {
            let ranked = proc_ranked(list, cpu);
            names.iter().filter_map(|n| ranked.iter().find(|p| &p.name == n).cloned()).collect()
        }
        None => proc_ranked(list, cpu),
    }
}

/// Height of the list area: every row, counting the open group's members.
fn proc_scroll_h(rows: &[std::borrow::Cow<crate::sys::Proc>], open: Option<&str>, avail: f32) -> f32 {
    let h: f32 = rows
        .iter()
        .map(|p| 38.0 + if p.count > 1 && open == Some(p.name.as_str()) { p.members.len().min(30) as f32 * 30.0 } else { 0.0 })
        .sum();
    h.min(avail.max(120.0))
}

#[cfg(test)]
mod limit_input_tests {
    use super::*;

    #[test]
    fn cpu_input_is_a_share_of_the_processor() {
        assert_eq!(parse_limit("12.5", true), Some(12.5));
        assert_eq!(parse_limit(" 5% ", true), Some(5.0));
        assert_eq!(parse_limit("100", true), Some(100.0));
        assert_eq!(parse_limit("0", true), None);
        assert_eq!(parse_limit("101", true), None);
        assert_eq!(parse_limit("abc", true), None);
        assert_eq!(parse_limit("", true), None);
    }

    #[test]
    fn memory_input_is_megabytes_of_at_least_16() {
        assert_eq!(parse_limit("512", false), Some(512.0));
        assert_eq!(parse_limit("16", false), Some(16.0));
        assert_eq!(parse_limit("15", false), None);
        assert_eq!(parse_limit("-1", false), None);
    }

    #[test]
    fn numbers_and_limits_read_well() {
        assert_eq!(fmt_num(25.0), "25");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(limit_text(true, 2.0), "2%");
        assert_eq!(limit_text(false, 512.0), "512 MB");
        assert_eq!(limit_text(false, 2048.0), "2.0 GB");
        assert_eq!(cpu_text(2.44), "2.4%");
        assert_eq!(cpu_text(35.4), "35%");
    }
}
