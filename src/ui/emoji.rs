//! The emoji picker card: a search box, group tabs, a grid of emoji, and a footer
//! with the marked emoji and the skin tone choice.

use egui::{pos2, vec2, Align2, Color32, FontId, Key, Rect, Sense, Ui};

use crate::emoji::{self, Memory};
use crate::icons::{self, Icon};
use crate::theme;
use crate::ui::widgets::id;
use crate::ui::Action;

const COLS: usize = 10;
const CELL: f32 = 40.0;
const HEADER_H: f32 = 24.0;
const TAB_H: f32 = 32.0;
/// One emoji for each group tab, in the order of the groups in the list.
const GROUP_TABS: [&str; 9] = ["😀", "👋", "🐻", "🍔", "🚗", "⚽", "💡", "💕", "🏁"];
const RECENT_TAB: &str = "🕘";
const TONE_HAND: [&str; 6] = ["✋", "✋🏻", "✋🏼", "✋🏽", "✋🏾", "✋🏿"];

#[derive(Default)]
pub struct State {
    pub query: String,
    /// The marked cell, as a place in the grid.
    pub sel: usize,
    /// Set when it opens, so the grid starts at the top.
    pub fresh: bool,
}

/// One line of the grid: a group name, or a row of cells (a range of places).
enum Line {
    Header(&'static str, Tab),
    Row(usize, usize),
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Recent,
    Group(usize),
}

fn emoji_font(size: f32) -> FontId {
    FontId::new(size, theme::emoji_family())
}

/// The grid: every place (an index into the emoji list) and the lines that show them.
fn layout(list: &emoji::List, mem: &Memory, query: &str) -> (Vec<usize>, Vec<Line>) {
    let mut places = Vec::new();
    let mut lines = Vec::new();
    let section = |places: &mut Vec<usize>, lines: &mut Vec<Line>, items: &mut dyn Iterator<Item = usize>| {
        let start = places.len();
        places.extend(items);
        let mut s = start;
        while s < places.len() {
            let e = (s + COLS).min(places.len());
            lines.push(Line::Row(s, e));
            s = e;
        }
    };
    if !query.trim().is_empty() {
        section(&mut places, &mut lines, &mut emoji::search(list, query).into_iter());
        return (places, lines);
    }
    if !mem.recent.is_empty() {
        lines.push(Line::Header("Recent", Tab::Recent));
        section(&mut places, &mut lines, &mut mem.recent.iter().copied());
    }
    for (g, name) in list.groups.iter().enumerate() {
        lines.push(Line::Header(name, Tab::Group(g)));
        section(&mut places, &mut lines, &mut list.emoji.iter().enumerate().filter(|(_, e)| e.group == g).map(|(i, _)| i));
    }
    (places, lines)
}

fn line_h(l: &Line) -> f32 {
    match l {
        Line::Header(..) => HEADER_H,
        Line::Row(..) => CELL,
    }
}

/// The place one row up or down from `sel`, in the same column or the last cell of a shorter row.
fn step_row(lines: &[Line], sel: usize, down: bool) -> usize {
    let rows: Vec<(usize, usize)> = lines.iter().filter_map(|l| if let Line::Row(s, e) = l { Some((*s, *e)) } else { None }).collect();
    let Some(r) = rows.iter().position(|(s, e)| (*s..*e).contains(&sel)) else { return sel };
    let col = sel - rows[r].0;
    let to = if down { r + 1 } else { r.wrapping_sub(1) };
    match rows.get(to) {
        Some((s, e)) => (s + col).min(e - 1),
        None => sel,
    }
}

pub fn show(ui: &mut Ui, st: &mut State, mem: &Memory, actions: &mut Vec<Action>) {
    let full = ui.max_rect();
    let list = emoji::list();
    let tone = mem.tone;

    // A click on the empty screen closes. The card is added after, so it wins.
    let bg = ui.interact(full, id("emoji-bg"), Sense::click());
    ui.painter().rect_filled(full, 0, Color32::from_black_alpha(205));
    if bg.clicked() {
        actions.push(Action::CloseOverlay);
    }

    // Keys first, so the search box does not see them.
    let (mut left, mut right, mut up, mut down, mut enter, mut esc) = (false, false, false, false, false, false);
    ui.input_mut(|i| {
        esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
        enter = i.consume_key(egui::Modifiers::NONE, Key::Enter);
        left = i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft);
        right = i.consume_key(egui::Modifiers::NONE, Key::ArrowRight);
        up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
        down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
    });
    if esc {
        // The first Escape clears what was typed. The next closes.
        if st.query.is_empty() {
            actions.push(Action::CloseOverlay);
        } else {
            st.query.clear();
            st.sel = 0;
            st.fresh = true;
        }
    }

