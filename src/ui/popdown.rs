//! The floating card every popdown is built on: the bar's panels, the launcher,
//! the clipboard and emoji pickers, and the capture toolbar.
//!
//! Why this exists: an egui Area hands its contents last frame's size as their
//! room (`Area::content_ui` uses `state.rect()`). A card whose contents grow, such
//! as a list that gains a row, can then only grow by what spills past that room
//! each frame. Frames only run on input once nothing animates, so the card crept
//! a few pixels taller with every mouse move. See spec/Bar.pi, popdown.
//!
//! A popdown instead gives its contents a fixed room: its width by the most
//! height it may take, worked out from the screen, never from last frame. And it
//! keeps drawing frames until its size stops changing, so it settles at once
//! without waiting for input.

use egui::{vec2, Align2, Context, InnerResponse, Margin, Pos2, Rect, Ui, UiBuilder};

use crate::ui::widgets::card_frame;

pub struct Popdown {
    id: egui::Id,
    pos: Pos2,
    pivot: Align2,
    /// The width of the contents. None lets the contents set it.
    width: Option<f32>,
    /// The most height the whole card may take.
    max_height: f32,
    margin: Margin,
}

impl Popdown {
    /// A card whose `pivot` corner sits at `pos`.
    pub fn new(id: egui::Id, pos: Pos2, pivot: Align2) -> Self {
        Popdown { id, pos, pivot, width: None, max_height: 4000.0, margin: Margin::ZERO }
    }

    /// The width of the contents, not counting the margin and border.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w.max(1.0));
        self
    }

    /// The most height the card may take. Contents that need more must scroll.
    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = h.max(1.0);
        self
    }

    pub fn margin(mut self, m: Margin) -> Self {
        self.margin = m;
        self
    }

    pub fn show<R>(self, ctx: &Context, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let Popdown { id, pos, pivot, width, max_height, margin } = self;
        egui::Area::new(id).fixed_pos(pos).pivot(pivot).order(egui::Order::Foreground).show(ctx, |ui| {
            // The room comes from the screen, not from the area's last size.
            let screen = ctx.content_rect();
            let chrome = vec2(margin.sum().x + 2.0, margin.sum().y + 2.0);
            let room_w = width.map(|w| w + chrome.x).unwrap_or(screen.width());
            let room = Rect::from_min_size(ui.max_rect().min, vec2(room_w, max_height));
            let mut child = ui.new_child(UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
            let inner = card_frame()
                .inner_margin(margin)
                .show(&mut child, |ui| {
                    if let Some(w) = width {
                        ui.set_width(w);
                    }
                    ui.set_max_height((max_height - chrome.y).max(1.0));
                    add(ui)
                })
                .inner;
            let used = child.min_rect();
            ui.advance_cursor_after_rect(used);

            // Draw again until the size holds still, so a change shows in full at once.
            let key = id.with("popdown-size");
            let size = used.size();
            let prev: Option<egui::Vec2> = ctx.data(|d| d.get_temp(key));
            if prev.is_none_or(|p| (p - size).length() > 0.25) {
                ctx.data_mut(|d| d.insert_temp(key, size));
                ctx.request_repaint();
            }
            inner
        })
    }
}
