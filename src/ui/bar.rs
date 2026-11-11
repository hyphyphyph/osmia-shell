//! The bar. Items come from the config layout. The look follows the picked
//! mockup (Bar.mockups round 1, variation 4).

use egui::{pos2, vec2, Align, Layout, Rect, Sense, Ui};

use crate::config::Layout as BarLayout;
use crate::icons::Icon;
use crate::sys::{Net, Stats};
use crate::theme;
use crate::ui::popups::clip;
use crate::ui::widgets::{divider, item, tile};
use crate::ui::{Action, Overlay};

pub struct Entry {
    pub index: usize,
    pub title: String,
    pub app_id: String,
    pub alert: bool,
}

pub struct Input<'a> {
    pub stats: &'a Stats,
    pub clock_date: String,
    pub clock_time: String,
    pub dnd: bool,
    pub unread: usize,
    pub notif_blocked: bool,
    pub attention: &'a [Entry],
    /// Set while recording: the time so far, a countdown, or "Saving".
    pub recording: Option<String>,
    /// The current player: what is playing, whether it is playing, and its bus name.
    pub media: Option<(String, bool, String)>,
    pub layout: &'a BarLayout,
    pub tray: &'a [crate::tray::Item],
    pub gpu: &'a crate::gpu::State,
    pub bt: &'a crate::bt::State,
    pub top: bool,
}

pub fn show(ui: &mut Ui, inp: &Input, actions: &mut Vec<Action>, rects: &mut Vec<(String, Rect)>) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BAR))
        .show(ui, |ui| {
            let full = ui.max_rect();
            // A 1 px line on the edge that faces the screen is drawn by the caller's frame.
            let (la, lb) = if inp.top { (full.left_bottom(), full.right_bottom()) } else { (full.left_top(), full.right_top()) };
            ui.painter().line_segment([la, lb], egui::Stroke::new(1.0, theme::LINE));
            // Left.
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(full).layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.add_space(4.0);
                    for id in &inp.layout.left {
                        render(ui, id, inp, actions, rects, false);
                    }
                },
            );
            // Center: measure, then draw centered.
            if !inp.layout.center.is_empty() {
                let mut m = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(full)
                        .layout(Layout::left_to_right(Align::Center))
                        .sizing_pass()
                        .invisible(),
                );
                let mut dummy = Vec::new();
                let mut dummy_rects = Vec::new();
                for id in &inp.layout.center {
                    render(&mut m, id, inp, &mut dummy, &mut dummy_rects, true);
                }
                let w = m.min_rect().width();
                let r = Rect::from_min_size(pos2(full.center().x - w / 2.0, full.min.y), vec2(w + 2.0, full.height()));
                ui.scope_builder(egui::UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)), |ui| {
                    for id in &inp.layout.center {
                        render(ui, id, inp, actions, rects, false);
                    }
                });
            }
            // Right, built from the far edge inward.
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(full).layout(Layout::right_to_left(Align::Center)),
                |ui| {
                    for id in inp.layout.right.iter().rev() {
                        render(ui, id, inp, actions, rects, false);
                    }
                    if let Some(text) = &inp.recording {
                        recording_item(ui, text, actions);
                    }
                },
            );
        });
}

/// A red square and the time. Click it to stop.
fn recording_item(ui: &mut Ui, text: &str, actions: &mut Vec<Action>) {
    let red = egui::Color32::from_rgb(0xe5, 0x48, 0x4d);
    let g = ui.painter().layout_no_wrap(text.to_owned(), crate::theme::fid(12.0), theme::TEXT);
    let saving = text == "Saving";
    let w = 10.0 + 8.0 + g.size().x + 16.0;
    let h = ui.available_height().max(22.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if saving { Sense::hover() } else { Sense::click() });
    if resp.hovered() && !saving {
        ui.painter().rect_filled(egui::Rect::from_center_size(rect.center(), vec2(rect.width() - 2.0, 22.0)), 4, theme::PLATE);
    }
    let cy = rect.center().y;
    ui.painter().rect_filled(egui::Rect::from_center_size(pos2(rect.min.x + 13.0, cy), vec2(10.0, 10.0)), 2, red);
    ui.painter().galley(pos2(rect.min.x + 26.0, cy - g.size().y / 2.0), g, theme::TEXT);
    if !saving {
        let resp = resp.on_hover_text("Stop recording");
        if resp.clicked() {
            actions.push(Action::StopRecording);
        }
    }
}

