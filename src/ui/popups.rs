//! The popup pile in the corner. The newest popups show as full cards.
//! When more do not fit, older ones collapse into readable strips.

use egui::{pos2, vec2, Align2, Rect, Sense, Ui};

use crate::notif::{strip_markup, Notification};
use crate::theme;
use crate::ui::widgets::{age, card_frame, pill, tile};
use crate::ui::Action;

pub const WIDTH: f32 = 360.0;
pub const MARGIN: f32 = 14.0;
const CARD_H_ESTIMATE: f32 = 96.0;

pub struct Out {
    /// Areas that take pointer input. Everything else passes through.
    pub input_rects: Vec<Rect>,
    pub hovered: bool,
}

pub fn show(ui: &mut Ui, popups: &[Notification], screen_h: f32, bottom_up: bool, actions: &mut Vec<Action>) -> Out {
    let full = ui.max_rect();
    let mut out = Out { input_rects: vec![], hovered: false };
    if popups.is_empty() {
        return out;
    }
    // Newest first for layout.
    let items: Vec<&Notification> = popups.iter().rev().collect();
    let fit = (((screen_h - 160.0) / (CARD_H_ESTIMATE + 8.0)).floor() as usize).max(1);
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let overflow = items.len() > fit;
    let pile_rect_guess = Rect::from_min_max(
        pos2(full.max.x - WIDTH - MARGIN, full.min.y),
        pos2(full.max.x - MARGIN, full.max.y),
    );
    let spread = overflow && hover_pos.is_some_and(|p| pile_rect_guess.contains(p));
    out.hovered = hover_pos.is_some_and(|p| pile_rect_guess.contains(p)) && !items.is_empty();

    let x = full.max.x - WIDTH - MARGIN;
    let width = WIDTH;
    let mut y_cursor = if bottom_up { full.max.y - MARGIN } else { full.min.y + MARGIN };

    // Lay cards out. For bottom_up the newest sits nearest the bottom edge.
    let place = |h: f32, y: &mut f32| -> Rect {
        if bottom_up {
            *y -= h;
            let r = Rect::from_min_size(pos2(x, *y), vec2(width, h));
            *y -= 8.0;
            r
        } else {
            let r = Rect::from_min_size(pos2(x, *y), vec2(width, h));
            *y += h + 8.0;
            r
        }
    };

    let full_count = if overflow && !spread { 1 } else if overflow { 1 } else { items.len() };
    for (i, n) in items.iter().enumerate() {
        if i < full_count {
            let h = card_height(n);
            let r = place(h, &mut y_cursor);
            out.input_rects.push(r.expand(2.0));
            card(ui, r, n, actions);
        } else if spread {
            let r = place(30.0, &mut y_cursor);
            out.input_rects.push(r.expand(2.0));
            row(ui, r, n, actions);
        }
    }
    if overflow && !spread {
        // Up to three strips and a "+N more" line, above the newest card.
        let older = &items[full_count..];
        let shown = older.len().min(3);
        let top_line = y_cursor;
        let mut ys = y_cursor;
        for (j, n) in older.iter().take(shown).enumerate() {
            let inset = 6.0 * (j as f32 + 1.0);
            let r = if bottom_up {
                ys -= 20.0;
                Rect::from_min_size(pos2(x + inset, ys), vec2(width - inset * 2.0, 20.0))
            } else {
                let r = Rect::from_min_size(pos2(x + inset, ys), vec2(width - inset * 2.0, 20.0));
                ys += 20.0;
                r
            };
            strip(ui, r, n);
            out.input_rects.push(r);
        }
        let more = older.len() - shown;
        let line_y = if bottom_up { ys - 22.0 } else { ys };
        let line = Rect::from_min_size(pos2(x, line_y), vec2(width, 22.0));
        if more > 0 {
            ui.painter().text(
                pos2(line.min.x + 4.0, line.center().y),
                Align2::LEFT_CENTER,
                format!("+{more} more"),
                crate::theme::fid(11.0),
                theme::DIM,
            );
        }
        clear_button(ui, line, items.len(), actions);
        out.input_rects.push(line);
        let _ = top_line;
    } else if overflow && spread {
        let line = Rect::from_min_size(pos2(x, full.min.y + 4.0), vec2(width, 22.0));
        clear_button(ui, line, items.len(), actions);
        out.input_rects.push(line);
    } else if items.len() >= 2 {
        // Everything fits as full cards. Still offer clear-all.
        let top = if bottom_up { y_cursor - 22.0 } else { y_cursor };
        let line = Rect::from_min_size(pos2(x, top), vec2(width, 22.0));
        clear_button(ui, line, items.len(), actions);
        out.input_rects.push(line);
    }
    out
}

fn clear_button(ui: &mut Ui, line: Rect, count: usize, actions: &mut Vec<Action>) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(line)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    if pill(&mut child, &format!("Clear all ({count})"), false) {
        actions.push(Action::ClearPopups);
    }
}

fn card_height(n: &Notification) -> f32 {
    let body_lines: f32 = strip_markup(&n.body)
        .split('\n')
        .map(|l| (l.chars().count() as f32 / 46.0).ceil().max(1.0))
        .sum::<f32>()
        .clamp(0.0, 4.0);
    let body_lines = if n.body.is_empty() { 0.0 } else { body_lines };
    let mut h = 56.0 + body_lines * 16.0;
    if n.buttons().next().is_some() {
        h += 34.0;
    }
    h
}

