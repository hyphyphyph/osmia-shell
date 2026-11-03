//! The capture overlay: the frozen screen, a selection, a window picker and a toolbar.
//! Drawn once per output. All outputs share one state.

use std::sync::Arc;

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

use super::grab::Image;
use super::video::Audio;
use crate::theme::{self, fid};
use crate::ui::settings::segmented;
use crate::ui::widgets::{card_frame, id, pill, switch, tile};
use crate::ui::{Action, Target};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Region,
    Window,
    Screen,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Photo,
    Video,
}

pub struct Shot {
    pub name: String,
    pub image: Arc<Image>,
}

pub struct WinShot {
    pub id: String,
    pub title: String,
    pub app_id: String,
    pub image: Arc<Image>,
    /// A small copy for the picker.
    pub thumb: Arc<Image>,
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    Create(egui::Pos2),
    Move(egui::Vec2),
    Resize(Handle, Rect),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Handle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

pub struct CaptureUi {
    pub mode: Mode,
    pub kind: Kind,
    pub shots: Vec<Shot>,
    pub windows: Vec<WinShot>,
    /// The selected area: which output, and the rectangle in its logical coordinates.
    pub sel: Option<(usize, Rect)>,
    drag: Option<Drag>,
    pub audio: Audio,
    pub cursor: bool,
    pub delay: u32,
    /// The output the pointer is on, so the toolbar follows it.
    pub pointer_out: usize,
    /// Changes for every capture, so cached textures are not reused.
    pub generation: u64,
}

impl CaptureUi {
    pub fn new(shots: Vec<Shot>, windows: Vec<WinShot>, mode: Mode, kind: Kind, audio: Audio, cursor: bool, delay: u32) -> Self {
        static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        CaptureUi {
            mode,
            kind,
            shots,
            windows,
            sel: None,
            drag: None,
            audio,
            cursor,
            delay,
            pointer_out: 0,
            generation: GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }
}

fn texture(ui: &Ui, key: impl std::hash::Hash + std::fmt::Debug, img: &Image) -> egui::TextureHandle {
    let tid = id(key);
    if let Some(t) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(tid)) {
        return t;
    }
    let ci = egui::ColorImage::from_rgba_unmultiplied([img.w as usize, img.h as usize], &img.rgba);
    let t = ui.ctx().load_texture(format!("{tid:?}"), ci, egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|d| d.insert_temp(tid, t.clone()));
    t
}

const UV: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(1.0, 1.0) };
const HANDLE: f32 = 9.0;

fn handle_rects(r: Rect) -> [(Handle, Rect); 8] {
    let c = |p: egui::Pos2| Rect::from_center_size(p, vec2(HANDLE, HANDLE));
    [
        (Handle::Nw, c(r.left_top())),
        (Handle::N, c(pos2(r.center().x, r.min.y))),
        (Handle::Ne, c(r.right_top())),
        (Handle::E, c(pos2(r.max.x, r.center().y))),
        (Handle::Se, c(r.right_bottom())),
        (Handle::S, c(pos2(r.center().x, r.max.y))),
        (Handle::Sw, c(r.left_bottom())),
        (Handle::W, c(pos2(r.min.x, r.center().y))),
    ]
}

fn resized(orig: Rect, h: Handle, p: egui::Pos2) -> Rect {
    let mut r = orig;
    match h {
        Handle::Nw => {
            r.min = p;
        }
        Handle::N => r.min.y = p.y,
        Handle::Ne => {
            r.min.y = p.y;
            r.max.x = p.x;
        }
        Handle::E => r.max.x = p.x,
        Handle::Se => r.max = p,
        Handle::S => r.max.y = p.y,
        Handle::Sw => {
            r.min.x = p.x;
            r.max.y = p.y;
        }
        Handle::W => r.min.x = p.x,
    }
    // Dragging past the opposite side flips the rectangle.
    Rect::from_two_pos(r.min, r.max)
}

fn cursor_for(h: Handle) -> egui::CursorIcon {
    match h {
        Handle::Nw | Handle::Se => egui::CursorIcon::ResizeNwSe,
        Handle::Ne | Handle::Sw => egui::CursorIcon::ResizeNeSw,
        Handle::N | Handle::S => egui::CursorIcon::ResizeVertical,
        Handle::E | Handle::W => egui::CursorIcon::ResizeHorizontal,
    }
}

