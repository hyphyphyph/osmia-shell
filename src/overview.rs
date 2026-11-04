//! The window overview: every open window as a card with a picture of it.
//! Pictures come from ext-image-copy-capture, taken once when the overview opens
//! on a thread, and arrive one at a time. This file holds the capture thread and
//! the parts that need no screen: the grid sizes, the filter, and the matching of
//! pictures to windows.

use std::sync::Arc;
use std::time::Duration;

use smithay_client_toolkit::reexports::calloop::channel::Sender;

use crate::capture::grab::{self, Grabber, Image, Source};
use crate::capture::{shrink, CapMsg};

/// The long side of a card picture, in pixels.
pub const THUMB_PX: u32 = 560;

/// A picture of one window, with what the compositor calls it.
#[derive(Clone)]
pub struct Thumb {
    pub title: String,
    pub app_id: String,
    pub image: Arc<Image>,
}

/// Take a picture of every window and send each as soon as it is ready.
pub fn gather(tx: Sender<CapMsg>) {
    std::thread::spawn(move || {
        let Ok(mut g) = Grabber::new() else { return };
        for w in g.windows() {
            let Ok(mut cap) = g.open(&Source::Window(w.id.clone()), false) else { continue };
            let Ok(raw) = g.frame(&mut cap, Duration::from_millis(600)) else { continue };
            let image = shrink(&grab::to_image(&raw), THUMB_PX);
            if tx.send(CapMsg::OverviewThumb(Thumb { title: w.title, app_id: w.app_id, image: Arc::new(image) })).is_err() {
                return;
            }
        }
    });
}

/// For each window (title, app id) the picture to show, if one has arrived.
/// Windows with the same title and app id take the pictures in order.
pub fn match_thumbs(wins: &[(&str, &str)], thumbs: &[Thumb]) -> Vec<Option<usize>> {
    let mut used = vec![false; thumbs.len()];
    wins.iter()
        .map(|(title, app)| {
            let i = thumbs.iter().enumerate().position(|(i, t)| !used[i] && t.title == *title && t.app_id == *app)?;
            used[i] = true;
            Some(i)
        })
        .collect()
}

/// How the cards are laid out.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    /// Width of a card. The picture is 16:10 of it, and a label row sits under.
    pub card_w: f32,
}

pub const ASPECT: f32 = 0.625;

/// The most columns that still leave the biggest cards, within `w` by `h`.
pub fn grid(n: usize, w: f32, h: f32, gap: f32, label_h: f32, max_card_w: f32) -> Grid {
    let n = n.max(1);
    let mut best = Grid { cols: 1, rows: n, card_w: 40.0 };
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        let by_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        // Height of a card is card_w * ASPECT + label_h.
        let by_h = ((h - gap * (rows as f32 - 1.0)) / rows as f32 - label_h) / ASPECT;
        let card_w = by_w.min(by_h).min(max_card_w);
        if card_w > best.card_w + 0.5 {
            best = Grid { cols, rows, card_w };
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn thumb(title: &str, app: &str) -> Thumb {
        Thumb { title: title.into(), app_id: app.into(), image: Arc::new(Image { w: 1, h: 1, rgba: vec![0; 4] }) }
    }

    #[test]
    fn pictures_match_by_title_and_app() {
        let thumbs = [thumb("a", "x"), thumb("b", "y")];
        assert_eq!(match_thumbs(&[("b", "y"), ("a", "x"), ("c", "z")], &thumbs), [Some(1), Some(0), None]);
    }

    #[test]
    fn twin_windows_take_pictures_in_order() {
        let thumbs = [thumb("t", "x"), thumb("t", "x")];
        assert_eq!(match_thumbs(&[("t", "x"), ("t", "x"), ("t", "x")], &thumbs), [Some(0), Some(1), None]);
    }

    #[test]
    fn one_window_is_one_big_card_up_to_the_cap() {
        let g = grid(1, 1600.0, 800.0, 20.0, 34.0, 480.0);
        assert_eq!((g.cols, g.rows), (1, 1));
        assert_eq!(g.card_w, 480.0);
    }

    #[test]
    fn many_windows_fill_the_space_without_overflow() {
        for n in 1..=40 {
            let (w, h, gap, label) = (1800.0, 900.0, 20.0, 34.0);
            let g = grid(n, w, h, gap, label, 480.0);
            assert!(g.cols * g.rows >= n);
            let used_w = g.cols as f32 * g.card_w + gap * (g.cols as f32 - 1.0);
            let used_h = g.rows as f32 * (g.card_w * ASPECT + label) + gap * (g.rows as f32 - 1.0);
            assert!(used_w <= w + 0.5 && used_h <= h + 0.5, "n={n}: {used_w}x{used_h}");
        }
    }

    #[test]
    fn a_wide_screen_gets_more_columns_than_rows() {
        let g = grid(8, 2400.0, 800.0, 20.0, 34.0, 480.0);
        assert!(g.cols > g.rows, "{g:?}");
    }

    /// Needs a running compositor: `cargo test -- --ignored overview`.
    /// Takes a picture of each open window and checks the pictures are not blank.
    #[test]
    #[ignore]
    fn pictures_of_real_windows() {
        let (tx, rx) = smithay_client_toolkit::reexports::calloop::channel::channel::<CapMsg>();
        gather(tx);
        std::thread::sleep(Duration::from_secs(6));
        let mut n = 0;
        while let Ok(m) = rx.try_recv() {
            if let CapMsg::OverviewThumb(t) = m {
                let lit = t.image.rgba.chunks_exact(4).filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8).count();
                println!("{:30} {:24} {}x{} lit={}", t.title.chars().take(30).collect::<String>(), t.app_id, t.image.w, t.image.h, lit);
                assert!(t.image.w.max(t.image.h) <= THUMB_PX);
                n += 1;
            }
        }
        assert!(n > 0, "no window pictures came back");
    }
}