fn record(rects: &mut Vec<(String, Rect)>, id: &str, r: Rect, measuring: bool) {
    if !measuring {
        rects.push((id.to_owned(), r));
    }
}

fn render(ui: &mut Ui, id: &str, inp: &Input, actions: &mut Vec<Action>, rects: &mut Vec<(String, Rect)>, measuring: bool) {
    let st = inp.stats;
    match id {
        "|" => divider(ui),
        "windows" => {
            let r = item(ui, Some((Icon::Windows, theme::DIM)), &[], Sense::click());
            record(rects, "windows", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Windows, r.rect.center().x));
            }
        }
        "attention" => attention(ui, inp, actions),
        "tray" => crate::tray::bar_items(ui, inp.tray, actions),
        "cpu" => {
            let r = item(ui, None, &[("CPU", theme::DIM), (&format!("{}%", st.cpu_percent), theme::TEXT)], Sense::click());
            record(rects, "cpu", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Cpu, r.rect.center().x));
            }
        }
        "ram" => {
            let r = item(ui, None, &[("RAM", theme::DIM), (&format!("{}%", st.ram_percent), theme::TEXT)], Sense::click());
            record(rects, "ram", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Ram, r.rect.center().x));
            }
        }
        "gpu" => {
            // Hidden on a machine without the service, as other missing sources are.
            let Some(s) = &inp.gpu.status else { return };
            use crate::gpu::Phase;
            let text = match s.state {
                _ if inp.gpu.pending.is_some() => "\u{2026}".to_owned(),
                Phase::Host => "Host".to_owned(),
                Phase::Vfio => "VFIO".to_owned(),
                Phase::VmRunning => clip(if s.vm_name.is_empty() { "VM" } else { &s.vm_name }, 12),
                Phase::Switching => "\u{2026}".to_owned(),
                Phase::Error => "Error".to_owned(),
                Phase::Unknown => "?".to_owned(),
            };
            let alert = s.preference_pending || inp.gpu.error.is_some() || s.state == Phase::Error;
            let col = if alert { theme::accent() } else { theme::TEXT };
            let r = item(ui, None, &[("GPU", theme::DIM), (&text, col)], Sense::click());
            record(rects, "gpu", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Gpu, r.rect.center().x));
            }
        }
        "battery" => {
            if let Some(b) = &st.battery {
                let charging = b.charging && b.percent < 100;
                let ic = if charging { Icon::BatteryCharging } else { Icon::Battery };
                let col = if b.percent < 15 && !b.charging { theme::accent() } else { theme::DIM };
                let r = item(ui, Some((ic, col)), &[(&format!("{}%", b.percent), theme::TEXT)], Sense::click());
                record(rects, "battery", r.rect, measuring);
                if r.clicked() {
                    actions.push(Action::ToggleOverlay(Overlay::Battery, r.rect.center().x));
                }
            }
        }
        "network" => {
            let (ic, text) = match &st.network {
                Some(Net::Wifi { ssid, .. }) => (Icon::Wifi, clip(ssid, 14)),
                Some(Net::Wired) => (Icon::Wifi, "Wired".to_owned()),
                Some(Net::Offline) => (Icon::Wifi, "Offline".to_owned()),
                None => return, // not read yet
            };
            let col = if matches!(st.network, Some(Net::Wifi { .. }) | Some(Net::Wired)) { theme::DIM } else { theme::accent() };
            let r = item(ui, Some((ic, col)), &[(&text, theme::TEXT)], Sense::click());
            record(rects, "network", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Network, r.rect.center().x));
            }
        }
        "bluetooth" => {
            // Hidden with no adapter, as other missing sources are.
            let b = inp.bt;
            let Some(a) = &b.adapter else { return };
            let connected = b.connected();
            let text = match connected.as_slice() {
                _ if !a.powered => "Off".to_owned(),
                [] => String::new(),
                [one] => clip(&one.alias, 14),
                many => format!("{}", many.len()),
            };
            let alert = !b.asks.is_empty() || b.error.is_some();
            let ic = if alert { theme::accent() } else { theme::DIM };
            let col = if a.powered { theme::TEXT } else { theme::DIM };
            let parts: Vec<(&str, egui::Color32)> = if text.is_empty() { vec![] } else { vec![(&text, col)] };
            let r = item(ui, Some((Icon::Bluetooth, ic)), &parts, Sense::click());
            record(rects, "bluetooth", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Bluetooth, r.rect.center().x));
            }
        }
        "volume" => {
            if let Some(v) = &st.volume {
                let ic = if v.muted { Icon::VolumeMuted } else { Icon::Volume };
                let r = item(ui, Some((ic, theme::DIM)), &[(&format!("{}%", v.percent), theme::TEXT)], Sense::click());
                record(rects, "volume", r.rect, measuring);
                if r.clicked() {
                    actions.push(Action::ToggleOverlay(Overlay::Audio, r.rect.center().x));
                }
                if r.secondary_clicked() {
                    actions.push(Action::ToggleMute);
                }
                if r.hovered() {
                    let dy = ui.input(|i| i.smooth_scroll_delta.y);
                    if dy.abs() > 0.5 {
                        actions.push(Action::VolumeScroll(if dy > 0.0 { 5 } else { -5 }));
                    }
                }
            }
        }
        "brightness" => {
            if let Some(b) = st.brightness_percent {
                let r = item(ui, Some((Icon::Sun, theme::DIM)), &[(&format!("{b}%"), theme::TEXT)], Sense::click());
                record(rects, "brightness", r.rect, measuring);
                if r.clicked() {
                    actions.push(Action::ToggleOverlay(Overlay::Screen, r.rect.center().x));
                }
                if r.hovered() {
                    let dy = ui.input(|i| i.smooth_scroll_delta.y);
                    if dy.abs() > 0.5 {
                        actions.push(Action::BrightnessScroll(if dy > 0.0 { 5 } else { -5 }));
                    }
                }
            }
        }
        "notifications" => {
            if inp.notif_blocked {
                item(ui, Some((Icon::BellOff, theme::accent())), &[("Blocked", theme::accent())], Sense::hover())
                    .on_hover_text("Another notification daemon owns the name. Notifications are off.");
            }
            // Filled in the accent colour when something is waiting. No count.
            let (icon, col) = if inp.dnd {
                (Icon::BellOff, theme::accent())
            } else if inp.unread > 0 {
                (Icon::BellFilled, theme::accent())
            } else {
                (Icon::Bell, theme::DIM)
            };
            let r = item(ui, Some((icon, col)), &[], Sense::click());
            record(rects, "notifications", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::History, r.rect.center().x));
            }
        }
        "clock" => {
            let parts: Vec<(&str, egui::Color32)> = if inp.clock_date.is_empty() {
                vec![(inp.clock_time.as_str(), theme::TEXT)]
            } else {
                vec![(inp.clock_date.as_str(), theme::DIM), (inp.clock_time.as_str(), theme::TEXT)]
            };
            let r = item(ui, None, &parts, Sense::click());
            record(rects, "clock", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Calendar(0), r.rect.center().x));
            }
        }
        "media" => {
            if let Some((line, playing, bus)) = &inp.media {
                let icon = if *playing { Icon::Pause } else { Icon::Play };
                let r = item(ui, Some((icon, theme::DIM)), &[(&clip(line, 34), theme::TEXT)], Sense::click());
                record(rects, "media", r.rect, measuring);
                if r.clicked() {
                    actions.push(Action::ToggleOverlay(Overlay::Media, r.rect.center().x));
                }
                if r.middle_clicked() {
                    actions.push(Action::MediaPlayPause(bus.clone()));
                }
                if r.secondary_clicked() {
                    actions.push(Action::MediaNext(bus.clone()));
                }
                r.on_hover_text("Click for controls. Middle-click plays or pauses. Right-click skips.");
            }
        }
        "capture" => {
            let r = item(ui, Some((Icon::Camera, theme::DIM)), &[], Sense::click());
            record(rects, "capture", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::CaptureOpen);
            }
        }
        "settings" => {
            let r = item(ui, Some((Icon::Gear, theme::DIM)), &[], Sense::click());
            record(rects, "settings", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::OpenSettings);
            }
        }
        "power" => {
            let r = item(ui, Some((Icon::Power, theme::DIM)), &[], Sense::click());
            record(rects, "power", r.rect, measuring);
            if r.clicked() {
                actions.push(Action::ToggleOverlay(Overlay::Power(None), r.rect.center().x));
            }
        }
        _ => {}
    }
}