fn label(ui: &Ui, pos: egui::Pos2, anchor: Align2, text: &str) {
    let g = ui.painter().layout_no_wrap(text.to_owned(), fid(12.0), theme::TEXT);
    let r = anchor.anchor_size(pos, g.size()).expand2(vec2(8.0, 4.0));
    ui.painter().rect(r, 4, theme::CARD, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
    ui.painter().galley(r.center() - g.size() / 2.0, g, theme::TEXT);
}

/// Draw the overlay for one output.
pub fn show(ui: &mut Ui, st: &mut CaptureUi, idx: usize, actions: &mut Vec<Action>) {
    let full = ui.max_rect();
    let Some(shot) = st.shots.get(idx) else { return };
    let tex = texture(ui, ("capshot", st.generation, idx), &shot.image);
    ui.painter().image(tex.id(), full, UV, Color32::WHITE);
    if ui.rect_contains_pointer(full) {
        st.pointer_out = idx;
    }
    let dim = Color32::from_black_alpha(150);

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        actions.push(Action::CaptureCancel);
    }

    match st.mode {
        Mode::Region => region(ui, st, idx, full, dim, actions),
        Mode::Window => {
            ui.painter().rect_filled(full, 0, dim);
            windows(ui, st, full, actions);
        }
        Mode::Screen => {
            ui.painter().rect_filled(full, 0, dim);
            let resp = ui.interact(full, id(("capscreen", idx)), Sense::click());
            if resp.hovered() {
                ui.painter().image(tex.id(), full, UV, Color32::WHITE);
                ui.painter().rect_stroke(full.shrink(2.0), 0, egui::Stroke::new(3.0, theme::accent()), egui::StrokeKind::Inside);
                label(ui, full.center(), Align2::CENTER_CENTER, if st.kind == Kind::Photo { "Click to capture this screen" } else { "Click to record this screen" });
            }
            if resp.clicked() {
                actions.push(Action::CaptureGo(Target::Screen(idx)));
            }
        }
    }
    if st.pointer_out == idx || (st.shots.len() == 1) {
        toolbar(ui, st, full, actions);
    }
}

