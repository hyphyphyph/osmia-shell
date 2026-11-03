//! Where captures go: folders, file names, the clipboard, and format conversion.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::grab::{png_bytes, save_png, Image};
use crate::config::CaptureCfg;

fn expand(dir: &str) -> PathBuf {
    if let Some(rest) = dir.strip_prefix("~/") {
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h).join(rest);
        }
    }
    PathBuf::from(dir)
}

/// A new file name in the folder, such as Screenshot_2026-11-03_14-02-11.png.
pub fn new_path(dir: &str, prefix: &str, ext: &str) -> Result<PathBuf, String> {
    let dir = expand(dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut p = dir.join(format!("{prefix}_{stamp}.{ext}"));
    let mut n = 1;
    while p.exists() {
        p = dir.join(format!("{prefix}_{stamp}_{n}.{ext}"));
        n += 1;
    }
    Ok(p)
}

/// Put PNG bytes on the clipboard. wl-copy keeps serving them after it returns.
pub fn copy_png(bytes: &[u8]) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("wl-copy is not available: {e}"))?;
    if let Some(mut si) = child.stdin.take() {
        si.write_all(bytes).map_err(|e| e.to_string())?;
    }
    child.wait().map(|_| ()).map_err(|e| e.to_string())
}

/// Save a screenshot and, if asked, copy it. Returns the file.
pub fn save_photo(img: &Image, cfg: &CaptureCfg) -> Result<PathBuf, String> {
    let path = new_path(&cfg.pictures_dir, "Screenshot", "png")?;
    save_png(img, &path)?;
    if cfg.copy {
        if let Err(e) = png_bytes(img).and_then(|b| copy_png(&b)) {
            log::warn!("clipboard: {e}");
        }
    }
    Ok(path)
}

pub fn videos_path(cfg: &CaptureCfg) -> Result<PathBuf, String> {
    new_path(&cfg.videos_dir, "Recording", "mp4")
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Export {
    Gif,
    Webm,
}

impl Export {
    pub fn ext(self) -> &'static str {
        match self {
            Export::Gif => "gif",
            Export::Webm => "webm",
        }
    }
    pub fn parse(s: &str) -> Option<Export> {
        match s {
            "gif" => Some(Export::Gif),
            "webm" => Some(Export::Webm),
            _ => None,
        }
    }
}

/// Convert a recording to GIF or WebM next to the original.
pub fn export(src: &Path, kind: Export) -> Result<PathBuf, String> {
    let dst = src.with_extension(kind.ext());
    let mut c = Command::new("ffmpeg");
    c.args(["-y", "-hide_banner", "-loglevel", "error", "-i"]).arg(src);
    match kind {
        Export::Gif => {
            c.args([
                "-vf",
                "fps=15,scale='min(960,iw)':-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5",
                "-loop",
                "0",
            ]);
        }
        Export::Webm => {
            c.args(["-c:v", "libvpx-vp9", "-crf", "32", "-b:v", "0", "-row-mt", "1", "-deadline", "good", "-cpu-used", "4", "-c:a", "libopus"]);
        }
    }
    let out = c.arg(&dst).stdin(Stdio::null()).output().map_err(|e| format!("ffmpeg is not available: {e}"))?;
    if out.status.success() {
        Ok(dst)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("ffmpeg failed").to_owned())
    }
}

/// Open a file or folder with the desktop's default program.
pub fn open(path: &Path) {
    crate::launch::spawn(&format!("xdg-open '{}'", path.display().to_string().replace('\'', "'\\''")));
}
