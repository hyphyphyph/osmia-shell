//! The on-screen display for volume, brightness and keyboard light changes:
//! a small card near the bottom of the screen with an icon, a level bar and the value.

use egui::{pos2, vec2, Align2, Rect, Ui};

use crate::icons::{self, Icon};
use crate::theme;
use crate::ui::widgets::card_frame;

/// The card. The surface is larger by `PAD` on every side, room for the shadow.
pub const CARD_W: f32 = 280.0;
pub const CARD_H: f32 = 52.0;
pub const PAD: f32 = 24.0;
/// The gap between the card and the bottom of the usable screen.
pub const BOTTOM_GAP: i32 = 72;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum What {
    Volume,
    Brightness,
    KeyboardLight,
}

/// What one OSD shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Level {
    /// 0 to 100.
    Percent(u32),
    /// A few steps, such as 2 of 3.
    Steps(u32, u32),
}

pub struct View {
    pub what: What,
    pub level: Level,
    pub muted: bool,
}

pub fn surface_size() -> (u32, u32) {
    ((CARD_W + PAD * 2.0) as u32, (CARD_H + PAD * 2.0) as u32)
}

pub fn show(ui: &mut Ui, v: &View) {
    let full = ui.max_rect();
    let card = Rect::from_center_size(full.center(), vec2(CARD_W, CARD_H));
    let p = ui.painter().clone();
    let frame = card_frame();
    p.add(frame.shadow.as_shape(card, frame.corner_radius));
    p.rect(card, frame.corner_radius, frame.fill, frame.stroke, egui::StrokeKind::Inside);

    let icon = match v.what {
        What::Volume if v.muted => Icon::VolumeMuted,
        What::Volume => Icon::Volume,
        What::Brightness | What::KeyboardLight => Icon::Sun,
    };
    let icon_size = if v.what == What::KeyboardLight { 14.0 } else { 20.0 };
    let ic = Rect::from_center_size(pos2(card.min.x + 28.0, card.center().y), vec2(icon_size, icon_size));
    icons::draw(&p, icon, ic, if v.muted { theme::DIM } else { theme::TEXT });

    // Muted still shows the level, so a volume key while muted shows where it will be.
    let text = match v.level {
        Level::Percent(n) => format!("{n}%"),
        Level::Steps(0, _) => "Off".to_owned(),
        Level::Steps(n, max) => format!("{n}/{max}"),
    };
    let text_right = card.max.x - 16.0;
    p.text(pos2(text_right, card.center().y), Align2::RIGHT_CENTER, &text, theme::fid(13.0), if v.muted { theme::DIM } else { theme::TEXT });

    // The level bar runs from after the icon to before the value.
    let track = Rect::from_min_max(pos2(card.min.x + 52.0, card.center().y - 3.0), pos2(text_right - 52.0, card.center().y + 3.0));
    let fill = if v.muted { theme::DIM } else { theme::accent() };
    match v.level {
        Level::Percent(n) => {
            p.rect_filled(track, 3, theme::LINE);
            let frac = (n.min(100) as f32) / 100.0;
            if frac > 0.0 {
                let r = Rect::from_min_max(track.min, pos2(track.min.x + track.width() * frac, track.max.y));
                p.rect_filled(r, 3, fill);
            }
        }
        Level::Steps(n, max) => {
            // One segment per step.
            let max = max.max(1);
            let gap = 4.0;
            let w = (track.width() - gap * (max - 1) as f32) / max as f32;
            for i in 0..max {
                let x = track.min.x + i as f32 * (w + gap);
                let r = Rect::from_min_size(pos2(x, track.min.y), vec2(w, track.height()));
                p.rect_filled(r, 3, if i < n { fill } else { theme::LINE });
            }
        }
    }
}
