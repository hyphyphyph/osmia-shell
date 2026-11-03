//! The built-in screenshot and screen recording tool.

pub mod grab;
pub mod out;
pub mod ui;
pub mod video;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use grab::{Grabber, Image, Source};
use smithay_client_toolkit::reexports::calloop::channel::Sender;

/// What the capture threads tell the shell.
pub enum CapMsg {
    /// The screens are captured; show the overlay.
    Ready { shots: Vec<ui::Shot>, mode: ui::Mode, kind: ui::Kind },
    /// Pictures of the windows, which take a little longer.
    Windows(Vec<ui::WinShot>),
    /// A screenshot is saved: the file and a small picture of it.
    PhotoDone(Result<(PathBuf, Option<PathBuf>), String>),
    VideoDone(Result<PathBuf, String>),
    ExportDone(Result<PathBuf, String>),
    Failed(String),
}

/// Cut a rectangle out of an image.
pub fn crop(img: &Image, x: u32, y: u32, w: u32, h: u32) -> Image {
    let x = x.min(img.w.saturating_sub(1));
    let y = y.min(img.h.saturating_sub(1));
    let w = w.min(img.w - x).max(1);
    let h = h.min(img.h - y).max(1);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let s = ((row * img.w + x) * 4) as usize;
        rgba.extend_from_slice(&img.rgba[s..s + (w * 4) as usize]);
    }
    Image { w, h, rgba }
}

/// Shrink an image so its long side is at most `max`. A simple box average.
pub fn shrink(img: &Image, max: u32) -> Image {
    let long = img.w.max(img.h);
    if long <= max {
        return img.clone();
    }
    let f = long as f32 / max as f32;
    let (w, h) = (((img.w as f32 / f).round() as u32).max(1), ((img.h as f32 / f).round() as u32).max(1));
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let (y0, y1) = ((y as f32 * f) as u32, (((y + 1) as f32 * f) as u32).min(img.h).max((y as f32 * f) as u32 + 1));
        for x in 0..w {
            let (x0, x1) = ((x as f32 * f) as u32, (((x + 1) as f32 * f) as u32).min(img.w).max((x as f32 * f) as u32 + 1));
            let mut acc = [0u32; 4];
            let mut n = 0;
            for sy in y0..y1.min(img.h) {
                for sx in x0..x1.min(img.w) {
                    let i = ((sy * img.w + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += img.rgba[i + c] as u32;
                    }
                    n += 1;
                }
            }
            let n = n.max(1);
            let d = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                rgba[d + c] = (acc[c] / n) as u8;
            }
        }
    }
    Image { w, h, rgba }
}

/// Capture every screen, then the windows. Runs on a thread.
pub fn gather(cursor: bool, mode: ui::Mode, kind: ui::Kind, tx: Sender<CapMsg>) {
    std::thread::spawn(move || {
        // Give a panel that was just closed a moment to leave the screen.
        std::thread::sleep(Duration::from_millis(90));
        let mut g = match Grabber::new() {
            Ok(g) => g,
            Err(e) => return drop(tx.send(CapMsg::Failed(e))),
        };
        let mut shots = Vec::new();
        for name in g.output_names() {
            match g.grab(&Source::Output(name.clone()), cursor) {
                Ok(img) => shots.push(ui::Shot { name, image: Arc::new(img) }),
                Err(e) => log::warn!("capture of {name}: {e}"),
            }
        }
        if shots.is_empty() {
            return drop(tx.send(CapMsg::Failed("no screen could be captured".into())));
        }
        let _ = tx.send(CapMsg::Ready { shots, mode, kind });
        let mut wins = Vec::new();
        for w in g.windows() {
            if let Ok(mut cap) = g.open(&Source::Window(w.id.clone()), false) {
                if let Ok(raw) = g.frame(&mut cap, Duration::from_millis(600)) {
                    let image = grab::to_image(&raw);
                    let thumb = shrink(&image, 480);
                    wins.push(ui::WinShot { id: w.id, title: w.title, app_id: w.app_id, image: Arc::new(image), thumb: Arc::new(thumb) });
                }
            }
        }
        let _ = tx.send(CapMsg::Windows(wins));
    });
}

/// Save a screenshot on a thread and report back. Also makes a small picture for the notification.
pub fn save_photo(img: Image, cfg: crate::config::CaptureCfg, tx: Sender<CapMsg>) {
    std::thread::spawn(move || {
        let r = out::save_photo(&img, &cfg).map(|path| {
            let thumb = shrink(&img, 96);
            let cache = std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
                .map(|c| c.join("osmia").join("thumbs"));
            let thumb_path = cache.and_then(|dir| {
                std::fs::create_dir_all(&dir).ok()?;
                let p = dir.join(path.file_name()?);
                grab::save_png(&thumb, &p).ok()?;
                Some(p)
            });
            (path, thumb_path)
        });
        let _ = tx.send(CapMsg::PhotoDone(r));
    });
}

/// Take a fresh picture of a source after a delay, crop it if asked, and save it.
pub fn photo_later(src: Source, crop_px: Option<(u32, u32, u32, u32)>, cursor: bool, delay: u32, cfg: crate::config::CaptureCfg, tx: Sender<CapMsg>) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(delay as u64));
        let img = Grabber::new().and_then(|mut g| g.grab(&src, cursor));
        match img {
            Ok(img) => {
                let img = match crop_px {
                    Some((x, y, w, h)) => crop(&img, x, y, w, h),
                    None => img,
                };
                save_photo(img, cfg, tx);
            }
            Err(e) => {
                let _ = tx.send(CapMsg::PhotoDone(Err(e)));
            }
        }
    });
}

