//! The overview: a card for every open window, over a dimmed screen.
//! Also the launcher: the search box is on top, and typing swaps the cards for results.

use egui::{pos2, vec2, Align2, Color32, Key, Rect, Sense, Ui};

use crate::launch::{Data, Kind, WinInfo};
use crate::overview::{self, Thumb};
use crate::theme::{self, fid};
use crate::ui::{launcher, Action, WinEntry};

const GAP: f32 = 22.0;
const LABEL_H: f32 = 34.0;
const MAX_CARD_W: f32 = 480.0;

/// Everything the overview needs to draw: the search state and data of the
/// launcher, the windows and their pictures, and the sizes from the config.
pub struct Ctx<'a> {
    pub st: &'a mut launcher::State,
    pub data: &'a Data,
    pub infos: &'a [WinInfo],
    pub wins: &'a [WinEntry],
    pub thumbs: &'a [Thumb],
    pub width: f32,
    pub max_rows: usize,
}

/// The overview and the launcher in one: a search box on top, the windows below,
/// and a row of the most used apps at the bottom. Typing swaps the windows for results.
pub fn show(ui: &mut Ui, c: Ctx, actions: &mut Vec<Action>) {
    let Ctx { st, data, infos, wins, thumbs, width, max_rows } = c;
    let full = ui.max_rect();
    let sel_id = crate::ui::widgets::id("ov-sel");
    let mut sel: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(0);
    let searching = !st.query.is_empty();

    // A click on the empty screen closes. Cards are added after, so they win.
    let bg = ui.interact(full, crate::ui::widgets::id("ov-bg"), Sense::click());
    ui.painter().rect_filled(full, 0, Color32::from_black_alpha(205));
    if bg.clicked() {
        actions.push(Action::CloseOverlay);
    }

    // Keys first, so the text box does not see them. With nothing typed the arrows
    // and Enter belong to the window cards. Once something is typed they belong to
    // the results.
    let (mut left, mut right, mut up, mut down, mut enter, mut esc) = (false, false, false, false, false, false);
    let mut keys = launcher::Keys::default();
    ui.input_mut(|i| {
        esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
        enter = i.consume_key(egui::Modifiers::NONE, Key::Enter);
        if searching {
            keys.enter = enter;
            keys.down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown) || i.consume_key(egui::Modifiers::CTRL, Key::N);
            keys.up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp) || i.consume_key(egui::Modifiers::CTRL, Key::P);
            for (n, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4].into_iter().enumerate() {
                if i.consume_key(egui::Modifiers::ALT, key) {
                    keys.alt_pick = Some(n);
                }
            }
        } else {
            left = i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft);
            right = i.consume_key(egui::Modifiers::NONE, Key::ArrowRight);
            up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
            down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
        }
    });
    if esc {
        // The first Escape clears what was typed. The next closes.
        if searching {
            st.query.clear();
            st.selected = 0;
        } else {
            actions.push(Action::CloseOverlay);
        }
    }

    // The search box, and the results under it while something is typed.
    let card_rect = launcher::card(ui, st, data, infos, width, max_rows, &keys, actions);
    if searching == st.query.is_empty() {
        // Typing began or the box was cleared: draw the other view next frame.
        ui.ctx().request_repaint();
    }
    if !st.query.is_empty() {
        return;
    }

    let n = wins.len();
    if n == 0 {
        ui.painter().text(full.center(), Align2::CENTER_CENTER, "No open windows", fid(14.0), theme::DIM);
    }

    // The most used apps, along the bottom.
    let dash_h = dash(ui, full, data, actions);

    if n == 0 {
        ui.data_mut(|d| d.insert_temp(sel_id, 0usize));
        return;
    }
    let ptop = card_rect.max.y.max(full.min.y + 90.0) + 28.0;
    let area = Rect::from_min_max(pos2(full.min.x + 56.0, ptop), pos2(full.max.x - 56.0, (full.max.y - dash_h - 28.0).max(ptop + 80.0)));
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
        actions.push(Action::FocusWindow(wins[sel].index));
    }

    let keys: Vec<(&str, &str)> = wins.iter().map(|w| (w.title.as_str(), w.app_id.as_str())).collect();
    let pics = overview::match_thumbs(&keys, thumbs);

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
    for (i, w) in wins.iter().enumerate() {
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

    ui.data_mut(|d| d.insert_temp(sel_id, sel));
}

const DASH_ICON: f32 = 44.0;
const DASH_APPS: usize = 8;

/// The most used apps in a row at the bottom. Returns the height it takes.
fn dash(ui: &mut Ui, full: Rect, data: &Data, actions: &mut Vec<Action>) -> f32 {
    let items = data.top(DASH_APPS);
    if items.is_empty() {
        return 0.0;
    }
    let pad = 12.0;
    let w = items.len() as f32 * (DASH_ICON + pad) + pad;
    let h = DASH_ICON + pad * 2.0;
    let bar = Rect::from_center_size(pos2(full.center().x, full.max.y - h / 2.0 - 20.0), vec2(w, h));
    ui.painter().rect(bar, 14, Color32::from_black_alpha(120), egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
    let mut hover_name: Option<(String, Rect)> = None;
    for (i, item) in items.iter().enumerate() {
        let Kind::App(a) = &item.kind else { continue };
        let c = pos2(bar.min.x + pad + DASH_ICON / 2.0 + i as f32 * (DASH_ICON + pad), bar.center().y);
        let r = Rect::from_center_size(c, vec2(DASH_ICON, DASH_ICON));
        let resp = ui.interact(r, crate::ui::widgets::id(("ov-dash", &a.id)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r.expand(4.0), 10, theme::PLATE_SEL);
            hover_name = Some((a.name.clone(), r));
        }
        let key = format!("{}\n{}\n{}", a.id, a.name, a.exec.split_whitespace().next().unwrap_or(""));
        crate::ui::widgets::tile(ui.painter(), c, DASH_ICON * 0.82, &key);
        if resp.clicked() {
            actions.push(Action::RunApp(a.id.clone()));
        }
    }
    if let Some((name, r)) = hover_name {
        ui.painter().text(pos2(r.center().x, bar.min.y - 12.0), Align2::CENTER_CENTER, name, fid(12.0), theme::TEXT);
    }
    h + 20.0
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
