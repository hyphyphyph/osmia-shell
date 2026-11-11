//! The clipboard history card: the latest copies, newest first, over a dimmed screen.

use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Color32, Key, Rect, Sense, Ui};

use crate::clipboard::{self, History};
use crate::theme;
use crate::ui::widgets::pill;
use crate::ui::Action;

const ROW_H: f32 = 30.0;
const WIDTH: f32 = 480.0;

pub fn show(ui: &mut Ui, hist: &History, actions: &mut Vec<Action>) {
    let full = ui.max_rect();
    let sel_id = crate::ui::widgets::id("clip-sel");
    let mut sel: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(0);
    let n = hist.entries.len();

    // A click on the empty screen closes. The card is added after, so it wins.
    let bg = ui.interact(full, crate::ui::widgets::id("clip-bg"), Sense::click());
    ui.painter().rect_filled(full, 0, Color32::from_black_alpha(205));
    if bg.clicked() {
        actions.push(Action::CloseOverlay);
    }

    let (mut up, mut down, mut enter, mut esc, mut del) = (false, false, false, false, false);
    ui.input_mut(|i| {
        esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
        enter = i.consume_key(egui::Modifiers::NONE, Key::Enter);
        del = i.consume_key(egui::Modifiers::NONE, Key::Delete);
        up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp) || i.consume_key(egui::Modifiers::CTRL, Key::P);
        down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown) || i.consume_key(egui::Modifiers::CTRL, Key::N);
    });
    if esc {
        actions.push(Action::CloseOverlay);
    }
    if n > 0 {
        sel = sel.min(n - 1);
        if up {
            sel = sel.saturating_sub(1);
        }
        if down {
            sel = (sel + 1).min(n - 1);
        }
        if enter {
            actions.push(Action::ClipPick(sel));
        }
        if del {
            actions.push(Action::ClipDelete(sel));
        }
    }
    let keyed = up || down;
    let list_h = (full.height() - 36.0 - 130.0).clamp(ROW_H * 3.0, ROW_H * 10.0);

    crate::ui::popdown::Popdown::new(crate::ui::widgets::id("clipboard"), pos2(full.center().x, 36.0), Align2::CENTER_TOP)
        .margin(egui::Margin::same(6))
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH.min(full.width() - 24.0) - 12.0);
            // Header.
            let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
            ui.painter().text(pos2(hr.min.x + 8.0, hr.center().y), Align2::LEFT_CENTER, "Clipboard", theme::fid(14.0), theme::TEXT);
            if n > 0 {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(hr.shrink2(vec2(4.0, 4.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
                if pill(&mut child, "Clear all", false) {
                    actions.push(Action::ClipClear);
                }
            }
            ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], egui::Stroke::new(1.0, theme::LINE));

            if n == 0 {
                let (er, _) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::hover());
                ui.painter().text(er.center(), Align2::CENTER_CENTER, "Nothing copied yet", theme::fid(13.0), theme::DIM);
            } else {
                egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                    for (i, e) in hist.entries.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                        if resp.hovered() && ui.input(|i| i.pointer.delta() != vec2(0.0, 0.0)) {
                            sel = i;
                        }
                        let p = ui.painter();
                        if i == sel {
                            p.rect_filled(r, 4, theme::PLATE_SEL);
                            p.rect_filled(Rect::from_min_size(r.min, vec2(2.0, r.height())), 0, theme::accent());
                            if keyed {
                                ui.scroll_to_rect(r, None);
                            }
                        }
                        let p = ui.painter();
                        let mut job = LayoutJob::default();
                        job.append(&clipboard::preview(&e.text), 0.0, TextFormat { font_id: theme::fid(13.0), color: theme::TEXT, ..Default::default() });
                        job.wrap.max_width = r.width() - 110.0;
                        job.wrap.max_rows = 1;
                        job.wrap.break_anywhere = true;
                        let g = p.layout_job(job);
                        p.galley(pos2(r.min.x + 12.0, r.center().y - g.size().y / 2.0), g, theme::TEXT);
                        // Right side: how long ago, or the x on the hovered row.
                        let x = Rect::from_center_size(pos2(r.max.x - 16.0, r.center().y), vec2(20.0, 20.0));
                        let over_x = resp.hovered() && ui.input(|i| i.pointer.hover_pos()).is_some_and(|pos| x.contains(pos));
                        if resp.hovered() || i == sel {
                            ui.painter().text(x.center(), Align2::CENTER_CENTER, "x", theme::fid(13.0), if over_x { theme::accent() } else { theme::DIM });
                        } else {
                            ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, clipboard::age(e.at.elapsed()), theme::fid(11.0), theme::DIM);
                        }
                        if resp.clicked() {
                            actions.push(if over_x { Action::ClipDelete(i) } else { Action::ClipPick(i) });
                        }
                    }
                });
            }

            // Footer.
            ui.add_space(4.0);
            let (fr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
            ui.painter().line_segment([fr.left_top(), fr.right_top()], egui::Stroke::new(1.0, theme::LINE));
            ui.painter().text(pos2(fr.min.x + 8.0, fr.center().y + 2.0), Align2::LEFT_CENTER, "Up/Down move   Enter copy   Del remove   Esc close", theme::fid(11.0), theme::DIM);
            });
    ui.data_mut(|d| d.insert_temp(sel_id, sel));
    // Ages tick over slowly. Redraw now and then while it is open.
    ui.ctx().request_repaint_after(Duration::from_secs(30));
}

use std::time::Duration;