fn region(ui: &mut Ui, st: &mut CaptureUi, idx: usize, full: Rect, dim: Color32, actions: &mut Vec<Action>) {
    let sel_here = st.sel.filter(|(o, _)| *o == idx).map(|(_, r)| r);
    let resp = ui.interact(full, id(("capregion", idx)), Sense::click_and_drag());
    let hover = ui.input(|i| i.pointer.hover_pos());

    // What would a press do here? Shown by the cursor, used when a drag starts.
    let grab_at = |p: egui::Pos2| -> Option<Drag> {
        if let Some(r) = sel_here {
            for (h, hr) in handle_rects(r) {
                if hr.expand(4.0).contains(p) {
                    return Some(Drag::Resize(h, r));
                }
            }
            if r.contains(p) {
                return Some(Drag::Move(p - r.min));
            }
        }
        None
    };
    if let Some(p) = hover {
        let icon = match (st.drag, grab_at(p)) {
            (Some(Drag::Resize(h, _)), _) | (None, Some(Drag::Resize(h, _))) => cursor_for(h),
            (Some(Drag::Move(_)), _) | (None, Some(Drag::Move(_))) => egui::CursorIcon::Move,
            _ => egui::CursorIcon::Crosshair,
        };
        ui.ctx().set_cursor_icon(icon);
    }

    if resp.drag_started() {
        if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
            st.drag = Some(grab_at(p).unwrap_or(Drag::Create(p)));
        }
    }
    if let (Some(d), Some(p)) = (st.drag, resp.interact_pointer_pos().or(hover)) {
        if resp.dragged() {
            let p = p.clamp(full.min, full.max);
            let r = match d {
                Drag::Create(start) => Rect::from_two_pos(start, p),
                Drag::Move(off) => {
                    let cur = sel_here.unwrap_or(Rect::from_min_size(p, vec2(1.0, 1.0)));
                    let min = (p - off).clamp(full.min, full.max - cur.size());
                    Rect::from_min_size(min, cur.size())
                }
                Drag::Resize(h, orig) => resized(orig, h, p),
            };
            st.sel = Some((idx, r.intersect(full)));
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        // A click without a drag leaves no selection behind.
        if st.sel.is_some_and(|(_, r)| r.width() < 4.0 || r.height() < 4.0) {
            st.sel = None;
        }
    }
    if resp.double_clicked() {
        if let Some(r) = sel_here {
            if hover.is_some_and(|p| r.contains(p)) {
                actions.push(Action::CaptureGo(Target::Region(idx, r)));
            }
        }
    }

    let sel_here = st.sel.filter(|(o, _)| *o == idx).map(|(_, r)| r);
    match sel_here {
        None => {
            ui.painter().rect_filled(full, 0, dim);
            if st.sel.is_none() && st.shots.len() > 0 {
                label(ui, full.center(), Align2::CENTER_CENTER, "Drag to select an area");
            }
        }
        Some(r) => {
            // Dim everything except the selection, then show the selection at full brightness.
            for band in [
                Rect::from_min_max(full.min, pos2(full.max.x, r.min.y)),
                Rect::from_min_max(pos2(full.min.x, r.max.y), full.max),
                Rect::from_min_max(pos2(full.min.x, r.min.y), pos2(r.min.x, r.max.y)),
                Rect::from_min_max(pos2(r.max.x, r.min.y), pos2(full.max.x, r.max.y)),
            ] {
                ui.painter().rect_filled(band, 0, dim);
            }
            ui.painter().rect_stroke(r, 0, egui::Stroke::new(1.0, theme::accent()), egui::StrokeKind::Outside);
            for (_, hr) in handle_rects(r) {
                ui.painter().rect(hr, 2, theme::TEXT, egui::Stroke::new(1.0, theme::accent()), egui::StrokeKind::Inside);
            }
            // Size in real pixels, which is what ends up in the file.
            let scale = st.shots[idx].image.w as f32 / full.width();
            let text = format!("{} \u{d7} {}", (r.width() * scale).round(), (r.height() * scale).round());
            let above = r.min.y > 40.0;
            let (pos, anchor) = if above { (pos2(r.center().x, r.min.y - 10.0), Align2::CENTER_BOTTOM) } else { (pos2(r.center().x, r.max.y + 10.0), Align2::CENTER_TOP) };
            label(ui, pos, anchor, &text);
        }
    }

    let keys = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.modifiers.shift));
    if let Some((o, r)) = st.sel.filter(|(o, _)| *o == idx) {
        if keys.0 {
            actions.push(Action::CaptureGo(Target::Region(o, r)));
        }
        // Arrow keys nudge the selection; Shift moves it ten pixels at a time.
        let step = if keys.5 { 10.0 } else { 1.0 };
        let d = vec2((keys.2 as i32 - keys.1 as i32) as f32 * step, (keys.4 as i32 - keys.3 as i32) as f32 * step);
        if d != vec2(0.0, 0.0) {
            let min = (r.min + d).clamp(full.min, full.max - r.size());
            st.sel = Some((o, Rect::from_min_size(min, r.size())));
        }
    }
}

fn windows(ui: &mut Ui, st: &mut CaptureUi, full: Rect, actions: &mut Vec<Action>) {
    const CELL_W: f32 = 250.0;
    const CELL_H: f32 = 186.0;
    if st.windows.is_empty() {
        label(ui, full.center(), Align2::CENTER_CENTER, "No windows to capture");
        return;
    }
    let cols = (((full.width() - 120.0) / CELL_W).floor() as usize).clamp(1, 6).min(st.windows.len());
    let rows = st.windows.len().div_ceil(cols);
    let width = cols as f32 * CELL_W;
    let height = (rows as f32 * CELL_H).min(full.height() - 220.0).max(CELL_H);
    let area = Rect::from_center_size(full.center() - vec2(0.0, 24.0), vec2(width, height));
    let mut go = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for row in st.windows.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    for (c, w) in row.1.iter().enumerate() {
                        let i = row.0 * cols + c;
                        let (cell, resp) = ui.allocate_exact_size(vec2(CELL_W, CELL_H), Sense::click());
                        let inner = cell.shrink(8.0);
                        ui.painter().rect(
                            inner,
                            4,
                            if resp.hovered() { theme::PLATE_SEL } else { theme::CARD },
                            egui::Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, if resp.hovered() { theme::accent() } else { theme::CARD_LINE }),
                            egui::StrokeKind::Inside,
                        );
                        // The picture, fitted into its box.
                        let bx = Rect::from_min_size(inner.min + vec2(8.0, 8.0), vec2(inner.width() - 16.0, inner.height() - 44.0));
                        let aspect = w.thumb.w as f32 / w.thumb.h.max(1) as f32;
                        let fit = if bx.width() / bx.height() > aspect { vec2(bx.height() * aspect, bx.height()) } else { vec2(bx.width(), bx.width() / aspect) };
                        let t = texture(ui, ("capwin", st.generation, i), &w.thumb);
                        ui.painter().image(t.id(), Rect::from_center_size(bx.center(), fit), UV, Color32::WHITE);
                        tile(ui.painter(), pos2(inner.min.x + 20.0, inner.max.y - 18.0), 16.0, &format!("{}\n{}", w.app_id, w.title));
                        ui.painter().text(
                            pos2(inner.min.x + 36.0, inner.max.y - 18.0),
                            Align2::LEFT_CENTER,
                            crate::ui::popups::clip(&w.title, 26),
                            fid(12.0),
                            theme::TEXT,
                        );
                        if resp.clicked() {
                            go = Some(i);
                        }
                    }
                });
            }
        });
    });
    if let Some(i) = go {
        actions.push(Action::CaptureGo(Target::Window(i)));
    }
}

