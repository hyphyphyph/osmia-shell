//! The overview: a card for every open window, over a dimmed screen.
//! Click a card to bring that window forward. Type to filter. Escape closes.

use egui::{pos2, vec2, Align2, Color32, Key, Rect, Sense, Ui};

use crate::overview::{self, Thumb};
use crate::theme::{self, fid};
use crate::ui::{Action, WinEntry};

const GAP: f32 = 22.0;
const LABEL_H: f32 = 34.0;
const MAX_CARD_W: f32 = 480.0;

pub fn show(ui: &mut Ui, wins: &[WinEntry], thumbs: &[Thumb], actions: &mut Vec<Action>) {
    let full = ui.max_rect();
    let query_id = crate::ui::widgets::id("ov-query");
    let sel_id = crate::ui::widgets::id("ov-sel");
    let mut query: String = ui.data(|d| d.get_temp(query_id)).unwrap_or_default();
    let mut sel: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(0);

    // A click on the empty screen closes. Cards are added after, so they win.
    let bg = ui.interact(full, crate::ui::widgets::id("ov-bg"), Sense::click());
    ui.painter().rect_filled(full, 0, Color32::from_black_alpha(205));
    if bg.clicked() {
        actions.push(Action::CloseOverlay);
    }

    // Keys: typing filters, arrows move, Enter picks, Escape closes.
    let (mut left, mut right, mut up, mut down, mut enter, mut esc, mut back) = (false, false, false, false, false, false, false);
    ui.input_mut(|i| {
        left = i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft);
        right = i.consume_key(egui::Modifiers::NONE, Key::ArrowRight);
        up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
        down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
        enter = i.consume_key(egui::Modifiers::NONE, Key::Enter);
        esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
        back = i.consume_key(egui::Modifiers::NONE, Key::Backspace);
        for e in &i.events {
            if let egui::Event::Text(t) = e {
                query.extend(t.chars().filter(|c| !c.is_control()));
            }
        }
    });
    if back {
        query.pop();
    }
    if esc {
        if query.is_empty() {
            actions.push(Action::CloseOverlay);
        } else {
            query.clear();
        }
    }

    let shown: Vec<&WinEntry> = wins.iter().filter(|w| overview::passes(&query, &w.title, &w.app_name)).collect();
    let n = shown.len();

    // The search line.
    let line = if query.is_empty() { "Type to filter windows".to_owned() } else { query.clone() };
    let col = if query.is_empty() { theme::DIM } else { theme::TEXT };
    ui.painter().text(pos2(full.center().x, full.min.y + 44.0), Align2::CENTER_CENTER, line, fid(15.0), col);
    let count = if n == 1 { "1 window".to_owned() } else { format!("{n} windows") };
    ui.painter().text(pos2(full.center().x, full.min.y + 68.0), Align2::CENTER_CENTER, count, fid(11.0), theme::DIM);

    if n == 0 {
        let msg = if wins.is_empty() { "No open windows" } else { "No window matches" };
        ui.painter().text(full.center(), Align2::CENTER_CENTER, msg, fid(14.0), theme::DIM);
        ui.data_mut(|d| {
            d.insert_temp(query_id, query);
            d.insert_temp(sel_id, 0usize);
        });
        return;
    }

    let area = Rect::from_min_max(pos2(full.min.x + 56.0, full.min.y + 96.0), pos2(full.max.x - 56.0, full.max.y - 56.0));
    let g = overview::grid(n, area.width(), area.height(), GAP, LABEL_H, MAX_CARD_W);
    let card_h = g.card_w * overview::ASPECT + LABEL_H;

    // Arrow keys move the mark by row and column.
    sel = sel.min(n - 1);
    if left {
        sel = sel.saturating_sub(1);
    }
    if right {
        sel = (sel + 1).min(n - 1);
    }
    if up && sel >= g.cols {
        sel -= g.cols;
    }
    if down && sel + g.cols < n {
        sel += g.cols;
    }
    if enter {
        actions.push(Action::FocusWindow(shown[sel].index));
    }

    // Pictures belong to windows by title and app id, over all windows, so a hidden
    // twin keeps its own picture.
    let all_keys: Vec<(&str, &str)> = wins.iter().map(|w| (w.title.as_str(), w.app_id.as_str())).collect();
    let all_pics = overview::match_thumbs(&all_keys, thumbs);
    let pics: Vec<Option<usize>> = shown.iter().map(|w| wins.iter().position(|x| x.index == w.index).and_then(|p| all_pics[p])).collect();

    let grid_w = g.cols as f32 * g.card_w + GAP * (g.cols as f32 - 1.0);
    let grid_h = g.rows as f32 * card_h + GAP * (g.rows as f32 - 1.0);
    let origin = pos2(area.center().x - grid_w / 2.0, area.center().y - grid_h / 2.0);

    // Where each card is, so the pointer can be tested before anything is drawn.
    let rects: Vec<Rect> = (0..n)
        .map(|i| {
            let (row, col) = (i / g.cols, i % g.cols);
            // The last row is centred.
            let in_row = if row == g.rows - 1 { n - row * g.cols } else { g.cols };
            let shift = (g.cols - in_row) as f32 * (g.card_w + GAP) / 2.0;
            Rect::from_min_size(origin + vec2(shift + col as f32 * (g.card_w + GAP), row as f32 * (card_h + GAP)), vec2(g.card_w, card_h))
        })
        .collect();
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let moved = ui.input(|i| i.pointer.delta() != vec2(0.0, 0.0));
    let hovered = pointer.and_then(|p| rects.iter().position(|r| r.contains(p)));
    // The pointer moves the mark. When it is still, the keys own it.
    if let (Some(h), true) = (hovered, moved) {
        sel = h;
    }
    for (i, w) in shown.iter().enumerate() {
        let r = rects[i];
        let resp = ui.interact(r, crate::ui::widgets::id(("ov-card", w.index)), Sense::click());
        card(ui, r, w, pics[i].map(|p| &thumbs[p]), sel == i, hovered == Some(i), actions);
        if resp.clicked() && hovered == Some(i) {
            actions.push(Action::FocusWindow(w.index));
        }
        if resp.middle_clicked() {
            actions.push(Action::CloseWindow(w.index));
        }
    }

    ui.data_mut(|d| {
        d.insert_temp(query_id, query);
        d.insert_temp(sel_id, sel);
    });
}