    let (places, lines) = layout(list, mem, &st.query);
    let n = places.len();
    if n > 0 {
        st.sel = st.sel.min(n - 1);
        if left {
            st.sel = st.sel.saturating_sub(1);
        }
        if right {
            st.sel = (st.sel + 1).min(n - 1);
        }
        if up || down {
            st.sel = step_row(&lines, st.sel, down);
        }
        if enter {
            actions.push(Action::EmojiPick(places[st.sel]));
        }
    }
    let keyed = left || right || up || down;
    let total_h: f32 = lines.iter().map(line_h).sum();
    let list_h = (full.height() - 36.0 - 190.0).clamp(CELL * 3.0, CELL * 10.0).min(total_h.max(80.0));
    let width = COLS as f32 * CELL;

    crate::ui::popdown::Popdown::new(id("emoji"), pos2(full.center().x, 36.0), Align2::CENTER_TOP)
        .margin(egui::Margin::same(6))
        .show(ui.ctx(), |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 0.0;

            // Search box.
            let before = st.query.clone();
            let (sb, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
            icons::draw(ui.painter(), Icon::Search, Rect::from_center_size(pos2(sb.min.x + 16.0, sb.center().y), vec2(16.0, 16.0)), theme::DIM);
            let edit_rect = Rect::from_min_max(pos2(sb.min.x + 34.0, sb.min.y + 7.0), pos2(sb.max.x - 8.0, sb.max.y - 1.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            let te = egui::TextEdit::singleline(&mut st.query)
                .frame(egui::Frame::NONE)
                .desired_width(edit_rect.width())
                .font(theme::fid(15.0))
                .hint_text("Search emoji");
            child.add(te).request_focus();
            if st.query != before {
                st.sel = 0;
                st.fresh = true;
                ui.ctx().request_repaint();
            }
            ui.painter().line_segment([sb.left_bottom(), sb.right_bottom()], egui::Stroke::new(1.0, theme::LINE));

            // Group tabs. Which one is marked is known only after the grid has scrolled,
            // so the tabs are drawn after the grid, into this space.
            let (tabs_rect, _) = ui.allocate_exact_size(vec2(width, TAB_H), Sense::hover());
            let mut tabs: Vec<Tab> = Vec::new();
            if !mem.recent.is_empty() {
                tabs.push(Tab::Recent);
            }
            tabs.extend((0..list.groups.len().min(GROUP_TABS.len())).map(Tab::Group));
            let tab_w = width / tabs.len().max(1) as f32;
            let tab_rects: Vec<Rect> = (0..tabs.len())
                .map(|i| Rect::from_min_size(pos2(tabs_rect.min.x + i as f32 * tab_w, tabs_rect.min.y), vec2(tab_w, TAB_H)))
                .collect();
            let mut jump: Option<Tab> = None;
            for (i, r) in tab_rects.iter().enumerate() {
                if ui.interact(*r, id(("emoji-tab", i)), Sense::click()).clicked() {
                    jump = Some(tabs[i]);
                }
            }
            if jump.is_some() && !st.query.is_empty() {
                // A tab shows the whole list again.
                st.query.clear();
                st.fresh = true;
                ui.ctx().request_repaint();
            }
            ui.painter().line_segment([tabs_rect.left_bottom(), tabs_rect.right_bottom()], egui::Stroke::new(1.0, theme::LINE));

            // The grid.
            let mut in_view: Option<Tab> = None;
            if n == 0 {
                let (er, _) = ui.allocate_exact_size(vec2(width, 80.0), Sense::hover());
                ui.painter().text(er.center(), Align2::CENTER_CENTER, "No emoji found", theme::fid(13.0), theme::DIM);
            } else {
                let mut sa = egui::ScrollArea::vertical().id_salt("emoji-grid").max_height(list_h).auto_shrink([false, true]);
                if st.fresh {
                    sa = sa.vertical_scroll_offset(0.0);
                    st.fresh = false;
                }
                if let Some(t) = jump.filter(|_| st.query.is_empty()) {
                    let y: f32 = lines.iter().take_while(|l| !matches!(l, Line::Header(_, h) if *h == t)).map(line_h).sum();
                    sa = sa.vertical_scroll_offset(y);
                }
                sa.show_viewport(ui, |ui, viewport| {
                    ui.set_height(total_h);
                    let origin = ui.min_rect().min;
                    let pointer = ui.input(|i| i.pointer.hover_pos());
                    let moved = ui.input(|i| i.pointer.delta() != vec2(0.0, 0.0));
                    let mut y = 0.0;
                    let mut header: Option<Tab> = None;
                    for line in &lines {
                        let h = line_h(line);
                        if let Line::Header(_, t) = line {
                            header = Some(*t);
                        }
                        // The group whose rows are at the top of the view.
                        if y <= viewport.min.y + 1.0 {
                            in_view = header;
                        }
                        let visible = y + h >= viewport.min.y && y <= viewport.max.y;
                        let line_rect = Rect::from_min_size(origin + vec2(0.0, y), vec2(width, h));
                        match line {
                            Line::Header(name, _) if visible => {
                                ui.painter().text(pos2(line_rect.min.x + 6.0, line_rect.center().y + 2.0), Align2::LEFT_CENTER, *name, theme::fid(11.0), theme::DIM);
                            }
                            Line::Row(s, e) => {
                                for p in *s..*e {
                                    let r = Rect::from_min_size(line_rect.min + vec2((p - s) as f32 * CELL, 0.0), vec2(CELL, CELL));
                                    if p == st.sel && keyed {
                                        ui.scroll_to_rect(r, None);
                                    }
                                    if !visible {
                                        continue;
                                    }
                                    let resp = ui.interact(r, id(("emoji-cell", p)), Sense::click());
                                    if moved && pointer.is_some_and(|pos| r.contains(pos)) && resp.hovered() {
                                        st.sel = p;
                                    }
                                    if p == st.sel {
                                        let plate = r.shrink(2.0);
                                        ui.painter().rect_filled(plate, 6, theme::PLATE_SEL);
                                        ui.painter().rect_stroke(plate, 6, egui::Stroke::new(1.5, theme::accent()), egui::StrokeKind::Inside);
                                    }
                                    let e = &list.emoji[places[p]];
                                    ui.painter().text(r.center(), Align2::CENTER_CENTER, e.with_tone(tone), emoji_font(26.0), Color32::WHITE);
                                    if resp.clicked() {
                                        actions.push(Action::EmojiPick(places[p]));
                                    }
                                }
                            }
                            _ => {}
                        }
                        y += h;
                    }
                });
            }

            // Now the tabs, with the one in view marked.
            for (i, r) in tab_rects.iter().enumerate() {
                let t = tabs[i];
                let on = st.query.is_empty() && in_view == Some(t);
                let hovered = ui.input(|inp| inp.pointer.hover_pos()).is_some_and(|p| r.contains(p));
                if on || hovered {
                    ui.painter().rect_filled(r.shrink2(vec2(3.0, 3.0)), 5, if on { theme::accent_bg() } else { theme::PLATE });
                }
                if on {
                    let b = Rect::from_min_max(pos2(r.min.x + 8.0, r.max.y - 2.0), pos2(r.max.x - 8.0, r.max.y));
                    ui.painter().rect_filled(b, 1, theme::accent());
                }
                let glyph = match t {
                    Tab::Recent => RECENT_TAB,
                    Tab::Group(g) => GROUP_TABS[g],
                };
                ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph, emoji_font(18.0), Color32::WHITE);
            }

            // Footer: the marked emoji and its name, and the skin tones on the right.
            let (fr, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
            ui.painter().line_segment([fr.left_top(), fr.right_top()], egui::Stroke::new(1.0, theme::LINE));
            let tones_w = 6.0 * 26.0;
            if let Some(&i) = places.get(st.sel) {
                let e = &list.emoji[i];
                ui.painter().text(pos2(fr.min.x + 22.0, fr.center().y + 1.0), Align2::CENTER_CENTER, e.with_tone(tone), emoji_font(24.0), Color32::WHITE);
                let mut name = e.name.to_owned();
                if let Some(c) = name.get(0..1) {
                    name.replace_range(0..1, &c.to_uppercase());
                }
                let mut job = egui::text::LayoutJob::single_section(name, egui::TextFormat { font_id: theme::fid(12.0), color: theme::TEXT, ..Default::default() });
                job.wrap.max_width = fr.width() - 52.0 - tones_w;
                job.wrap.max_rows = 1;
                let g = ui.painter().layout_job(job);
                ui.painter().galley(pos2(fr.min.x + 44.0, fr.center().y + 1.0 - g.size().y / 2.0), g, theme::TEXT);
            }
            for (t, hand) in TONE_HAND.iter().enumerate() {
                let r = Rect::from_center_size(pos2(fr.max.x - tones_w + 13.0 + t as f32 * 26.0, fr.center().y + 1.0), vec2(24.0, 24.0));
                let choice = t.checked_sub(1);
                let resp = ui.interact(r, id(("emoji-tone", t)), Sense::click());
                if choice == tone {
                    ui.painter().rect_stroke(r, 5, egui::Stroke::new(1.5, theme::accent()), egui::StrokeKind::Inside);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 5, theme::PLATE);
                }
                ui.painter().text(r.center(), Align2::CENTER_CENTER, *hand, emoji_font(16.0), Color32::WHITE);
                if resp.clicked() {
                    actions.push(Action::EmojiTone(choice));
                }
            }
            });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_step_by_column_and_clamp_to_short_rows() {
        let lines = vec![Line::Header("A", Tab::Group(0)), Line::Row(0, 10), Line::Row(10, 13), Line::Header("B", Tab::Group(1)), Line::Row(13, 23)];
        assert_eq!(step_row(&lines, 7, true), 12);
        assert_eq!(step_row(&lines, 12, true), 15);
        assert_eq!(step_row(&lines, 15, false), 12);
        assert_eq!(step_row(&lines, 3, false), 3);
        assert_eq!(step_row(&lines, 20, true), 20);
    }

    #[test]
    fn group_tabs_match_the_groups() {
        assert_eq!(emoji::list().groups.len(), GROUP_TABS.len());
    }
}