fn toolbar(ui: &mut Ui, st: &mut CaptureUi, full: Rect, actions: &mut Vec<Action>) {
    let pos = pos2(full.center().x, full.max.y - 36.0);
    egui::Area::new(id("capture-toolbar"))
        .fixed_pos(pos)
        .pivot(Align2::CENTER_BOTTOM)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            card_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.set_height(48.0);
                    let modes = [Mode::Region, Mode::Window, Mode::Screen];
                    if let Some(i) = segmented(ui, "cap-mode", &["Area", "Window", "Screen"], modes.iter().position(|m| *m == st.mode)) {
                        st.mode = modes[i];
                    }
                    ui.add_space(14.0);
                    if let Some(i) = segmented(ui, "cap-kind", &["Photo", "Video"], Some(st.kind as usize)) {
                        st.kind = [Kind::Photo, Kind::Video][i];
                    }
                    ui.add_space(14.0);
                    if st.kind == Kind::Video {
                        let audios = [Audio::None, Audio::System, Audio::Mic, Audio::Both];
                        ui.label(egui::RichText::new("Sound").size(theme::scaled(12.0)).color(theme::DIM));
                        ui.add_space(6.0);
                        if let Some(i) = segmented(ui, "cap-audio", &["Off", "System", "Mic", "Both"], audios.iter().position(|a| *a == st.audio)) {
                            st.audio = audios[i];
                        }
                        ui.add_space(14.0);
                    }
                    ui.label(egui::RichText::new("Cursor").size(theme::scaled(12.0)).color(theme::DIM));
                    ui.add_space(6.0);
                    if switch(ui, st.cursor) {
                        st.cursor = !st.cursor;
                    }
                    ui.add_space(14.0);
                    let delays = [0u32, 3, 5, 10];
                    ui.label(egui::RichText::new("Delay").size(theme::scaled(12.0)).color(theme::DIM));
                    ui.add_space(6.0);
                    if let Some(i) = segmented(ui, "cap-delay", &["0", "3 s", "5 s", "10 s"], delays.iter().position(|d| *d == st.delay)) {
                        st.delay = delays[i];
                    }
                    ui.add_space(18.0);
                    if pill(ui, "Cancel", false) {
                        actions.push(Action::CaptureCancel);
                    }
                    ui.add_space(8.0);
                    let verb = if st.kind == Kind::Photo { "Capture" } else { "Record" };
                    match (st.mode, st.sel) {
                        (Mode::Region, Some((o, r))) => {
                            if pill(ui, verb, true) {
                                actions.push(Action::CaptureGo(Target::Region(o, r)));
                            }
                        }
                        (Mode::Region, None) => {
                            ui.label(egui::RichText::new("Select an area").size(theme::scaled(12.0)).color(theme::DIM));
                        }
                        (Mode::Window, _) => {
                            ui.label(egui::RichText::new("Pick a window").size(theme::scaled(12.0)).color(theme::DIM));
                        }
                        (Mode::Screen, _) => {
                            ui.label(egui::RichText::new("Click a screen").size(theme::scaled(12.0)).color(theme::DIM));
                        }
                    }
                    ui.add_space(12.0);
                });
            });
        });
}
