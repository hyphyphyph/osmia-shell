//! Screen and window capture, on a Wayland connection of its own.
//!
//! Uses ext-image-copy-capture with an output or a toplevel as the source, so a
//! window can be captured even when it is covered or minimized. Capturing blocks,
//! so callers run it on a thread. Pixels arrive through shared memory.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};

/// Straight-alpha RGBA, row after row.
#[derive(Clone)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// An output by name, such as "eDP-1".
    Output(String),
    /// A window by its foreign-toplevel identifier.
    Window(String),
}

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub app_id: String,
}

/// The byte order of a raw frame in memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fmt {
    /// B, G, R, A or B, G, R, X (wl_shm argb8888 / xrgb8888 on little-endian).
    Bgra,
    /// R, G, B, A or R, G, B, X (abgr8888 / xbgr8888).
    Rgba,
    /// R, G, B, three bytes per pixel (wl_shm bgr888).
    Rgb24,
    /// B, G, R, three bytes per pixel (wl_shm rgb888).
    Bgr24,
    /// 10 bits per channel, red in the low bits (xbgr2101010 / abgr2101010).
    Rgb10Low,
    /// 10 bits per channel, blue in the low bits (xrgb2101010 / argb2101010).
    Rgb10High,
}

impl Fmt {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Fmt::Rgb24 | Fmt::Bgr24 => 3,
            _ => 4,
        }
    }

    fn from_wl(f: u32) -> Option<(Fmt, bool)> {
        // (format, has_alpha). Codes are the wl_shm.format values.
        Some(match f {
            0 => (Fmt::Bgra, true),
            1 => (Fmt::Bgra, false),
            0x3432_4241 => (Fmt::Rgba, true),
            0x3432_4258 => (Fmt::Rgba, false),
            0x3432_4742 => (Fmt::Rgb24, false),
            0x3432_4752 => (Fmt::Bgr24, false),
            0x3033_4258 => (Fmt::Rgb10Low, false),
            0x3033_4241 => (Fmt::Rgb10Low, true),
            0x3033_5258 => (Fmt::Rgb10High, false),
            0x3033_5241 => (Fmt::Rgb10High, true),
            _ => return None,
        })
    }
}

/// A frame as the compositor wrote it.
pub struct Raw<'a> {
    pub w: u32,
    pub h: u32,
    pub stride: usize,
    pub fmt: Fmt,
    pub alpha: bool,
    pub transform: u32,
    pub data: &'a [u8],
}

// ---------- protocol state ----------

#[derive(Default)]
struct SessInfo {
    size: (u32, u32),
    formats: Vec<u32>,
    done: bool,
    stopped: bool,
}

#[derive(Default)]
struct FrameInfo {
    ready: bool,
    failed: Option<u32>,
    transform: u32,
}

#[derive(Default)]
struct WinData {
    id: String,
    title: String,
    app_id: String,
    closed: bool,
}

struct St {
    windows: Vec<ExtForeignToplevelHandleV1>,
}