fn card(ui: &mut Ui, r: Rect, n: &Notification, actions: &mut Vec<Action>) {
    let id = crate::ui::widgets::id(("popup", n.id));
    let resp = ui.interact(r, id, Sense::click());
    ui.painter().add(card_frame().paint(r));
    if n.urgency == crate::notif::URGENCY_CRITICAL {
        let edge = Rect::from_min_max(r.min + vec2(0.0, 8.0), pos2(r.min.x + 3.0, r.max.y - 8.0));
        ui.painter().rect_filled(edge, 1.5, theme::accent());
    }
    let p = ui.painter();
    tile(p, pos2(r.min.x + 20.0, r.min.y + 19.0), 16.0, &n.icon_key());
    p.text(
        pos2(r.min.x + 34.0, r.min.y + 19.0),
        Align2::LEFT_CENTER,
        &n.app_name,
        crate::theme::fid(11.0),
        if n.urgency == crate::notif::URGENCY_CRITICAL { theme::accent() } else { theme::TEXT },
    );
    p.text(
        pos2(r.max.x - 12.0, r.min.y + 19.0),
        Align2::RIGHT_CENTER,
        age(n.born),
        crate::theme::fid(11.0),
        theme::DIM,
    );
    p.text(
        pos2(r.min.x + 12.0, r.min.y + 38.0),
        Align2::LEFT_CENTER,
        clip(&n.summary, 44),
        crate::theme::fid(13.0),
        theme::TEXT,
    );
    let body = strip_markup(&n.body);
    if !body.is_empty() {
        let galley = p.layout(
            body,
            crate::theme::fid(12.0),
            theme::DIM,
            width_wrap(r),
        );
        let clipped = p.with_clip_rect(Rect::from_min_size(r.min + vec2(12.0, 48.0), vec2(r.width() - 24.0, 64.0)));
        clipped.galley(pos2(r.min.x + 12.0, r.min.y + 48.0), galley, theme::DIM);
    }
    let buttons: Vec<_> = n.buttons().collect();
    if !buttons.is_empty() {
        let mut bx = r.min.x + 12.0;
        let by = r.max.y - 30.0;
        for (key, label) in buttons.iter().take(4) {
            let rect = Rect::from_min_size(pos2(bx, by), vec2(label.chars().count() as f32 * 6.5 + 22.0, 22.0));
            let bresp = ui.interact(rect, crate::ui::widgets::id(("pbtn", n.id, key.as_str())), Sense::click());
            let fill = if bresp.hovered() { theme::PLATE_SEL } else { theme::BTN };
            ui.painter().rect(rect, 4, fill, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, crate::theme::fid(12.0), theme::TEXT);
            if bresp.clicked() {
                actions.push(Action::NotifButton(n.id, (*key).clone()));
            }
            bx += rect.width() + 6.0;
        }
    }
    let mut closed = false;
    if ui.rect_contains_pointer(r) {
        let x = Rect::from_center_size(pos2(r.max.x - 20.0, r.min.y + 38.0), vec2(18.0, 18.0));
        let xr = ui.interact(x, crate::ui::widgets::id(("pclose", n.id)), Sense::click());
        ui.painter().rect_filled(x, 4, if xr.hovered() { theme::PLATE_SEL } else { theme::BTN });
        crate::icons::draw(ui.painter(), crate::icons::Icon::Close, x.shrink(3.0), theme::DIM);
        if xr.clicked() {
            actions.push(Action::NotifDismiss(n.id));
            closed = true;
        }
    }
    if resp.clicked() && !closed {
        actions.push(Action::NotifClick(n.id));
    }
    if resp.secondary_clicked() {
        actions.push(Action::NotifDismiss(n.id));
    }
}

fn width_wrap(r: Rect) -> f32 {
    r.width() - 24.0
}

fn strip(ui: &mut Ui, r: Rect, n: &Notification) {
    let p = ui.painter();
    p.rect(r, 4, egui::Color32::from_rgb(0x29, 0x2d, 0x34), egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
    tile(p, pos2(r.min.x + 12.0, r.center().y), 12.0, &n.icon_key());
    p.text(
        pos2(r.min.x + 24.0, r.center().y),
        Align2::LEFT_CENTER,
        clip(&n.summary, 40),
        crate::theme::fid(11.0),
        theme::DIM,
    );
}

fn row(ui: &mut Ui, r: Rect, n: &Notification, actions: &mut Vec<Action>) {
    let resp = ui.interact(r, crate::ui::widgets::id(("popuprow", n.id)), Sense::click());
    let p = ui.painter();
    p.rect(
        r,
        4,
        if resp.hovered() { theme::PLATE } else { theme::CARD },
        egui::Stroke::new(1.0, theme::CARD_LINE),
        egui::StrokeKind::Inside,
    );
    tile(p, pos2(r.min.x + 16.0, r.center().y), 14.0, &n.icon_key());
    p.text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, clip(&n.app_name, 8), crate::theme::fid(11.0), theme::DIM);
    p.text(pos2(r.min.x + 92.0, r.center().y), Align2::LEFT_CENTER, clip(&n.summary, 30), crate::theme::fid(12.0), theme::TEXT);
    p.text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, age(n.born), crate::theme::fid(11.0), theme::DIM);
    if resp.clicked() {
        actions.push(Action::NotifClick(n.id));
    }
    if resp.secondary_clicked() {
        actions.push(Action::NotifDismiss(n.id));
    }
}

pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('\u{2026}');
        t
    }
}
