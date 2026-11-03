//! Small line icons, drawn with egui shapes so no icon font is needed.

use egui::{epaint::PathStroke, pos2, vec2, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Battery,
    BatteryCharging,
    Wifi,
    Volume,
    VolumeMuted,
    Sun,
    Bell,
    Windows,
    BellFilled,
    BellOff,
    Power,
    Search,
    Lock,
    Check,
    Chevron,
    Close,
    Gauge,
    Gear,
    Camera,
    Play,
    Pause,
    Next,
    Prev,
}

pub const SIZE: f32 = 14.0;

pub fn draw(p: &Painter, icon: Icon, rect: Rect, color: egui::Color32) {
    let s = Stroke::new(1.4, color);
    let c = rect.center();
    let u = rect.width() / 16.0; // icon grid is 16x16
    let pt = |x: f32, y: f32| pos2(rect.min.x + x * u, rect.min.y + y * u);
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], s);
    let path = |pts: Vec<Pos2>| {
        p.add(Shape::line(pts, PathStroke::new(1.4, color)));
    };
    match icon {
        Icon::Battery | Icon::BatteryCharging => {
            p.rect_stroke(
                Rect::from_min_max(pt(1.5, 4.5), pt(12.5, 11.5)),
                2.0 * u,
                s,
                egui::StrokeKind::Inside,
            );
            line(pt(14.0, 7.0), pt(14.0, 9.0));
            if icon == Icon::BatteryCharging {
                path(vec![pt(7.5, 5.5), pt(5.5, 8.2), pt(8.5, 8.2), pt(6.5, 10.8)]);
            } else {
                line(pt(4.0, 7.0), pt(4.0, 9.0));
                line(pt(6.5, 7.0), pt(6.5, 9.0));
            }
        }
        Icon::Wifi => {
            arc(p, pt(8.0, 13.5), 11.0 * u, -2.35, -0.79, s);
            arc(p, pt(8.0, 13.5), 7.0 * u, -2.35, -0.79, s);
            p.circle_filled(pt(8.0, 12.0), 0.9 * u, color);
        }
        Icon::Volume | Icon::VolumeMuted => {
            path(vec![
                pt(2.0, 6.0),
                pt(4.5, 6.0),
                pt(8.0, 3.0),
                pt(8.0, 13.0),
                pt(4.5, 10.0),
                pt(2.0, 10.0),
                pt(2.0, 6.0),
            ]);
            if icon == Icon::Volume {
                arc(p, pt(8.0, 8.0), 4.0 * u, -0.8, 0.8, s);
            } else {
                line(pt(10.5, 6.0), pt(14.0, 10.0));
                line(pt(14.0, 6.0), pt(10.5, 10.0));
            }
        }
        Icon::Sun => {
            p.circle_stroke(pt(8.0, 8.0), 2.5 * u, s);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([pt(8.0, 8.0) + d * 4.6 * u, pt(8.0, 8.0) + d * 6.4 * u], s);
            }
        }
        Icon::BellFilled => {
            let mut pts = vec![];
            for k in 0..=12 {
                let a = std::f32::consts::PI + std::f32::consts::PI * k as f32 / 12.0;
                pts.push(pt(8.0 + 4.0 * a.cos(), 7.0 + 4.0 * a.sin()));
            }
            pts.extend([pt(12.0, 11.0), pt(13.0, 12.5), pt(3.0, 12.5), pt(4.0, 11.0)]);
            p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
            p.rect_filled(Rect::from_min_max(pt(6.2, 13.2), pt(9.8, 14.8)), 1, color);
        }
        Icon::Bell | Icon::BellOff => {
            arc(p, pt(8.0, 7.0), 4.0 * u, std::f32::consts::PI, std::f32::consts::TAU, s);
            path(vec![pt(4.0, 7.0), pt(4.0, 11.0), pt(3.0, 12.0), pt(13.0, 12.0), pt(12.0, 11.0), pt(12.0, 7.0)]);
            line(pt(6.5, 14.0), pt(9.5, 14.0));
            if icon == Icon::BellOff {
                line(pt(2.0, 2.0), pt(14.0, 14.0));
            }
        }
        Icon::Power => {
            line(pt(8.0, 2.0), pt(8.0, 7.0));
            // open circle, gap at the top
            let a0 = -std::f32::consts::FRAC_PI_2 + 0.75;
            let a1 = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU - 0.75;
            arc(p, pt(8.0, 8.5), 5.2 * u, a0, a1, s);
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![pt(4.5, 2.5), pt(13.0, 8.0), pt(4.5, 13.5)], color, Stroke::NONE));
        }
        Icon::Pause => {
            p.rect_filled(Rect::from_min_max(pt(3.5, 2.5), pt(6.5, 13.5)), 0, color);
            p.rect_filled(Rect::from_min_max(pt(9.5, 2.5), pt(12.5, 13.5)), 0, color);
        }
        Icon::Next => {
            p.add(Shape::convex_polygon(vec![pt(3.0, 3.0), pt(11.0, 8.0), pt(3.0, 13.0)], color, Stroke::NONE));
            p.rect_filled(Rect::from_min_max(pt(11.5, 3.0), pt(13.5, 13.0)), 0, color);
        }
        Icon::Prev => {
            p.add(Shape::convex_polygon(vec![pt(13.0, 3.0), pt(5.0, 8.0), pt(13.0, 13.0)], color, Stroke::NONE));
            p.rect_filled(Rect::from_min_max(pt(2.5, 3.0), pt(4.5, 13.0)), 0, color);
        }
        Icon::Camera => {
            p.rect_stroke(Rect::from_min_max(pt(1.5, 5.0), pt(14.5, 13.0)), 1.5 * u, s, egui::StrokeKind::Inside);
            path(vec![pt(5.5, 5.0), pt(6.5, 3.0), pt(9.5, 3.0), pt(10.5, 5.0)]);
            p.circle_stroke(pt(8.0, 9.0), 2.4 * u, s);
        }
        Icon::Gear => {
            p.circle_stroke(pt(8.0, 8.0), 4.2 * u, s);
            p.circle_stroke(pt(8.0, 8.0), 1.6 * u, s);
            for k in 0..6 {
                let a = k as f32 * std::f32::consts::FRAC_PI_3;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([pt(8.0, 8.0) + d * 4.2 * u, pt(8.0, 8.0) + d * 6.6 * u], Stroke::new(2.4, color));
            }
        }
        Icon::Search => {
            p.circle_stroke(pt(7.0, 7.0), 4.5 * u, s);
            line(pt(10.5, 10.5), pt(14.0, 14.0));
        }
        Icon::Lock => {
            p.rect_stroke(
                Rect::from_min_max(pt(3.5, 7.0), pt(12.5, 13.5)),
                1.2 * u,
                s,
                egui::StrokeKind::Inside,
            );
            arc(p, pt(8.0, 5.0), 2.5 * u, std::f32::consts::PI, std::f32::consts::TAU, s);
        }
        Icon::Check => path(vec![pt(3.0, 8.5), pt(6.5, 12.0), pt(13.0, 4.5)]),
        Icon::Windows => {
            for (x, y) in [(2.5, 2.5), (9.0, 2.5), (2.5, 9.0), (9.0, 9.0)] {
                p.rect_stroke(Rect::from_min_max(pt(x, y), pt(x + 4.5, y + 4.5)), 1, s, egui::StrokeKind::Inside);
            }
        }
        Icon::Chevron => path(vec![pt(6.0, 3.5), pt(10.5, 8.0), pt(6.0, 12.5)]),
        Icon::Gauge => {
            arc(p, pt(8.0, 11.0), 6.0 * u, std::f32::consts::PI, std::f32::consts::TAU, s);
            line(pt(8.0, 11.0), pt(11.0, 6.5));
            line(pt(2.0, 11.0), pt(14.0, 11.0));
        }
        Icon::Close => {
            line(pt(4.0, 4.0), pt(12.0, 12.0));
            line(pt(12.0, 4.0), pt(4.0, 12.0));
        }
    }
    let _ = c;
}

fn arc(p: &Painter, center: Pos2, r: f32, a0: f32, a1: f32, s: Stroke) {
    let n = 14;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            center + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(Shape::line(pts, PathStroke::new(s.width, s.color)));
}
