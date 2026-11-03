//! Screen recording. Frames come from the grabber, are cropped, and are piped
//! to ffmpeg as raw video at a steady rate. ffmpeg encodes and mixes the audio.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use super::grab::{to_image, Fmt, Grabber, Poll, Raw, Source};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Audio {
    None,
    System,
    Mic,
    Both,
}

impl Audio {
    pub fn parse(s: &str) -> Audio {
        match s {
            "system" => Audio::System,
            "mic" => Audio::Mic,
            "both" => Audio::Both,
            _ => Audio::None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub source: Source,
    /// x, y, width, height in buffer pixels. None records the whole source.
    pub crop: Option<(u32, u32, u32, u32)>,
    pub cursor: bool,
    pub fps: u32,
    pub audio: Audio,
    /// "auto", "hardware" or "software".
    pub encoder: String,
    /// "small", "balanced" or "high".
    pub quality: String,
    pub out: PathBuf,
    /// Wait this long before the first frame.
    pub delay: u32,
}

pub struct Recording {
    stop: Arc<AtomicBool>,
    pub started: Instant,
}

impl Recording {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Starts recording on a thread. `done` gets the file, or why it failed.
pub fn start(opts: Opts, done: impl FnOnce(Result<PathBuf, String>) + Send + 'static) -> Recording {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let delay = Duration::from_secs(opts.delay as u64);
    let rec = Recording { stop, started: Instant::now() + delay };
    std::thread::spawn(move || {
        let end = Instant::now() + delay;
        while Instant::now() < end && !flag.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(50));
        }
        if flag.load(Ordering::Relaxed) {
            return done(Err("cancelled".into()));
        }
        done(run(&opts, &flag));
    });
    rec
}

fn pactl(args: &[&str]) -> String {
    Command::new("pactl")
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// Does ffmpeg's VAAPI encoder work on this machine? Tried once.
fn hardware_works() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| {
        if !std::path::Path::new("/dev/dri/renderD128").exists() {
            return false;
        }
        Command::new("ffmpeg")
            .args([
                "-hide_banner", "-loglevel", "error", "-vaapi_device", "/dev/dri/renderD128", "-f", "lavfi", "-i", "color=black:s=256x256:d=0.2",
                "-vf", "format=nv12,hwupload", "-c:v", "h264_vaapi", "-f", "null", "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

fn ffmpeg_args(o: &Opts, w: u32, h: u32, pix: &str, hw: bool) -> Vec<String> {
    let mut a: Vec<String> = ["-y", "-hide_banner", "-loglevel", "error"].map(String::from).to_vec();
    if hw {
        a.extend(["-vaapi_device", "/dev/dri/renderD128"].map(String::from));
    }
    // Frames are written in real time, so wall-clock stamps keep them in step with the audio.
    a.extend(["-use_wallclock_as_timestamps", "1", "-f", "rawvideo", "-pix_fmt", pix, "-s", &format!("{w}x{h}"), "-i", "-"].map(String::from));
    let mut audio_inputs = 0;
    if matches!(o.audio, Audio::System | Audio::Both) {
        let sink = pactl(&["get-default-sink"]);
        a.extend(["-thread_queue_size", "1024", "-f", "pulse", "-i", &format!("{sink}.monitor")].map(String::from));
        audio_inputs += 1;
    }
    if matches!(o.audio, Audio::Mic | Audio::Both) {
        let src = pactl(&["get-default-source"]);
        a.extend(["-thread_queue_size", "1024", "-f", "pulse", "-i", if src.is_empty() { "default" } else { &src }].map(String::from));
        audio_inputs += 1;
    }
    let (crf, qp) = match o.quality.as_str() {
        "small" => ("28", "30"),
        "high" => ("18", "19"),
        _ => ("23", "24"),
    };
    if hw {
        a.extend(["-vf", "format=nv12,hwupload", "-c:v", "h264_vaapi", "-qp", qp].map(String::from));
    } else {
        a.extend(["-vf", "format=yuv420p", "-c:v", "libx264", "-preset", "veryfast", "-crf", crf].map(String::from));
    }
    a.extend(["-fps_mode", "cfr", "-r", &o.fps.to_string(), "-map", "0:v"].map(String::from));
    match audio_inputs {
        1 => a.extend(["-map", "1:a", "-c:a", "aac", "-b:a", "160k"].map(String::from)),
        2 => a.extend(["-filter_complex", "[1:a][2:a]amix=inputs=2:duration=longest[a]", "-map", "[a]", "-c:a", "aac", "-b:a", "160k"].map(String::from)),
        _ => {}
    }
    if audio_inputs > 0 {
        // The video ends the recording; live audio never ends by itself.
        a.push("-shortest".into());
    }
    a.push(o.out.display().to_string());
    a
}

/// Copy a region of the frame into `dst` (w x h, 4 bytes per pixel), black where there is no source.
fn blit(raw: &Raw, crop: (u32, u32, u32, u32), dst: &mut [u8], w: u32, h: u32, convert: bool) {
    let (cx, cy, _, _) = crop;
    let rgba;
    let (data, stride) = if convert {
        rgba = to_image(raw);
        (&rgba.rgba[..], rgba.w as usize * 4)
    } else {
        (raw.data, raw.stride)
    };
    let (sw, sh) = (raw.w as usize, raw.h as usize);
    for row in 0..h as usize {
        let sy = cy as usize + row;
        let d = &mut dst[row * w as usize * 4..(row + 1) * w as usize * 4];
        if sy >= sh || cx as usize >= sw {
            d.fill(0);
            continue;
        }
        let n = (w as usize).min(sw - cx as usize);
        let s = &data[sy * stride + cx as usize * 4..sy * stride + (cx as usize + n) * 4];
        d[..n * 4].copy_from_slice(s);
        d[n * 4..].fill(0);
    }
}

fn run(o: &Opts, stop: &AtomicBool) -> Result<PathBuf, String> {
    let mut g = Grabber::new()?;
    let mut cap = g.open(&o.source, o.cursor)?;
    // The first frame tells us the size and the pixel layout.
    let first = g.frame(&mut cap, Duration::from_secs(5))?;
    let (fw, fh) = (first.w, first.h);
    let (mut cx, mut cy, mut cw, mut ch) = o.crop.unwrap_or((0, 0, fw, fh));
    cx = cx.min(fw.saturating_sub(2));
    cy = cy.min(fh.saturating_sub(2));
    cw = cw.min(fw - cx);
    ch = ch.min(fh - cy);
    // H.264 wants even sizes.
    let (w, h) = ((cw & !1).max(2), (ch & !1).max(2));
    let convert = !matches!(first.fmt, Fmt::Bgra | Fmt::Rgba);
    let pix = if first.fmt == Fmt::Bgra { "bgra" } else { "rgba" };
    let crop = (cx, cy, w, h);
    let mut latest = vec![0u8; (w * h * 4) as usize];
    blit(&first, crop, &mut latest, w, h, convert);

    let want_hw = match o.encoder.as_str() {
        "hardware" => true,
        "software" => false,
        _ => hardware_works(),
    };
    let spawn = |hw: bool| -> Result<Child, String> {
        Command::new("ffmpeg")
            .args(ffmpeg_args(o, w, h, pix, hw))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg is not available: {e}"))
    };
    let mut child = spawn(want_hw)?;
    let mut stdin = child.stdin.take().ok_or("no ffmpeg input")?;

    let dur = Duration::from_secs_f64(1.0 / o.fps.max(1) as f64);
    let start = Instant::now();
    let mut n: u64 = 0;
    let mut pending = g.begin(&mut cap)?;
    let mut failure: Option<String> = None;
    while !stop.load(Ordering::Relaxed) {
        let tick = start + dur * n as u32;
        let wait = tick.saturating_duration_since(Instant::now());
        match g.poll(&mut cap, &mut pending, wait.min(Duration::from_millis(20))) {
            Ok(Poll::Ready) => {
                if let Ok(raw) = g.view(&cap, &pending) {
                    blit(&raw, crop, &mut latest, w, h, convert);
                }
                match g.begin(&mut cap) {
                    Ok(p) => pending = p,
                    Err(e) => {
                        failure = Some(e);
                        break;
                    }
                }
            }
            Ok(Poll::Resized) => match g.begin(&mut cap) {
                Ok(p) => pending = p,
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            },
            Ok(Poll::Waiting) => {}
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
        if Instant::now() >= tick {
            if stdin.write_all(&latest).is_err() {
                failure = Some("ffmpeg stopped".into());
                break;
            }
            n += 1;
            // If we fell far behind, skip ahead instead of bursting.
            if Instant::now() > start + dur * (n as u32 + 5) {
                n = (Instant::now() - start).as_secs_f64().div_euclid(dur.as_secs_f64()) as u64;
            }
        }
    }
    drop(stdin);
    // Let ffmpeg write the end of the file. Interrupt it if audio keeps it open.
    let t = Instant::now();
    let mut interrupted = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if t.elapsed() > Duration::from_secs(20) => {
                let _ = child.kill();
                break child.wait().ok();
            }
            Ok(None) => {
                if !interrupted && t.elapsed() > Duration::from_secs(1) {
                    // SAFETY: signalling our own child process.
                    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
                    interrupted = true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break None,
        }
    };
    let mut err_text = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut e, &mut err_text);
    }
    if let Some(f) = failure {
        if !o.out.exists() {
            return Err(f);
        }
    }
    if !o.out.exists() || std::fs::metadata(&o.out).map(|m| m.len()).unwrap_or(0) < 1024 {
        // Hardware encoding can fail on odd machines. Say what ffmpeg said.
        let why = err_text.lines().last().unwrap_or("ffmpeg wrote nothing").to_owned();
        return Err(if status.is_some_and(|s| s.success()) { "the recording was empty".into() } else { why });
    }
    Ok(o.out.clone())
}