/// Minimized windows show an icon. Windows in alert mode open up with a title.
/// A window title cleaned for the bar: no "(3)" unread count in front and no
/// app name tacked on the end, so "(1) Cyril Robert \u{2013} (Telegram)" is "Cyril Robert".
fn short_title(title: &str) -> String {
    let t = title.trim();
    let t = match t.strip_prefix('(') {
        Some(rest) => match rest.split_once(')') {
            Some((n, after)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => after.trim_start(),
            _ => t,
        },
        None => t,
    };
    let t = [" \u{2013} ", " \u{2014} ", " - ", " | "]
        .iter()
        .filter_map(|sep| t.split_once(sep).map(|(first, _)| first))
        .min_by_key(|f| f.len())
        .unwrap_or(t)
        .trim();
    if t.is_empty() { title.trim().to_owned() } else { t.to_owned() }
}

fn attention(ui: &mut Ui, inp: &Input, actions: &mut Vec<Action>) {
    for e in inp.attention {
        let h = 22.0;
        let title = clip(&short_title(&e.title), 22);
        let font = crate::theme::fid(12.0);
        let galley = ui.painter().layout_no_wrap(title, font, theme::TEXT);
        let w = if e.alert { 26.0 + 2.0 + galley.size().x + 8.0 } else { 26.0 };
        ui.add_space(4.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
        let p = ui.painter();
        // A quiet fill, no outline. The accent dot on the icon says it wants you.
        if e.alert {
            p.rect_filled(rect, 4, if resp.hovered() { theme::PLATE_SEL } else { theme::accent_bg() });
        } else if resp.hovered() {
            p.rect_filled(rect, 4, theme::PLATE);
        }
        let name = format!("{}\n{}", e.app_id, e.title);
        let icon_c = pos2(rect.min.x + 13.0, rect.center().y);
        tile(p, icon_c, 16.0, &name);
        if e.alert {
            p.circle_filled(icon_c + vec2(6.5, -6.5), 3.5, theme::accent());
            p.circle_stroke(icon_c + vec2(6.5, -6.5), 3.5, egui::Stroke::new(1.0, theme::BAR));
            p.galley(pos2(rect.min.x + 28.0, rect.center().y - galley.size().y / 2.0), galley, theme::TEXT);
        }
        let resp = resp.on_hover_text(e.title.clone());
        if resp.clicked() {
            actions.push(Action::FocusWindow(e.index));
        }
    }
}