type Shared<T> = Arc<Mutex<T>>;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for St {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_output::WlOutput, Shared<String>> for St {
    fn event(_: &mut Self, _: &wl_output::WlOutput, event: wl_output::Event, name: &Shared<String>, _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Name { name: n } = event {
            *name.lock().unwrap() = n;
        }
    }
}

macro_rules! ignore_events {
    ($($iface:ty),* $(,)?) => {$(
        impl Dispatch<$iface, ()> for St {
            fn event(_: &mut Self, _: &$iface, _: <$iface as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
ignore_events!(
    wl_shm::WlShm,
    wl_shm_pool::WlShmPool,
    wl_buffer::WlBuffer,
    ExtOutputImageCaptureSourceManagerV1,
    ExtForeignToplevelImageCaptureSourceManagerV1,
    ExtImageCaptureSourceV1,
    ExtImageCopyCaptureManagerV1,
);

impl Dispatch<ExtImageCopyCaptureSessionV1, Shared<SessInfo>> for St {
    fn event(
        _: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        info: &Shared<SessInfo>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_session_v1::Event as E;
        let mut i = info.lock().unwrap();
        match event {
            E::BufferSize { width, height } => i.size = (width, height),
            E::ShmFormat { format } => i.formats.push(match format {
                WEnum::Value(f) => f as u32,
                WEnum::Unknown(u) => u,
            }),
            E::Done => i.done = true,
            E::Stopped => i.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, Shared<FrameInfo>> for St {
    fn event(
        _: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        info: &Shared<FrameInfo>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_frame_v1::Event as E;
        let mut i = info.lock().unwrap();
        match event {
            E::Transform { transform } => {
                i.transform = match transform {
                    WEnum::Value(t) => t as u32,
                    WEnum::Unknown(u) => u,
                }
            }
            E::Ready => i.ready = true,
            E::Failed { reason } => {
                i.failed = Some(match reason {
                    WEnum::Value(r) => r as u32,
                    WEnum::Unknown(u) => u,
                })
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for St {
    fn event(state: &mut Self, _: &ExtForeignToplevelListV1, event: ext_foreign_toplevel_list_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.windows.push(toplevel);
        }
    }
    event_created_child!(St, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, Arc::new(Mutex::new(WinData::default()))),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, Shared<WinData>> for St {
    fn event(_: &mut Self, _: &ExtForeignToplevelHandleV1, event: ext_foreign_toplevel_handle_v1::Event, d: &Shared<WinData>, _: &Connection, _: &QueueHandle<Self>) {
        use ext_foreign_toplevel_handle_v1::Event as E;
        let mut d = d.lock().unwrap();
        match event {
            E::Title { title } => d.title = title,
            E::AppId { app_id } => d.app_id = app_id,
            E::Identifier { identifier } => d.id = identifier,
            E::Closed => d.closed = true,
            _ => {}
        }
    }
}

// ---------- shared memory buffers ----------

struct ShmBuf {
    _fd: OwnedFd,
    ptr: *mut u8,
    len: usize,
    buffer: wl_buffer::WlBuffer,
    w: u32,
    h: u32,
    stride: usize,
}

// SAFETY: the mapping is only touched by the thread that owns the Grabber.
unsafe impl Send for ShmBuf {}

impl ShmBuf {
    fn new(shm: &wl_shm::WlShm, qh: &QueueHandle<St>, w: u32, h: u32, format: u32) -> Result<ShmBuf, String> {
        let bpp = Fmt::from_wl(format).map(|(f, _)| f.bytes_per_pixel()).unwrap_or(4);
        let stride = w as usize * bpp;
        let len = stride * h as usize;
        // SAFETY: plain libc calls; the descriptor and mapping are owned by the result.
        unsafe {
            let fd = libc::memfd_create(c"osmia-capture".as_ptr(), libc::MFD_CLOEXEC);
            if fd < 0 {
                return Err("memfd_create failed".into());
            }
            let fd = OwnedFd::from_raw_fd(fd);
            if libc::ftruncate(fd.as_raw_fd(), len as libc::off_t) != 0 {
                return Err("ftruncate failed".into());
            }
            let ptr = libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, fd.as_raw_fd(), 0);
            if ptr == libc::MAP_FAILED {
                return Err("mmap failed".into());
            }
            let pool = shm.create_pool(std::os::fd::AsFd::as_fd(&fd), len as i32, qh, ());
            let fmt = wl_shm::Format::try_from(format).map_err(|_| "unknown buffer format".to_string())?;
            let buffer = pool.create_buffer(0, w as i32, h as i32, stride as i32, fmt, qh, ());
            pool.destroy();
            Ok(ShmBuf { _fd: fd, ptr: ptr as *mut u8, len, buffer, w, h, stride })
        }
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: the mapping is len bytes long and lives as long as self.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl Drop for ShmBuf {
    fn drop(&mut self) {
        self.buffer.destroy();
        // SAFETY: unmapping the region mapped in new().
        unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.len) };
    }
}

// ---------- the grabber ----------

pub struct Grabber {
    queue: EventQueue<St>,
    qh: QueueHandle<St>,
    st: St,
    shm: wl_shm::WlShm,
    copy: ExtImageCopyCaptureManagerV1,
    out_src: Option<ExtOutputImageCaptureSourceManagerV1>,
    win_src: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    outputs: Vec<(Shared<String>, wl_output::WlOutput)>,
    _list: Option<ExtForeignToplevelListV1>,
    _conn: Connection,
}

/// A frame that has been asked for.
pub struct Pending {
    frame: ExtImageCopyCaptureFrameV1,
    info: Shared<FrameInfo>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.frame.destroy();
    }
}

pub enum Poll {
    Ready,
    Waiting,
    /// The source changed size. Ask again.
    Resized,
}

/// One capture session on one source, with its buffer.
pub struct Cap {
    source: ExtImageCaptureSourceV1,
    session: ExtImageCopyCaptureSessionV1,
    info: Shared<SessInfo>,
    buf: Option<ShmBuf>,
    format: u32,
}

impl Drop for Cap {
    fn drop(&mut self) {
        self.session.destroy();
        self.source.destroy();
    }
}

impl Grabber {
    pub fn new() -> Result<Grabber, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("cannot connect to the display: {e}"))?;
        let (globals, mut queue) = registry_queue_init::<St>(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).map_err(|_| "the compositor has no wl_shm")?;
        let copy: ExtImageCopyCaptureManagerV1 = globals
            .bind(&qh, 1..=1, ())
            .map_err(|_| "the compositor does not offer ext-image-copy-capture")?;
        let out_src = globals.bind::<ExtOutputImageCaptureSourceManagerV1, _, _>(&qh, 1..=1, ()).ok();
        let win_src = globals.bind::<ExtForeignToplevelImageCaptureSourceManagerV1, _, _>(&qh, 1..=1, ()).ok();
        let list = globals.bind::<ExtForeignToplevelListV1, _, _>(&qh, 1..=1, ()).ok();
        let mut outputs = Vec::new();
        let infos: Vec<(u32, u32)> = globals
            .contents()
            .with_list(|l| l.iter().filter(|g| g.interface == "wl_output").map(|g| (g.name, g.version)).collect());
        for (name, version) in infos {
            let data: Shared<String> = Arc::new(Mutex::new(String::new()));
            let o: wl_output::WlOutput = globals.registry().bind(name, version.min(4), &qh, data.clone());
            outputs.push((data, o));
        }
        let mut st = St { windows: Vec::new() };
        // Two round trips: the first delivers names and handles, the second their details.
        queue.roundtrip(&mut st).map_err(|e| e.to_string())?;
        queue.roundtrip(&mut st).map_err(|e| e.to_string())?;
        Ok(Grabber { queue, qh, st, shm, copy, out_src, win_src, outputs, _list: list, _conn: conn })
    }

    pub fn output_names(&self) -> Vec<String> {
        self.outputs.iter().map(|(n, _)| n.lock().unwrap().clone()).collect()
    }

    pub fn windows(&self) -> Vec<WindowInfo> {
        self.st
            .windows
            .iter()
            .filter_map(|h| {
                let d = h.data::<Shared<WinData>>()?.lock().unwrap();
                (!d.closed && !d.id.is_empty()).then(|| WindowInfo { id: d.id.clone(), title: d.title.clone(), app_id: d.app_id.clone() })
            })
            .collect()
    }

    fn pump(&mut self, ms: i32) -> Result<(), String> {
        self.queue.dispatch_pending(&mut self.st).map_err(|e| e.to_string())?;
        self.queue.flush().map_err(|e| e.to_string())?;
        if let Some(guard) = self.queue.prepare_read() {
            let mut p = libc::pollfd { fd: guard.connection_fd().as_raw_fd(), events: libc::POLLIN, revents: 0 };
            // SAFETY: one valid pollfd.
            let r = unsafe { libc::poll(&mut p, 1, ms) };
            if r > 0 {
                guard.read().map_err(|e| e.to_string())?;
            } else {
                drop(guard);
            }
        }
        self.queue.dispatch_pending(&mut self.st).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn wait(&mut self, timeout: Duration, mut cond: impl FnMut() -> bool) -> Result<bool, String> {
        let end = Instant::now() + timeout;
        loop {
            if cond() {
                return Ok(true);
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(false);
            }
            self.pump(left.as_millis().min(50) as i32)?;
        }
    }

    /// Start a capture session on a source.
    pub fn open(&mut self, src: &Source, cursor: bool) -> Result<Cap, String> {
        let source = match src {
            Source::Output(name) => {
                let mgr = self.out_src.as_ref().ok_or("no output capture")?;
                let out = self
                    .outputs
                    .iter()
                    .find(|(n, _)| *n.lock().unwrap() == *name)
                    .map(|(_, o)| o.clone())
                    .or_else(|| self.outputs.first().map(|(_, o)| o.clone()))
                    .ok_or("no such output")?;
                mgr.create_source(&out, &self.qh, ())
            }
            Source::Window(id) => {
                let mgr = self.win_src.as_ref().ok_or("the compositor cannot capture single windows")?;
                let handle = self
                    .st
                    .windows
                    .iter()
                    .find(|h| h.data::<Shared<WinData>>().is_some_and(|d| d.lock().unwrap().id == *id))
                    .ok_or("that window is gone")?
                    .clone();
                mgr.create_source(&handle, &self.qh, ())
            }
        };
        let info: Shared<SessInfo> = Arc::new(Mutex::new(SessInfo::default()));
        let options = if cursor { Options::PaintCursors } else { Options::empty() };
        let session = self.copy.create_session(&source, options, &self.qh, info.clone());
        let mut cap = Cap { source, session, info, buf: None, format: 0 };
        self.refresh(&mut cap)?;
        Ok(cap)
    }

    /// Read the buffer constraints and make a buffer to match.
    fn refresh(&mut self, cap: &mut Cap) -> Result<(), String> {
        let info = cap.info.clone();
        if !self.wait(Duration::from_secs(3), || info.lock().unwrap().done || info.lock().unwrap().stopped)? {
            return Err("the compositor did not answer".into());
        }
        let (size, formats, stopped) = {
            let mut i = info.lock().unwrap();
            i.done = false;
            (i.size, std::mem::take(&mut i.formats), i.stopped)
        };
        if stopped {
            return Err("the source went away".into());
        }
        let pick = [0u32, 1, 0x3432_4241, 0x3432_4258, 0x3033_5258, 0x3033_4258, 0x3033_5241, 0x3033_4241]
            .into_iter()
            .find(|f| formats.contains(f))
            .or_else(|| [0x3432_4752u32, 0x3432_4742].into_iter().find(|f| formats.contains(f)))
            .ok_or_else(|| format!("no buffer format we can read (offered {formats:x?})"))?;
        if size.0 == 0 || size.1 == 0 {
            return Err("the source has no size".into());
        }
        cap.buf = Some(ShmBuf::new(&self.shm, &self.qh, size.0, size.1, pick)?);
        cap.format = pick;
        Ok(())
    }

    /// Ask for the next frame. It arrives once something on the source changes.
    pub fn begin(&mut self, cap: &mut Cap) -> Result<Pending, String> {
        let (w, h, buffer) = {
            let b = cap.buf.as_ref().ok_or("no buffer")?;
            (b.w, b.h, b.buffer.clone())
        };
        let info: Shared<FrameInfo> = Arc::new(Mutex::new(FrameInfo::default()));
        let frame = cap.session.create_frame(&self.qh, info.clone());
        frame.attach_buffer(&buffer);
        frame.damage_buffer(0, 0, w as i32, h as i32);
        frame.capture();
        Ok(Pending { frame, info })
    }

    /// Wait up to `timeout` for a requested frame. Ready means the buffer holds it.
    pub fn poll(&mut self, cap: &mut Cap, p: &mut Pending, timeout: Duration) -> Result<Poll, String> {
        let fi = p.info.clone();
        let finished = self.wait(timeout, || {
            let f = fi.lock().unwrap();
            f.ready || f.failed.is_some() || cap.info.lock().unwrap().stopped
        })?;
        if !finished {
            return Ok(Poll::Waiting);
        }
        if cap.info.lock().unwrap().stopped {
            return Err("the source went away".into());
        }
        let (ready, failed) = {
            let f = p.info.lock().unwrap();
            (f.ready, f.failed)
        };
        if ready {
            return Ok(Poll::Ready);
        }
        // 1 means the buffer constraints changed, for example the window was resized.
        if failed == Some(1) {
            self.refresh(cap)?;
            return Ok(Poll::Resized);
        }
        Err(format!("the compositor could not capture this ({failed:?})"))
    }

    /// The frame that `poll` reported ready. Valid until the next request.
    pub fn view<'a>(&self, cap: &'a Cap, p: &Pending) -> Result<Raw<'a>, String> {
        let b = cap.buf.as_ref().ok_or("no buffer")?;
        let (fmt, alpha) = Fmt::from_wl(cap.format).ok_or("bad format")?;
        let transform = p.info.lock().unwrap().transform;
        Ok(Raw { w: b.w, h: b.h, stride: b.stride, fmt, alpha, transform, data: b.bytes() })
    }

    /// Capture one frame and wait for it.
    pub fn frame<'a>(&mut self, cap: &'a mut Cap, timeout: Duration) -> Result<Raw<'a>, String> {
        for _ in 0..3 {
            let mut p = self.begin(cap)?;
            match self.poll(cap, &mut p, timeout)? {
                Poll::Ready => return self.view(cap, &p),
                Poll::Waiting => return Err("the capture timed out".into()),
                Poll::Resized => continue,
            }
        }
        Err("the capture kept failing".into())
    }

    /// Capture one frame of a source as an image.
    pub fn grab(&mut self, src: &Source, cursor: bool) -> Result<Image, String> {
        let mut cap = self.open(src, cursor)?;
        let raw = self.frame(&mut cap, Duration::from_secs(5))?;
        Ok(to_image(&raw))
    }
}

// ---------- pixels ----------

/// Convert a raw frame to upright, straight-alpha RGBA.
pub fn to_image(raw: &Raw) -> Image {
    let (w, h) = (raw.w as usize, raw.h as usize);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let bpp = raw.fmt.bytes_per_pixel();
        let row = &raw.data[y * raw.stride..y * raw.stride + w * bpp];
        let dst = &mut out[y * w * 4..(y + 1) * w * 4];
        for (s, d) in row.chunks_exact(bpp).zip(dst.chunks_exact_mut(4)) {
            let (r, g, b, a) = match raw.fmt {
                Fmt::Rgb24 => (s[0], s[1], s[2], 255),
                Fmt::Bgr24 => (s[2], s[1], s[0], 255),
                Fmt::Bgra => (s[2], s[1], s[0], s[3]),
                Fmt::Rgba => (s[0], s[1], s[2], s[3]),
                Fmt::Rgb10Low | Fmt::Rgb10High => {
                    let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                    let (c0, c1, c2) = ((v & 0x3ff) >> 2, ((v >> 10) & 0x3ff) >> 2, ((v >> 20) & 0x3ff) >> 2);
                    let a = (v >> 30) as u8 * 85;
                    if raw.fmt == Fmt::Rgb10Low { (c0 as u8, c1 as u8, c2 as u8, a) } else { (c2 as u8, c1 as u8, c0 as u8, a) }
                }
            };
            if raw.alpha {
                // Buffers carry premultiplied alpha. Undo it for PNG.
                if a == 0 {
                    d.copy_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    d.copy_from_slice(&[r, g, b, 255]);
                } else {
                    let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                    d.copy_from_slice(&[un(r), un(g), un(b), a]);
                }
            } else {
                d.copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    rotate_back(Image { w: raw.w, h: raw.h, rgba: out }, raw.transform)
}

/// The compositor says it rotated the buffer. Turn it back so the picture is upright.
fn rotate_back(img: Image, transform: u32) -> Image {
    let (w, h) = (img.w as usize, img.h as usize);
    // wl_output.transform: 1 = 90, 2 = 180, 3 = 270 degrees, counter-clockwise.
    let turn = match transform & 3 {
        0 => return img,
        1 => 3, // undo 90 ccw = 90 cw... expressed as ccw turns of the result
        2 => 2,
        _ => 1,
    };
    let (nw, nh) = if turn % 2 == 1 { (h, w) } else { (w, h) };
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = match turn {
                1 => (y, w - 1 - x),      // 90 ccw
                2 => (w - 1 - x, h - 1 - y),
                _ => (h - 1 - y, x),      // 90 cw
            };
            let (s, d) = ((y * w + x) * 4, (ny * nw + nx) * 4);
            out[d..d + 4].copy_from_slice(&img.rgba[s..s + 4]);
        }
    }
    Image { w: nw as u32, h: nh as u32, rgba: out }
}

/// Write a PNG file.
pub fn save_png(img: &Image, path: &std::path::Path) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w, img.h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&img.rgba).map_err(|e| e.to_string())
}

/// PNG bytes in memory, for the clipboard.
pub fn png_bytes(img: &Image) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, img.w, img.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&img.rgba).map_err(|e| e.to_string())?;
    }
    Ok(buf)
}
