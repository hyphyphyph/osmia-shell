//! Small shared widgets: bar items, dividers, hover plates.

use egui::{pos2, vec2, Align2, Response, Sense, Ui};

use crate::icons::{self, Icon};
use crate::theme;

/// A bar item: optional icon and text. Returns the click response.
pub fn item(ui: &mut Ui, icon: Option<(Icon, egui::Color32)>, text: &[(&str, egui::Color32)], sense: Sense) -> Response {
    let font = crate::theme::fid(12.0);
    let pad = 8.0;
    let gap = 5.0;
    // measure
    let galleys: Vec<_> = text
        .iter()
        .map(|(t, c)| ui.painter().layout_no_wrap((*t).to_owned(), font.clone(), *c))
        .collect();
    let mut w = pad * 2.0;
    if icon.is_some() {
        w += icons::SIZE;
    }
    for (i, g) in galleys.iter().enumerate() {
        if icon.is_some() || i > 0 {
            w += gap;
        }
        w += g.size().x;
    }
    let h = ui.available_height().max(22.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    if ui.is_rect_visible(rect) {
        if resp.hovered() && sense != Sense::hover() {
            let plate = egui::Rect::from_center_size(rect.center(), vec2(rect.width() - 2.0, 22.0));
            ui.painter().rect_filled(plate, 4.0, theme::PLATE);
        }
        let mut x = rect.min.x + pad;
        let cy = rect.center().y;
        if let Some((ic, col)) = icon {
            let r = egui::Rect::from_center_size(pos2(x + icons::SIZE / 2.0, cy), vec2(icons::SIZE, icons::SIZE));
            icons::draw(ui.painter(), ic, r, col);
            x += icons::SIZE + gap;
        }
        for g in galleys {
            let sz = g.size();
            ui.painter().galley(pos2(x, cy - sz.y / 2.0), g, theme::TEXT);
            x += sz.x + gap;
        }
    }
    resp
}

pub fn divider(ui: &mut Ui) {
    let h = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, h), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [pos2(x, rect.center().y - 7.0), pos2(x, rect.center().y + 7.0)],
        egui::Stroke::new(1.0, theme::LINE),
    );
}

#[allow(dead_code)]
pub fn label_at(ui: &Ui, pos: egui::Pos2, text: &str, color: egui::Color32) {
    ui.painter()
        .text(pos, Align2::LEFT_CENTER, text, crate::theme::fid(12.0), color);
}

/// A colored letter tile that stands in for an app icon.
pub fn tile(painter: &egui::Painter, center: egui::Pos2, size: f32, name: &str) {
    if let Some(tex) = crate::appicon::texture(painter.ctx(), name, size) {
        let r = egui::Rect::from_center_size(center, vec2(size, size));
        painter.image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        return;
    }
    const COLORS: [egui::Color32; 8] = [
        egui::Color32::from_rgb(0xe8, 0x70, 0x4a),
        egui::Color32::from_rgb(0x6a, 0x8f, 0xcb),
        egui::Color32::from_rgb(0x6a, 0xa8, 0x7a),
        egui::Color32::from_rgb(0xc5, 0x8b, 0xe0),
        egui::Color32::from_rgb(0xf0, 0xa3, 0x5e),
        egui::Color32::from_rgb(0x5b, 0xb3, 0xc4),
        egui::Color32::from_rgb(0xd2, 0x6f, 0x8e),
        egui::Color32::from_rgb(0x9a, 0xb8, 0x5a),
    ];
    let h = name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let color = COLORS[(h % 8) as usize];
    let r = egui::Rect::from_center_size(center, vec2(size, size));
    painter.rect_filled(r, (size * 0.2).min(4.0), color);
    let letter = name.chars().find(|c| c.is_alphanumeric()).unwrap_or('?').to_uppercase().to_string();
    painter.text(
        center,
        Align2::CENTER_CENTER,
        letter,
        crate::theme::fid(size * 0.58),
        egui::Color32::from_rgb(0x14, 0x16, 0x1a),
    );
}

pub fn age(since: std::time::Instant) -> String {
    let s = since.elapsed().as_secs();
    match s {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m", s / 60),
        3600..=86399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

/// The floating card frame used by every panel, popup, and the launcher.
pub fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::CARD)
        .stroke(egui::Stroke::new(1.0, theme::CARD_LINE))
        .corner_radius(4)
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: egui::Color32::from_black_alpha(130),
        })
}

/// A small pill button. Returns true when clicked.
pub fn pill(ui: &mut Ui, text: &str, accent: bool) -> bool {
    let font = crate::theme::fid(11.0);
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        font,
        if accent { egui::Color32::from_rgb(0x14, 0x16, 0x1a) } else { theme::TEXT },
    );
    let size = galley.size() + vec2(20.0, 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if accent {
        theme::accent()
    } else if resp.hovered() {
        theme::PLATE_SEL
    } else {
        theme::BTN
    };
    ui.painter().rect(rect, 4, fill, egui::Stroke::new(1.0, theme::CARD_LINE), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme::TEXT);
    resp.clicked()
}

/// The do-not-disturb switch.
pub fn switch(ui: &mut Ui, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 16.0), Sense::click());
    ui.painter().rect_filled(rect, 4, if on { theme::accent() } else { theme::CARD_LINE });
    let x = if on { rect.max.x - 8.0 } else { rect.min.x + 8.0 };
    ui.painter().rect_filled(
        egui::Rect::from_center_size(pos2(x, rect.center().y), vec2(12.0, 12.0)),
        3,
        if on { egui::Color32::from_rgb(0x14, 0x16, 0x1a) } else { egui::Color32::from_rgb(0xae, 0xb4, 0xbd) },
    );
    resp.clicked()
}

/// A stable widget id from any hashable value.
pub fn id(source: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
    #[allow(deprecated)]
    egui::Id::new(source)
}
