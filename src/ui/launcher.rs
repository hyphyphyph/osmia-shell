//! The launcher card. Compact rows, Alt+number shortcuts, a Calculator chip,
//! and a footer that lists the keys and prefixes (Launcher.mockups round 1, variation 3).

use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Key, Modifiers, Rect, Sense, Ui};

use crate::icons::{self, Icon};
use crate::launch::{self, Data, Item, Kind, Source, WinInfo};
use crate::theme;
use crate::ui::widgets::{card_frame, tile};
use crate::ui::{Action, Overlay, PowerAction};

#[derive(Default)]
pub struct State {
    pub query: String,
    pub selected: usize,
}

const ROW_H: f32 = 28.0;

pub fn show(ui: &mut Ui, st: &mut State, data: &Data, windows: &[WinInfo], width: f32, max_rows: usize, actions: &mut Vec<Action>) {
    let full = ui.max_rect();
    let bg = ui.interact(full, crate::ui::widgets::id("launcher-bg"), Sense::click());
    if bg.clicked() {
        actions.push(Action::CloseOverlay);
    }

    // Keys first, so the text box does not see them.
    let (down, up, enter, esc) = ui.input_mut(|i| {
        let down = i.consume_key(Modifiers::NONE, Key::ArrowDown) || i.consume_key(Modifiers::CTRL, Key::N);
        let up = i.consume_key(Modifiers::NONE, Key::ArrowUp) || i.consume_key(Modifiers::CTRL, Key::P);
        let enter = i.consume_key(Modifiers::NONE, Key::Enter);
        let esc = i.consume_key(Modifiers::NONE, Key::Escape);
        (down, up, enter, esc)
    });
    let mut alt_pick = None;
    ui.input_mut(|i| {
        for (n, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4].into_iter().enumerate() {
            if i.consume_key(Modifiers::ALT, key) {
                alt_pick = Some(n);
            }
        }
    });
    if esc {
        actions.push(Action::CloseOverlay);
    }

    let before = st.query.clone();
    egui::Area::new(crate::ui::widgets::id("launcher"))
        .fixed_pos(pos2(full.center().x, full.height() * 0.18))
        .pivot(Align2::CENTER_TOP)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            card_frame().inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                ui.set_width(width - 12.0);
                // Search box.
                let (sb, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
                icons::draw(ui.painter(), Icon::Search, Rect::from_center_size(pos2(sb.min.x + 18.0, sb.center().y), vec2(16.0, 16.0)), theme::DIM);
                let calc_active = crate::calc::answer(st.query.trim()).is_some();
                let mut x = sb.min.x + 36.0;
                if calc_active {
                    let r = Rect::from_min_size(pos2(x, sb.center().y - 10.0), vec2(74.0, 20.0));
                    ui.painter().rect(r, 4, theme::BTN, egui::Stroke::new(1.0, theme::accent()), egui::StrokeKind::Inside);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "Calculator", crate::theme::fid(11.0), theme::accent());
                    x += 82.0;
                }
                let edit_rect = Rect::from_min_max(pos2(x, sb.min.y + 4.0), pos2(sb.max.x - 8.0, sb.max.y - 4.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                let te = egui::TextEdit::singleline(&mut st.query)
                    .frame(egui::Frame::NONE)
                    .desired_width(edit_rect.width())
                    .font(crate::theme::fid(15.0))
                    .hint_text("Search apps, windows, actions");
                let resp = child.add(te);
                resp.request_focus();
                ui.painter().line_segment([sb.left_bottom(), sb.right_bottom()], egui::Stroke::new(1.0, theme::LINE));

                if st.query != before {
                    st.selected = 0;
                }
                let items = launch::search(data, windows, &st.query, 24);
                if !items.is_empty() {
                    st.selected = st.selected.min(items.len() - 1);
                }
                if down && st.selected + 1 < items.len() {
                    st.selected += 1;
                }
                if up && st.selected > 0 {
                    st.selected -= 1;
                }
                let offset = st.selected.saturating_sub(max_rows - 1);
                let mut clicked = None;
                if let Some(first) = items.first().filter(|i| i.source == Source::Calc) {
                    // The calculator answer gets a tall row of its own.
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::click());
                    ui.painter().rect(r, 4, theme::PLATE_SEL, egui::Stroke::NONE, egui::StrokeKind::Inside);
                    ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(2.0, r.height())), 0, theme::accent());
                    ui.painter().text(pos2(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, format!("= {}", first.name), crate::theme::fid(28.0), theme::accent());
                    ui.painter().text(pos2(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, "Enter copies and closes", crate::theme::fid(11.0), theme::DIM);
                    if resp.clicked() {
                        clicked = Some(0);
                    }
                }
                for (idx, item) in items.iter().enumerate().skip(offset).take(max_rows) {
                    if item.source == Source::Calc {
                        continue;
                    }
                    let shortcut = non_calc_rank(&items, idx);
                    if let Some(c) = row(ui, item, windows, idx == st.selected, idx, shortcut) {
                        clicked = Some(c);
                    }
                }
                // Footer.
                ui.add_space(4.0);
                let (fr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
                ui.painter().line_segment([fr.left_top(), fr.right_top()], egui::Stroke::new(1.0, theme::LINE));
                ui.painter().text(pos2(fr.min.x + 8.0, fr.center().y + 2.0), Align2::LEFT_CENTER, "Up/Down move   Enter run   Esc close", crate::theme::fid(11.0), theme::DIM);
                ui.painter().text(pos2(fr.max.x - 8.0, fr.center().y + 2.0), Align2::RIGHT_CENTER, "> command   @ window   : action", crate::theme::fid(11.0), theme::DIM);

                let non_calc: Vec<usize> = items.iter().enumerate().filter(|(_, i)| i.source != Source::Calc).map(|(i, _)| i).collect();
                let alt_target = alt_pick.and_then(|n| non_calc.get(n).copied());
                let target = if enter { Some(st.selected) } else { alt_target.or(clicked) };
                if let Some(i) = target {
                    if let Some(item) = items.get(i) {
                        actions.push(run_action(item));
                    }
                }
            });
        });
}