fn card(ui: &mut Ui, r: Rect, w: &WinEntry, thumb: Option<&Thumb>, marked: bool, hover: bool, actions: &mut Vec<Action>) {
    let pic = Rect::from_min_size(r.min, vec2(r.width(), r.width() * overview::ASPECT));
    let radius = 8;
    ui.painter().rect_filled(pic, radius, theme::PLATE);
    match thumb {
        Some(t) => {
            // Fit inside the picture area, keeping the shape of the window.
            let s = (pic.width() / t.image.w as f32).min(pic.height() / t.image.h as f32);
            let size = vec2(t.image.w as f32 * s, t.image.h as f32 * s);
            let dst = Rect::from_center_size(pic.center(), size);
            let tex = texture(ui, w.index, t);
            let tint = if w.minimized { Color32::from_gray(150) } else { Color32::WHITE };
            ui.painter().image(tex.id(), dst, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
        }
        None => {
            // No picture yet, or none possible: the app icon, large.
            let key = if w.app_id.is_empty() { w.app_name.as_str() } else { w.app_id.as_str() };
            if let Some(tex) = crate::appicon::texture(ui.ctx(), key, 64.0) {
                ui.painter().image(tex.id(), Rect::from_center_size(pic.center(), vec2(64.0, 64.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
    }
    let stroke = if marked {
        egui::Stroke::new(2.0, theme::accent())
    } else if w.active {
        egui::Stroke::new(1.5, theme::accent_bg())
    } else {
        egui::Stroke::new(1.0, theme::CARD_LINE)
    };
    ui.painter().rect_stroke(pic, radius, stroke, egui::StrokeKind::Outside);
    if w.minimized {
        ui.painter().text(pic.left_top() + vec2(10.0, 12.0), Align2::LEFT_CENTER, "Minimized", fid(10.0), theme::DIM);
    }

    // The label: small icon and title.
    let ly = pic.max.y + LABEL_H / 2.0 + 2.0;
    let mut tx = r.min.x + 4.0;
    let key = if w.app_id.is_empty() { w.app_name.as_str() } else { w.app_id.as_str() };
    if let Some(tex) = crate::appicon::texture(ui.ctx(), key, 16.0) {
        ui.painter().image(tex.id(), Rect::from_center_size(pos2(tx + 8.0, ly), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        tx += 22.0;
    }
    let title = if w.title.is_empty() { w.app_name.as_str() } else { w.title.as_str() };
    let clipped = crate::ui::panels::clip_px(ui, title, 12.0, r.max.x - tx - 4.0);
    ui.painter().text(pos2(tx, ly), Align2::LEFT_CENTER, clipped, fid(12.0), if marked { theme::TEXT } else { theme::DIM });

    // A close button on the hovered card.
    if hover {
        let c = Rect::from_center_size(pic.right_top() + vec2(-16.0, 16.0), vec2(22.0, 22.0));
        let cr = ui.interact(c, crate::ui::widgets::id(("ov-close", w.index)), Sense::click());
        ui.painter().circle_filled(c.center(), 11.0, if cr.hovered() { theme::accent() } else { Color32::from_black_alpha(180) });
        crate::icons::draw(ui.painter(), crate::icons::Icon::Close, c.shrink(6.0), if cr.hovered() { Color32::BLACK } else { theme::TEXT });
        if cr.clicked() {
            actions.push(Action::CloseWindow(w.index));
        }
    }
}

/// The picture as a texture, made once per window and kept for this overview.
fn texture(ui: &Ui, key: usize, t: &Thumb) -> egui::TextureHandle {
    let tid = crate::ui::widgets::id(("ov-tex", key, t.image.w, t.image.h));
    if let Some(h) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(tid)) {
        return h;
    }
    let ci = egui::ColorImage::from_rgba_unmultiplied([t.image.w as usize, t.image.h as usize], &t.image.rgba);
    let h = ui.ctx().load_texture(format!("ov-{key}"), ci, egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|d| d.insert_temp(tid, h.clone()));
    h
}