/// `osmia grab [--list] [--output NAME | --window ID] [--cursor] FILE.png`
/// Takes one picture without the shell. Mostly for testing and scripts.
pub fn cli(args: &[String]) -> i32 {
    let mut src = None;
    let mut cursor = false;
    let mut file = None;
    let mut list = false;
    let mut video: Option<u64> = None;
    let mut audio = video::Audio::None;
    let mut crop = None;
    let mut encoder = String::from("auto");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--list" => list = true,
            "--cursor" => cursor = true,
            "--video" => video = it.next().and_then(|n| n.parse::<u64>().ok()),
            "--audio" => audio = it.next().map(|a| video::Audio::parse(a)).unwrap_or(video::Audio::None),
            "--crop" => {
                crop = it.next().and_then(|c| {
                    let v: Vec<u32> = c.split(',').filter_map(|n| n.parse().ok()).collect();
                    (v.len() == 4).then(|| (v[0], v[1], v[2], v[3]))
                })
            }
            "--encoder" => encoder = it.next().cloned().unwrap_or_default(),
            "--output" => src = it.next().map(|n| grab::Source::Output(n.clone())),
            "--window" => src = it.next().map(|n| grab::Source::Window(n.clone())),
            f => file = Some(f.to_owned()),
        }
    }
    let mut g = match grab::Grabber::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("osmia grab: {e}");
            return 1;
        }
    };
    if list {
        for o in g.output_names() {
            println!("output {o}");
        }
        for w in g.windows() {
            println!("window {} [{}] {}", w.id, w.app_id, w.title);
        }
        return 0;
    }
    let src = src.unwrap_or_else(|| grab::Source::Output(g.output_names().into_iter().next().unwrap_or_default()));
    let Some(file) = file else {
        eprintln!("usage: osmia grab [--list] [--output NAME | --window ID] [--cursor] FILE.png");
        return 1;
    };
    if let Some(secs) = video {
        drop(g);
        let (tx, rx) = std::sync::mpsc::channel();
        let rec = video::start(
            video::Opts { source: src, crop, cursor, fps: 30, audio, encoder, quality: "balanced".into(), out: file.into(), delay: 0 },
            move |r| {
                let _ = tx.send(r);
            },
        );
        std::thread::sleep(std::time::Duration::from_secs(secs));
        rec.stop();
        return match rx.recv() {
            Ok(Ok(p)) => {
                println!("recorded {}", p.display());
                0
            }
            Ok(Err(e)) => {
                eprintln!("osmia grab: {e}");
                1
            }
            Err(e) => {
                eprintln!("osmia grab: {e}");
                1
            }
        };
    }
    let t = std::time::Instant::now();
    match g.grab(&src, cursor).and_then(|img| grab::save_png(&img, std::path::Path::new(&file)).map(|_| img)) {
        Ok(img) => {
            println!("{}x{} in {:?}", img.w, img.h, t.elapsed());
            0
        }
        Err(e) => {
            eprintln!("osmia grab: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Image {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        Image { w, h, rgba }
    }

    #[test]
    fn crop_takes_the_right_pixels_and_stays_inside() {
        let img = gradient(10, 8);
        let c = crop(&img, 2, 3, 4, 2);
        assert_eq!((c.w, c.h), (4, 2));
        assert_eq!(&c.rgba[0..4], &[2, 3, 0, 255]);
        assert_eq!(&c.rgba[(4 * 4) as usize..(4 * 4 + 4) as usize], &[2, 4, 0, 255]);
        // A rectangle that hangs off the edge is cut to fit.
        let c = crop(&img, 8, 6, 100, 100);
        assert_eq!((c.w, c.h), (2, 2));
    }

    #[test]
    fn shrink_keeps_the_aspect_ratio_and_only_shrinks() {
        let img = gradient(200, 100);
        let s = shrink(&img, 50);
        assert_eq!((s.w, s.h), (50, 25));
        let same = shrink(&img, 500);
        assert_eq!((same.w, same.h), (200, 100));
    }

    #[test]
    fn premultiplied_alpha_is_undone_for_png() {
        use grab::{to_image, Fmt, Raw};
        // One pixel, half transparent, premultiplied: B=50 G=100 R=150 A=200 is not valid;
        // use a valid one: R=100 G=50 B=0 at alpha 128 premultiplied means about 199, 99, 0.
        let data = [0u8, 50, 100, 128];
        let raw = Raw { w: 1, h: 1, stride: 4, fmt: Fmt::Bgra, alpha: true, transform: 0, data: &data };
        let img = to_image(&raw);
        assert_eq!(img.rgba[3], 128);
        assert!(img.rgba[0] >= 198 && img.rgba[0] <= 200, "{:?}", img.rgba);
        assert!(img.rgba[1] >= 98 && img.rgba[1] <= 100);
        // Opaque formats get full alpha whatever the unused byte says.
        let raw = Raw { w: 1, h: 1, stride: 4, fmt: Fmt::Bgra, alpha: false, transform: 0, data: &[1, 2, 3, 0] };
        assert_eq!(to_image(&raw).rgba, vec![3, 2, 1, 255]);
    }
}