fn non_calc_rank(items: &[Item], idx: usize) -> Option<usize> {
    let rank = items.iter().take(idx).filter(|i| i.source != Source::Calc).count();
    (rank < 4).then_some(rank)
}

fn row(ui: &mut Ui, item: &Item, windows: &[crate::launch::WinInfo], selected: bool, idx: usize, shortcut: Option<usize>) -> Option<usize> {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let p = ui.painter();
    if selected || resp.hovered() {
        p.rect_filled(r, 4, if selected { theme::PLATE_SEL } else { theme::PLATE });
    }
    if selected {
        p.rect_filled(Rect::from_min_size(r.min, vec2(2.0, r.height())), 0, theme::accent());
    }
    let key = match &item.kind {
        Kind::App(a) => format!("{}\n{}\n{}", a.id, a.name, a.exec.split_whitespace().next().unwrap_or("")),
        Kind::Window(i) => windows.iter().find(|w| w.index == *i).map(|w| format!("{}\n{}", w.app_id, w.title)).unwrap_or_default(),
        Kind::Action(_) | Kind::Calc(_) | Kind::Command(_) => String::new(),
    };
    tile(p, pos2(r.min.x + 20.0, r.center().y), 16.0, if key.is_empty() { &item.name } else { &key });
    // Name with matched letters in the accent color.
    let mut job = LayoutJob::default();
    for (i, ch) in item.name.chars().enumerate() {
        let matched = item.matches.contains(&i);
        job.append(
            &ch.to_string(),
            0.0,
            TextFormat {
                font_id: crate::theme::fid(13.0),
                color: if matched { theme::accent() } else { theme::TEXT },
                ..Default::default()
            },
        );
    }
    job.wrap.max_width = r.width() - 190.0;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let g = p.layout_job(job);
    p.galley(pos2(r.min.x + 36.0, r.center().y - g.size().y / 2.0), g, theme::TEXT);
    // Right side: source label and the Alt+number chip.
    let mut rx = r.max.x - 10.0;
    if let Some(n) = shortcut {
        let chip = Rect::from_center_size(pos2(rx - 22.0, r.center().y), vec2(44.0, 18.0));
        p.rect(chip, 4, theme::BTN, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
        p.text(chip.center(), Align2::CENTER_CENTER, format!("Alt+{}", n + 1), crate::theme::fid(10.0), theme::DIM);
        rx -= 52.0;
    }
    p.text(pos2(rx, r.center().y), Align2::RIGHT_CENTER, item.source.label(), crate::theme::fid(11.0), theme::DIM);
    resp.clicked().then_some(idx)
}

fn run_action(item: &Item) -> Action {
    match &item.kind {
        Kind::App(a) => Action::RunApp(a.id.clone()),
        Kind::Window(i) => Action::FocusWindow(*i),
        Kind::Action(a) => {
            if a.needs_confirm() {
                Action::ToggleOverlay(Overlay::Power(Some(*a)), f32::NAN)
            } else {
                Action::Power(*a)
            }
        }
        Kind::Command(c) => Action::RunCommand(c.clone()),
        Kind::Calc(a) => Action::CopyAndClose(a.clone()),
    }
}

#[allow(dead_code)]
fn _unused(_: PowerAction) {}
