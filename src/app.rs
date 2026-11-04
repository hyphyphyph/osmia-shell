//! The shell: Wayland state, event handlers, and the draw loop.

use std::ptr::NonNull;
use std::time::{Duration, Instant};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{channel, EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1;

use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter};

use crate::config::{self, Config};
use crate::control;
use crate::gfx::Gfx;
use crate::launch;
use crate::notif::{self, Notification, Notifs};
use crate::sys::{self, Sampler, Stats};
use crate::toplevel::Toplevel;
use crate::ui::{self, bar, launcher, panels, popups, Action, Overlay, PowerAction};
use crate::win::{Kind, Surf, Win};

const SLOW_TICK: Duration = Duration::from_secs(2);

/// A recording in progress, or being saved.
pub struct RecState {
    pub rec: crate::capture::video::Recording,
    pub stopping: bool,
}

pub struct Shared {
    pub wifi_scanning: bool,
    pub workspaces: Vec<crate::workspace::Workspace>,
    /// Keyboard shortcuts read from labwc's rc.xml, for the settings page.
    pub binds: Vec<crate::keybinds::Bind>,
    pub binds_error: Option<String>,
    /// The settings page is waiting for a key press to name a shortcut.
    pub recording_key: bool,
    /// A settings page to switch to on the next draw, such as "shortcuts".
    pub settings_goto: Option<String>,
    /// What it heard. Empty means the recording was cancelled.
    pub captured_key: Option<String>,
    pub stats: Stats,
    pub sampler: Sampler,
    pub cfg: Config,
    pub cfg_mtime: Option<std::time::SystemTime>,
    pub notifs: Notifs,
    pub notif_conn: Option<zbus::Connection>,
    pub overlay: Option<Overlay>,
    pub overlay_anchor: f32,
    pub launcher: launcher::State,
    pub launch_data: Option<launch::Data>,
    pub bar_rects: Vec<(String, egui::Rect)>,
    pub last_output: Option<wl_output::WlOutput>,
    pub toplevels: Vec<Toplevel>,
    pub popup_hover: bool,
    pub tray: Vec<crate::tray::Item>,
    pub tray_menu: Vec<crate::tray::MenuNode>,
    pub tray_menu_loaded: bool,
    pub wifi: Vec<sys::WifiNet>,
    pub screen_timeout: u32,
    pub capture: Option<crate::capture::ui::CaptureUi>,
    pub media: crate::mpris::State,
    pub recording: Option<RecState>,
    /// Files that notifications from the capture tool point at, by notification id.
    pub cap_notes: std::collections::HashMap<u32, std::path::PathBuf>,
    pub audio: sys::AudioInfo,
    pub battery: Option<sys::BatteryDetail>,
    pub profiles: sys::PowerProfiles,
    pub audio_set_at: Instant,
    pub procs: sys::ProcList,
    pub proc_sampler: sys::ProcSampler,
    pub limits: crate::limits::Limits,
    pub default_apps: crate::defaults::DefaultApps,
}

pub struct App {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    xdg_shell: Option<XdgShell>,
    gfx: Gfx,
    pub wins: Vec<Win>,
    pub shared: Shared,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    kb_focus: Option<wl_surface::WlSurface>,
    mods: egui::Modifiers,
    logo: bool,
    pub started: Instant,
    pub inhibit_mgr: Option<wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1>,
    pub inhibitor: Option<wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1>,
    pub workspace_mgr: Option<wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::ExtWorkspaceManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    loop_handle: LoopHandle<'static, App>,
    qh_store: Option<QueueHandle<App>>,
    tray_cmd: Option<std::sync::mpsc::Sender<crate::tray::Cmd>>,
    slow_poke: Option<std::sync::mpsc::Sender<()>>,
    idle: Option<crate::idle::Idle>,
    audio_tx: Option<channel::Sender<sys::AudioInfo>>,
    cap_tx: Option<channel::Sender<crate::capture::CapMsg>>,
    n_tx: Option<channel::Sender<notif::Event>>,
    mpris_cmd: Option<std::sync::mpsc::Sender<crate::mpris::Cmd>>,
    tick_count: u64,
    prof_tx: Option<channel::Sender<sys::PowerProfiles>>,
    cursor_mgr: Option<WpCursorShapeManagerV1>,
    cursor_dev: Option<WpCursorShapeDeviceV1>,
    /// The surface the pointer is over, the serial of its enter event, and the shape last set.
    pointer_on: Option<(wl_surface::WlSurface, u32)>,
    cursor_shape: Option<Shape>,
    frac_mgr: Option<WpFractionalScaleManagerV1>,
    viewporter: Option<WpViewporter>,
    wifi_tx: Option<channel::Sender<Vec<sys::WifiNet>>>,
    launch_tx: Option<channel::Sender<launch::Data>>,
    next_tick: Instant,
    pub exit: bool,
}

static HEARTBEAT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

extern "C" fn dump_backtrace(_: libc::c_int) {
    eprintln!("watchdog: main thread backtrace:\n{}", std::backtrace::Backtrace::force_capture());
}

/// Debug aid, only with OSMIA_DEBUG. Prints where the main thread is stuck.
fn start_watchdog() {
    // SAFETY: installs a signal handler for SIGUSR1, then reads the thread id.
    let main = unsafe {
        libc::signal(libc::SIGUSR1, dump_backtrace as extern "C" fn(libc::c_int) as libc::sighandler_t);
        libc::pthread_self()
    };
    std::thread::spawn(move || {
        let mut last = 0;
        loop {
            std::thread::sleep(Duration::from_millis(1500));
            let now = HEARTBEAT.load(std::sync::atomic::Ordering::Relaxed);
            let idle_ok = now != last;
            last = now;
            if !idle_ok {
                // The loop wakes at least every two seconds. No change means it is stuck.
                unsafe { libc::pthread_kill(main, libc::SIGUSR1) };
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    });
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("OSMIA_DEBUG").is_some() {
        start_watchdog();
    }
    let loaded = config::load();
    theme_from(&loaded.config);
    crate::appicon::warm_up();

    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let display = NonNull::new(conn.backend().display_ptr() as *mut _).ok_or("no display ptr")?;
    let mut event_loop: EventLoop<App> = EventLoop::try_new()?;

    let mut sampler = Sampler::new();
    let stats = sampler.sample();
    let cfg = loaded.config.clone();
    let mut app = App {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor: CompositorState::bind(&globals, &qh)?,
        layer_shell: LayerShell::bind(&globals, &qh)
            .map_err(|_| "the compositor does not offer wlr-layer-shell")?,
        xdg_shell: XdgShell::bind(&globals, &qh).ok(),
        gfx: Gfx::new(display),
        wins: Vec::new(),
        shared: Shared {
            stats,
            sampler,
            cfg_mtime: config::mtime(),
            notifs: Notifs::new(cfg.notifications.history_size),
            cfg,
            notif_conn: None,
            overlay: None,
            overlay_anchor: 0.0,
            launcher: launcher::State::default(),
            launch_data: None,
            bar_rects: Vec::new(),
            last_output: None,
            toplevels: Vec::new(),
            popup_hover: false,
            tray: Vec::new(),
            tray_menu: Vec::new(),
            tray_menu_loaded: false,
            wifi: Vec::new(),
            wifi_scanning: false,
            screen_timeout: 0,
            procs: Default::default(),
            proc_sampler: Default::default(),
            limits: crate::limits::Limits::load(),
            default_apps: Default::default(),
            workspaces: Vec::new(),
            binds: Vec::new(),
            binds_error: None,
            recording_key: false,
            settings_goto: None,
            captured_key: None,
            capture: None,
            media: Default::default(),
            recording: None,
            cap_notes: Default::default(),
            audio: Default::default(),
            battery: None,
            profiles: Default::default(),
            audio_set_at: Instant::now() - Duration::from_secs(10),
        },
        pointer: None,
        keyboard: None,
        kb_focus: None,
        mods: egui::Modifiers::NONE,
        logo: false,
        started: Instant::now(),
        inhibit_mgr: None,
        inhibitor: None,
        workspace_mgr: None,
        seat: None,
        loop_handle: event_loop.handle(),
        qh_store: Some(qh.clone()),
        tray_cmd: None,
        slow_poke: None,
        idle: None,
        audio_tx: None,
        cap_tx: None,
        n_tx: None,
        mpris_cmd: None,
        tick_count: 0,
        prof_tx: None,
        cursor_mgr: globals.bind::<WpCursorShapeManagerV1, _, _>(&qh, 1..=1, ()).ok(),
        cursor_dev: None,
        pointer_on: None,
        cursor_shape: None,
        frac_mgr: globals.bind::<WpFractionalScaleManagerV1, _, _>(&qh, 1..=1, ()).ok(),
        viewporter: globals.bind::<WpViewporter, _, _>(&qh, 1..=1, ()).ok(),
        wifi_tx: None,
        launch_tx: None,
        next_tick: Instant::now() + SLOW_TICK,
        exit: false,
    };

    // Window list. Optional: without it the attention list and window search stay empty.
    if globals.bind::<ZwlrForeignToplevelManagerV1, _, _>(&qh, 1..=3, ()).is_err() {
        log::warn!("the compositor has no foreign-toplevel protocol. Window list is off.");
    }

    app.inhibit_mgr = globals.bind(&qh, 1..=1, ()).ok();
    app.workspace_mgr = globals
        .bind::<wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::ExtWorkspaceManagerV1, _, _>(&qh, 1..=1, ())
        .ok();

    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle())?;

    // Control socket.
    let (ctl_tx, ctl_rx) = channel::channel::<control::Request>();
    control::serve(ctl_tx)?;
    event_loop.handle().insert_source(ctl_rx, |ev, _, app| {
        if let channel::Event::Msg(req) = ev {
            log::debug!("control: {}", req.line);
            let reply = app.handle_control(&req.line);
            log::debug!("control: replied {}", reply.lines().next().unwrap_or(""));
            let _ = req.reply.send(reply);
        }
    })?;

    // Notification daemon.
    let (n_tx, n_rx) = channel::channel::<notif::Event>();
    app.n_tx = Some(n_tx.clone());
    // A nested test session must not take the notification name from the real shell.
    let started = if std::env::var_os("OSMIA_NESTED").is_some() {
        Err("nested session".to_owned())
    } else {
        notif::start(n_tx, app.shared.cfg.notifications.default_timeout_ms)
    };
    match started {
        Ok(c) => app.shared.notif_conn = Some(c),
        Err(e) => {
            log::warn!("notifications are off: {e}");
            app.shared.notifs.blocked = true;
        }
    }
    event_loop.handle().insert_source(n_rx, |ev, _, app| {
        if let channel::Event::Msg(e) = ev {
            app.on_notif_event(e);
        }
    })?;

    // App list for the launcher, read once in the background.
    let (l_tx, l_rx) = channel::channel::<launch::Data>();
    let l_tx2 = l_tx.clone();
    std::thread::spawn(move || {
        let _ = l_tx.send(launch::Data::load());
    });
    event_loop.handle().insert_source(l_rx, |ev, _, app| {
        if let channel::Event::Msg(d) = ev {
            app.shared.launch_data = Some(d);
            if matches!(app.shared.overlay, Some(Overlay::Launcher)) {
                app.mark_kind(Kind::Overlay);
            }
        }
    })?;
    app.launch_tx = Some(l_tx2);

    // Screen timeout: we run swayidle so the bar can change it.
    let idle = crate::idle::Idle::start();
    app.shared.screen_timeout = idle.minutes;
    app.shared.notifs.dnd = app.shared.cfg.notifications.dnd_on_start;
    app.idle = Some(idle);

    // Volume and network, from background threads.
    let (s_tx, s_rx) = channel::channel::<sys::Slow>();
    app.slow_poke = Some(sys::start_slow(s_tx));
    event_loop.handle().insert_source(s_rx, |ev, _, app| {
        if let channel::Event::Msg(s) = ev {
            log::debug!("slow stats: volume {} network {}", s.volume.is_some(), s.network.is_some());
            if s.volume_only {
                app.shared.sampler.set_volume(s.volume);
            } else {
                app.shared.sampler.set_slow(s.volume, s.network);
            }
            let stats = app.shared.sampler.sample();
            app.shared.stats = stats;
            app.mark_kind(Kind::Bar);
            if matches!(app.shared.overlay, Some(Overlay::Audio)) {
                app.refresh_audio();
            }
        }
    })?;

    let (w_tx, w_rx) = channel::channel::<Vec<sys::WifiNet>>();
    app.wifi_tx = Some(w_tx);
    event_loop.handle().insert_source(w_rx, |ev, _, app| {
        if let channel::Event::Msg(list) = ev {
            app.shared.wifi = list;
            app.shared.wifi_scanning = false;
            app.mark_kind(Kind::Overlay);
        }
    })?;

    let (a_tx, a_rx) = channel::channel::<sys::AudioInfo>();
    app.audio_tx = Some(a_tx);
    event_loop.handle().insert_source(a_rx, |ev, _, app| {
        if let channel::Event::Msg(info) = ev {
            // While a slider is being dragged the answers are stale. Keep the values we set.
            if app.shared.audio_set_at.elapsed() > Duration::from_millis(400) {
                app.shared.audio = info;
                app.mark_kind(Kind::Overlay);
            }
        }
    })?;

    let (c_tx, c_rx) = channel::channel::<crate::capture::CapMsg>();
    app.cap_tx = Some(c_tx);
    event_loop.handle().insert_source(c_rx, |ev, _, app| {
        if let channel::Event::Msg(m) = ev {
            app.on_cap_msg(m);
        }
    })?;

    let (p_tx, p_rx) = channel::channel::<sys::PowerProfiles>();
    app.prof_tx = Some(p_tx);
    event_loop.handle().insert_source(p_rx, |ev, _, app| {
        if let channel::Event::Msg(p) = ev {
            app.shared.profiles = p;
            app.mark_kind(Kind::Overlay);
        }
    })?;

    // Media players.
    let (m_tx, m_rx) = channel::channel::<crate::mpris::Msg>();
    app.mpris_cmd = Some(crate::mpris::start(m_tx));
    event_loop.handle().insert_source(m_rx, |ev, _, app| {
        if let channel::Event::Msg(m) = ev {
            use crate::mpris::Msg;
            match m {
                Msg::Upsert(p) => app.shared.media.upsert(p),
                Msg::Remove(b) => app.shared.media.remove(&b),
                Msg::Art(url, img) => {
                    app.shared.media.art.insert(url, std::sync::Arc::new(img));
                }
            }
            app.mark_kind(Kind::Bar);
            app.mark_kind(Kind::Overlay);
        }
    })?;

    // System tray.
    let (t_tx, t_rx) = channel::channel::<crate::tray::Msg>();
    if std::env::var_os("OSMIA_NESTED").is_none() {
        app.tray_cmd = Some(crate::tray::start(t_tx));
    }
    event_loop.handle().insert_source(t_rx, |ev, _, app| {
        if let channel::Event::Msg(m) = ev {
            app.on_tray_msg(m);
        }
    })?;

    for w in &loaded.warnings {
        log::warn!("{w}");
        app.notify_self("Config warning", w);
    }
    if let Some(e) = &loaded.error {
        log::warn!("config error: {e}");
        let first = e.lines().next().unwrap_or("");
        app.notify_self("Config not applied", &format!("Using defaults. {first}"));
    }

    while !app.exit {
        HEARTBEAT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let now = Instant::now();
        let mut wake = app.next_tick;
        for w in &app.wins {
            if let Some(d) = w.deadline() {
                wake = wake.min(d);
            }
        }
        if !app.shared.popup_hover {
            if let Some(t) = app.shared.notifs.next_expiry() {
                wake = wake.min(t);
            }
        }
        let timeout = wake.saturating_duration_since(now);
        let t0 = Instant::now();
        event_loop.dispatch(Some(timeout), &mut app)?;
        let t1 = Instant::now();
        app.after_dispatch(&qh);
        let (d1, d2) = (t1 - t0, t1.elapsed());
        if d2 > Duration::from_millis(200) || (d1 > timeout + Duration::from_millis(200)) {
            log::warn!("loop stall: dispatch took {d1:?} (timeout {timeout:?}), after_dispatch took {d2:?}");
        }
    }
    let _ = std::fs::remove_file(control::socket_path());
    // Painters hold raw handles. Drop them before the connection goes away.
    app.wins.clear();
    Ok(())
}

/// Ids for the shell's own notifications. They stay far above the daemon's counter.
/// The text for the bar while recording: a countdown, the time so far, or "Saving".
fn recording_label(r: Option<&RecState>) -> Option<String> {
    let r = r?;
    if r.stopping {
        return Some("Saving".into());
    }
    let now = Instant::now();
    if r.rec.started > now {
        return Some(format!("Starting in {}", (r.rec.started - now).as_secs() + 1));
    }
    let s = (now - r.rec.started).as_secs();
    Some(format!("{:02}:{:02}", s / 60, s % 60))
}

fn next_self_id() -> u32 {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x7000_0000);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn theme_from(cfg: &Config) {
    crate::theme::set_font_size(cfg.theme.font_size);
    crate::appicon::set_theme(&cfg.theme.icon_theme);
    if let Some(c) = config::parse_color(&cfg.theme.accent) {
        crate::theme::set_accent(c);
    }
}

impl App {
    // ---------- loop work ----------

    fn after_dispatch(&mut self, qh: &QueueHandle<App>) {
        let now = Instant::now();
        self.expire_popups(now);
        if now >= self.next_tick {
            // A clock that shows seconds needs a faster tick.
            let f = &self.shared.cfg.bar.clock_format;
            let fast = f.contains("%S") || f.contains("%T") || f.contains("%s");
            self.tick_count += 1;
            if self.tick_count % 5 == 0 {
                self.check_notification_name();
            }
            let recording = self.shared.recording.is_some();
            let watching = matches!(self.shared.overlay, Some(Overlay::Cpu | Overlay::Ram));
            self.next_tick = now + if fast || recording || watching { Duration::from_secs(1) } else { SLOW_TICK };
            if recording {
                self.mark_kind(Kind::Bar);
            }
            let s = self.shared.sampler.sample();
            if s != self.shared.stats {
                self.shared.stats = s;
                self.mark_kind(Kind::Bar);
                if matches!(self.shared.overlay, Some(Overlay::Screen)) {
                    self.mark_kind(Kind::Overlay);
                }
            }
            if watching {
                let lock = self.shared.procs.lock.take();
                self.shared.procs = self.shared.proc_sampler.sample();
                self.shared.procs.lock = lock;
                self.shared.limits.enforce(&self.shared.procs.all);
                self.shared.procs.limits = self.shared.limits.set.clone();
                self.mark_kind(Kind::Overlay);
            } else if !self.shared.limits.set.is_empty() && self.tick_count % 5 == 1 {
                // Catch new processes of limited programs.
                let p = self.shared.proc_sampler.sample();
                self.shared.limits.enforce(&p.all);
            }
            if matches!(self.shared.overlay, Some(Overlay::Battery)) {
                let b = sys::read_battery_detail();
                if b != self.shared.battery {
                    self.shared.battery = b;
                    self.mark_kind(Kind::Overlay);
                }
            }
            self.check_clock();
            self.check_config(qh);
        }
        self.sync_popups(qh);
        for i in 0..self.wins.len() {
            if self.wins[i].wants_draw(now) {
                let t0 = Instant::now();
                let kind = self.wins[i].kind;
                self.draw(i, qh);
                let ms = t0.elapsed().as_millis();
                if ms > 40 {
                    log::warn!("slow draw: {kind:?} took {ms} ms");
                }
            }
        }
    }

    fn check_clock(&mut self) {
        thread_local! { static LAST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
        let now = chrono::Local::now().format(&self.shared.cfg.bar.clock_format).to_string();
        let changed = LAST.with(|l| std::mem::replace(&mut *l.borrow_mut(), now.clone()) != now);
        if changed {
            self.mark_kind(Kind::Bar);
        }
    }

    fn check_config(&mut self, qh: &QueueHandle<App>) {
        let m = config::mtime();
        if m != self.shared.cfg_mtime {
            self.shared.cfg_mtime = m;
            self.reload_config(qh);
        }
    }

    /// Returns a message for the user: "ok" or a problem.
    fn reload_config(&mut self, qh: &QueueHandle<App>) -> String {
        let loaded = config::load();
        self.shared.cfg_mtime = config::mtime();
        if let Some(e) = loaded.error {
            let first = e.lines().next().unwrap_or("").to_owned();
            self.notify_self("Config not applied", &format!("Kept the last good config. {first}"));
            self.mark_kind(Kind::Popups);
            return format!("error: {e}");
        }
        for w in &loaded.warnings {
            self.notify_self("Config warning", w);
        }
        let layout_changed = {
            let (a, b) = (&self.shared.cfg.bar, &loaded.config.bar);
            a.edge != b.edge || a.height != b.height
        };
        let theme_changed = self.shared.cfg.theme.icon_theme != loaded.config.theme.icon_theme;
        let corner_changed = self.shared.cfg.notifications.corner != loaded.config.notifications.corner;
        notif::set_default_timeout(loaded.config.notifications.default_timeout_ms);
        theme_from(&loaded.config);
        for w in &self.wins {
            crate::theme::apply_text_styles(&w.ctx);
        }
        self.mark_all();
        self.shared.notifs.history_cap = loaded.config.notifications.history_size;
        self.shared.cfg = loaded.config;
        if layout_changed {
            self.rebuild_bars(qh);
        }
        if corner_changed {
            // The popup window is anchored when it is made. Make it again at the new corner.
            self.wins.retain(|w| w.kind != Kind::Popups);
            self.shared.popup_hover = false;
            self.mark_kind(Kind::Popups);
        }
        if theme_changed {
            if let Some(tx) = &self.tray_cmd {
                let _ = tx.send(crate::tray::Cmd::Refresh);
            }
        }
        self.mark_kind(Kind::Overlay);
        self.mark_kind(Kind::Settings);
        self.mark_kind(Kind::Bar);
        "ok".into()
    }

    /// The shell tells the user about itself as a normal popup.
    fn notify_self(&mut self, summary: &str, body: &str) {
        let n = Notification {
            id: next_self_id(),
            app_name: "osmia".into(),
            app_icon: String::new(),
            summary: summary.into(),
            body: body.into(),
            actions: vec![],
            urgency: 1,
            desktop_entry: None,
            transient: true,
            timeout: Some(Duration::from_secs(8)),
            born: Instant::now(),
            unread: false,
        };
        self.shared.notifs.add(n, 0);
    }

    fn mark_kind(&mut self, kind: Kind) {
        for w in &mut self.wins {
            if w.kind == kind {
                w.dirty = true;
            }
        }
    }

    fn mark_all(&mut self) {
        for w in &mut self.wins {
            w.dirty = true;
        }
    }

    fn win_index(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.wins.iter().position(|w| w.surf.wl_surface() == surface)
    }

    /// The workspace list or the active workspace changed.
    pub fn on_workspaces_changed(&mut self) {
        let cur = crate::workspace::current(&self.shared.workspaces);
        if let Some(cur) = cur {
            // A window focused just before this arrived was on the new workspace.
            let now = Instant::now();
            for t in &mut self.shared.toplevels {
                if t.ws_at.is_some_and(|at| now.duration_since(at) < Duration::from_millis(150)) {
                    t.ws = Some(cur.clone());
                }
            }
        }
        self.mark_kind(Kind::Bar);
        if matches!(self.shared.overlay, Some(Overlay::Windows)) {
            self.mark_kind(Kind::Overlay);
        }
    }

    pub fn on_toplevels_changed(&mut self) {
        self.mark_kind(Kind::Bar);
        if matches!(self.shared.overlay, Some(Overlay::Launcher)) {
            self.mark_kind(Kind::Overlay);
        }
    }

    // ---------- windows ----------

    fn bar_on_top(&self) -> bool {
        self.shared.cfg.bar.edge != "bottom"
    }

    /// A window, with fractional scaling when the compositor offers it.
    fn new_win(&self, qh: &QueueHandle<App>, kind: Kind, surf: Surf, output: Option<wl_output::WlOutput>) -> Win {
        let mut w = Win::new(kind, surf, output);
        if let (Some(m), Some(v)) = (&self.frac_mgr, &self.viewporter) {
            let surface = w.surf.wl_surface().clone();
            let f = m.get_fractional_scale(&surface, qh, surface.clone());
            let vp = v.get_viewport(&surface, qh, ());
            w.frac = Some((f, vp));
        }
        w
    }

    fn add_bar(&mut self, qh: &QueueHandle<App>, output: wl_output::WlOutput) {
        let h = self.shared.cfg.bar.height;
        let surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(qh, surface, Layer::Top, Some("osmia-bar"), Some(&output));
        let edge = if self.bar_on_top() { Anchor::TOP } else { Anchor::BOTTOM };
        layer.set_anchor(edge | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, h);
        layer.set_exclusive_zone(h as i32);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();
        self.wins.push(self.new_win(qh, Kind::Bar, Surf::Layer(layer), Some(output)));
    }

    fn rebuild_bars(&mut self, qh: &QueueHandle<App>) {
        self.wins.retain(|w| w.kind != Kind::Bar);
        let outputs: Vec<_> = self.output_state.outputs().collect();
        for o in outputs {
            self.add_bar(qh, o);
        }
    }

    fn focused_output(&self) -> Option<wl_output::WlOutput> {
        self.shared
            .last_output
            .clone()
            .or_else(|| self.output_state.outputs().next())
    }

    /// Show the popup surface while there are popups. Remove it when empty.
    fn sync_popups(&mut self, qh: &QueueHandle<App>) {
        let want = !self.shared.notifs.popups.is_empty();
        let has = self.wins.iter().any(|w| w.kind == Kind::Popups);
        if want && !has {
            let output = self.focused_output();
            let surface = self.compositor.create_surface(qh);
            let layer = self.layer_shell.create_layer_surface(qh, surface, Layer::Overlay, Some("osmia-popups"), output.as_ref());
            let corner = self.shared.cfg.notifications.corner.clone();
            let vert = if corner.starts_with("top") { Anchor::TOP } else { Anchor::BOTTOM };
            let horiz = if corner.ends_with("left") { Anchor::LEFT } else { Anchor::RIGHT };
            layer.set_anchor(vert | Anchor::TOP | Anchor::BOTTOM | horiz);
            layer.set_size((popups::WIDTH + popups::MARGIN * 2.0) as u32, 0);
            layer.set_exclusive_zone(0);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.commit();
            self.wins.push(self.new_win(qh, Kind::Popups, Surf::Layer(layer), output));
        } else if !want && has {
            self.wins.retain(|w| w.kind != Kind::Popups);
            self.shared.popup_hover = false;
        } else if want {
            self.mark_kind(Kind::Popups);
        }
    }

    fn load_binds(&mut self) {
        match crate::keybinds::load_file() {
            Ok(b) => {
                self.shared.binds = b;
                self.shared.binds_error = None;
            }
            Err(e) => {
                self.shared.binds.clear();
                self.shared.binds_error = Some(e);
            }
        }
    }

    /// Apply one edit to rc.xml, save it and tell labwc to read it again.
    fn edit_binds(&mut self, f: impl FnOnce(&str) -> Result<String, String>) {
        let p = crate::keybinds::path();
        let result = std::fs::read_to_string(&p)
            .map_err(|e| format!("{}: {e}", p.display()))
            .and_then(|t| f(&t).map(|n| (t, n)));
        match result {
            Ok((old, new)) => {
                let bak = p.with_extension("xml.bak-osmia");
                if !bak.exists() {
                    let _ = std::fs::write(&bak, &old);
                }
                let tmp = p.with_extension("xml.tmp");
                if let Err(e) = std::fs::write(&tmp, &new).and_then(|_| std::fs::rename(&tmp, &p)) {
                    self.shared.binds_error = Some(format!("Could not save: {e}"));
                } else {
                    reconfigure_labwc();
                    self.load_binds();
                    self.shared.binds_error = None;
                }
            }
            Err(e) => self.shared.binds_error = Some(e),
        }
        self.mark_kind(Kind::Settings);
    }

    fn open_settings(&mut self, qh: &QueueHandle<App>) {
        self.load_binds();
        self.shared.default_apps = crate::defaults::load();
        if self.wins.iter().any(|w| w.kind == Kind::Settings) {
            self.mark_kind(Kind::Settings);
            return;
        }
        let Some(xdg) = &self.xdg_shell else {
            self.notify_self("Settings", "The compositor has no xdg-shell. Edit config.toml by hand.");
            self.mark_kind(Kind::Popups);
            return;
        };
        let surface = self.compositor.create_surface(qh);
        let window = xdg.create_window(surface, WindowDecorations::RequestServer, qh);
        window.set_title("Osmia settings");
        window.set_app_id("osmia-settings");
        window.set_min_size(Some((640, 420)));
        window.commit();
        let w = self.new_win(qh, Kind::Settings, Surf::Window(window), None);
        self.wins.push(w);
    }

    fn ensure_overlay(&mut self, qh: &QueueHandle<App>) {
        if self.wins.iter().any(|w| w.kind == Kind::Overlay) {
            self.mark_kind(Kind::Overlay);
            return;
        }
        let output = self.focused_output();
        let surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(qh, surface, Layer::Overlay, Some("osmia-overlay"), output.as_ref());
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.commit();
        self.wins.push(self.new_win(qh, Kind::Overlay, Surf::Layer(layer), output));
    }

    fn open_overlay(&mut self, qh: &QueueHandle<App>, ov: Overlay, anchor: f32) {
        match &ov {
            Overlay::History => {
                for h in self.shared.notifs.history.iter_mut() {
                    h.unread = false;
                }
            }
            Overlay::Audio => self.refresh_audio(),
            Overlay::Cpu | Overlay::Ram => {
                // A baseline now, so the first tick can show a difference.
                self.shared.proc_sampler.reset();
                self.shared.procs = self.shared.proc_sampler.sample();
                self.shared.procs.lock = None;
                self.shared.procs.limits = self.shared.limits.set.clone();
                self.next_tick = self.next_tick.min(Instant::now() + Duration::from_millis(1000));
            }
            Overlay::Battery => {
                self.shared.battery = sys::read_battery_detail();
                if let Some(tx) = self.prof_tx.clone() {
                    sys::profiles_run(None, tx);
                }
            }
            Overlay::Network => {
                self.shared.wifi.clear();
                self.shared.wifi_scanning = true;
                if let Some(tx) = &self.wifi_tx {
                    sys::scan_wifi(tx.clone());
                }
            }
            Overlay::Launcher => {
                // Use the app list we already have. Refresh it in the background.
                if let Some(tx) = self.launch_tx.clone() {
                    std::thread::spawn(move || {
                        let _ = tx.send(launch::Data::load());
                    });
                }
                self.shared.launcher = launcher::State::default();
            }
            _ => {}
        }
        if !anchor.is_nan() {
            self.shared.overlay_anchor = anchor;
        }
        self.shared.overlay = Some(ov);
        self.ensure_overlay(qh);
        self.mark_kind(Kind::Bar);
    }

    fn close_overlay(&mut self) {
        self.shared.overlay = None;
        self.wins.retain(|w| w.kind != Kind::Overlay);
        self.kb_focus = None;
        self.mark_kind(Kind::Bar);
    }

    fn on_tray_msg(&mut self, m: crate::tray::Msg) {
        use crate::tray::Msg;
        match m {
            Msg::Upsert(item) => {
                match self.shared.tray.iter_mut().find(|i| i.id == item.id) {
                    Some(slot) => *slot = item,
                    None => self.shared.tray.push(item),
                }
                self.mark_kind(Kind::Bar);
            }
            Msg::Remove(id) => {
                self.shared.tray.retain(|i| i.id != id);
                self.mark_kind(Kind::Bar);
            }
            Msg::ActivateFailed(id) => self.launch_tray_app(&id),
            Msg::Menu(_id, nodes) => {
                self.shared.tray_menu = nodes;
                self.shared.tray_menu_loaded = true;
                self.mark_kind(Kind::Overlay);
            }
        }
    }

    // ---------- notifications ----------

    fn on_notif_event(&mut self, e: notif::Event) {
        match e {
            notif::Event::Notify(n, replaces) => {
                self.match_alert(&n);
                self.shared.notifs.add(n, replaces);
                self.mark_kind(Kind::Bar);
                self.mark_kind(Kind::Popups);
                if matches!(self.shared.overlay, Some(Overlay::History)) {
                    self.mark_kind(Kind::Overlay);
                }
            }
            notif::Event::Close(id) => {
                self.shared.notifs.remove_popup(id, false);
                self.emit_closed(id, notif::REASON_CLOSED_BY_APP);
                self.mark_kind(Kind::Popups);
            }
        }
    }

    fn emit_closed(&self, id: u32, reason: u32) {
        if let Some(c) = &self.shared.notif_conn {
            notif::emit_closed(c, id, reason);
        }
    }

    fn expire_popups(&mut self, now: Instant) {
        if self.shared.popup_hover {
            return;
        }
        let ids = self.shared.notifs.expired(now, false);
        for id in ids {
            self.shared.notifs.remove_popup(id, true);
            self.emit_closed(id, notif::REASON_EXPIRED);
            self.mark_kind(Kind::Popups);
            self.mark_kind(Kind::Bar);
        }
    }

    /// Find the window a notification belongs to: desktop-entry hint first,
    /// then the alerts table, then the app name.
    /// Names a tray item and an app might share, lowercase and without suffixes.
    fn tray_names(&self, id: &str) -> Vec<String> {
        let Some(it) = self.shared.tray.iter().find(|i| i.id == id) else { return vec![] };
        let norm = |s: &str| s.trim().trim_end_matches(".desktop").to_lowercase();
        // The item id for a registered item is "bus/path". Its Title and Id property is what matters.
        let mut v: Vec<String> = vec![norm(&it.title), norm(it.id.rsplit('/').next().unwrap_or(""))];
        v.extend(it.app_key.split('\n').map(norm));
        v.retain(|s| s.len() >= 3);
        v.dedup();
        v
    }

    /// Two names refer to the same app when they are equal or one starts with the other.
    fn same_app(a: &str, b: &str) -> bool {
        let (a, b) = (a.trim_end_matches(".desktop").to_lowercase(), b.trim_end_matches(".desktop").to_lowercase());
        a.len() >= 4 && b.len() >= 4 && (a == b || a.starts_with(&b) || b.starts_with(&a))
    }

    fn tray_toplevel(&self, id: &str) -> Option<usize> {
        let names = self.tray_names(id);
        self.shared.toplevels.iter().position(|t| t.ready && names.iter().any(|n| Self::same_app(n, &t.app_id)))
    }

    /// "Open" on a media player always opens something. Many players, browsers
    /// among them, say they cannot be raised over MPRIS, so go by window first,
    /// then ask the player, then start the app from its desktop entry.
    /// While recording keys, ask the compositor to stop acting on its own
    /// shortcuts so the settings window hears them.
    fn set_inhibit(&mut self, on: bool, qh: &QueueHandle<Self>) {
        if let Some(i) = self.inhibitor.take() {
            i.destroy();
        }
        if !on {
            return;
        }
        let (Some(m), Some(seat)) = (&self.inhibit_mgr, &self.seat) else { return };
        let Some(w) = self.wins.iter().find(|w| w.kind == Kind::Settings) else { return };
        self.inhibitor = Some(m.inhibit_shortcuts(w.surf.wl_surface(), seat, qh, ()));
    }

    fn open_player(&mut self, bus: &str) {
        let Some(p) = self.shared.media.players.iter().find(|p| p.bus == bus).cloned() else { return };
        let short = bus.strip_prefix("org.mpris.MediaPlayer2.").unwrap_or(bus);
        let short = short.split(".instance").next().unwrap_or(short).to_owned();
        let names: Vec<String> = [short, p.identity.clone(), p.desktop_entry.trim_end_matches(".desktop").to_owned()]
            .into_iter()
            .filter(|n| n.len() >= 3)
            .collect();
        let win = self.shared.toplevels.iter().position(|t| t.ready && names.iter().any(|n| Self::same_app(n, &t.app_id)));
        if let Some(i) = win {
            self.focus_toplevel(i);
            return;
        }
        self.media_cmd(crate::mpris::Cmd::Raise(bus.to_owned()));
        let term = self.shared.cfg.launcher.terminal.clone();
        let apps = match &self.shared.launch_data {
            Some(d) => d.apps.clone(),
            None => launch::Data::load().apps,
        };
        let hit = apps.iter().find(|a| names.iter().any(|n| Self::same_app(n, &a.id) || Self::same_app(n, &a.name)));
        match hit {
            Some(app) => launch::run_app(app, &term),
            None => {
                self.notify_self("Cannot open", &format!("No window or desktop entry found for {}.", p.identity));
            }
        }
    }

    /// The app would not open from the tray: start it from its desktop entry.
    fn launch_tray_app(&mut self, id: &str) {
        let names = self.tray_names(id);
        let term = self.shared.cfg.launcher.terminal.clone();
        let apps = match &self.shared.launch_data {
            Some(d) => d.apps.clone(),
            None => launch::Data::load().apps,
        };
        let hit = apps.iter().find(|a| names.iter().any(|n| Self::same_app(n, &a.id) || Self::same_app(n, &a.name)));
        match hit {
            Some(app) => launch::run_app(app, &term),
            None => {
                self.notify_self("Cannot open", "This app did not respond and has no desktop entry to start it.");
                self.mark_kind(Kind::Popups);
            }
        }
    }

    fn find_toplevel(&self, n: &Notification) -> Option<usize> {
        let tl = &self.shared.toplevels;
        let norm = |s: &str| s.trim_end_matches(".desktop").to_lowercase();
        if let Some(de) = &n.desktop_entry {
            let de = norm(de);
            if let Some(i) = tl.iter().position(|t| norm(&t.app_id) == de) {
                return Some(i);
            }
        }
        if let Some(id) = self.shared.cfg.alerts.get(&n.app_name) {
            let id = norm(id);
            if let Some(i) = tl.iter().position(|t| norm(&t.app_id) == id) {
                return Some(i);
            }
        }
        let name = n.app_name.to_lowercase();
        if name.is_empty() {
            return None;
        }
        tl.iter().position(|t| norm(&t.app_id) == name)
    }

    fn match_alert(&mut self, n: &Notification) {
        if let Some(i) = self.find_toplevel(n) {
            let t = &mut self.shared.toplevels[i];
            if !t.activated {
                t.alert_ids.push(n.id);
                t.refresh_attention();
            }
        }
    }

    fn clear_alert(&mut self, id: u32) {
        for t in &mut self.shared.toplevels {
            t.alert_ids.retain(|a| *a != id);
            t.refresh_attention();
        }
    }

    fn focus_toplevel(&mut self, i: usize) {
        let seat = self.seat.clone();
        let Some(t) = self.shared.toplevels.get(i) else { return };
        log::debug!("focus window {i} ({:?}), minimized={}, seat={}", t.title, t.minimized, seat.is_some());
        // Restore first. This does not need a seat.
        if t.minimized {
            t.handle.unset_minimized();
        }
        if let Some(seat) = seat {
            t.handle.activate(&seat);
        }
    }

    // ---------- control socket ----------

    fn handle_control(&mut self, line: &str) -> String {
        let qh = self.qh();
        let parts: Vec<&str> = line.split_whitespace().collect();
        let json = parts.contains(&"--json");
        let parts: Vec<&str> = parts.into_iter().filter(|p| *p != "--json").collect();
        let has = |o: &Overlay| self.shared.overlay.as_ref().is_some_and(|c| c.same_kind(o));
        match parts.as_slice() {
            ["launcher", act @ ("toggle" | "show" | "hide")] => {
                let open = has(&Overlay::Launcher);
                match (*act, open) {
                    ("toggle", true) | ("hide", true) => self.close_overlay(),
                    ("toggle", false) | ("show", false) => self.open_overlay(&qh, Overlay::Launcher, f32::NAN),
                    _ => {}
                }
                "ok".into()
            }
            ["network"] => {
                if has(&Overlay::Network) {
                    self.close_overlay();
                } else {
                    let anchor = self.anchor_for("network");
                    self.open_overlay(&qh, Overlay::Network, anchor);
                }
                "ok".into()
            }
            ["cpu"] => {
                if has(&Overlay::Cpu) {
                    self.close_overlay();
                } else {
                    let anchor = self.anchor_for("cpu");
                    self.open_overlay(&qh, Overlay::Cpu, anchor);
                }
                "ok".into()
            }
            ["ram"] => {
                if has(&Overlay::Ram) {
                    self.close_overlay();
                } else {
                    let anchor = self.anchor_for("ram");
                    self.open_overlay(&qh, Overlay::Ram, anchor);
                }
                "ok".into()
            }
            ["windows"] => {
                if has(&Overlay::Windows) {
                    self.close_overlay();
                } else {
                    let anchor = self.anchor_for("windows");
                    self.open_overlay(&qh, Overlay::Windows, anchor);
                }
                "ok".into()
            }
            ["capture", rest @ ..] => self.control_capture(&qh, rest),
            ["media", rest @ ..] => self.control_media(rest),
            ["history", act @ ("toggle" | "show" | "hide")] => {
                let open = has(&Overlay::History);
                let anchor = self.anchor_for("notifications");
                match (*act, open) {
                    ("toggle", true) | ("hide", true) => self.close_overlay(),
                    ("toggle", false) | ("show", false) => self.open_overlay(&qh, Overlay::History, anchor),
                    _ => {}
                }
                "ok".into()
            }
            ["history", "clear"] => {
                self.shared.notifs.history.clear();
                self.mark_all();
                "ok".into()
            }
            ["dnd", act @ ("on" | "off" | "toggle" | "status")] => {
                match *act {
                    "on" => self.shared.notifs.dnd = true,
                    "off" => self.shared.notifs.dnd = false,
                    "toggle" => self.shared.notifs.dnd = !self.shared.notifs.dnd,
                    _ => {}
                }
                self.mark_all();
                if self.shared.notifs.dnd { "on".into() } else { "off".into() }
            }
            ["popups", "clear"] => {
                self.clear_popups();
                "ok".into()
            }
            // Test aid for nested sessions, where the tray is off: a fake tray item
            // with a long menu, opened the way a click would, menu arriving after.
            ["demo-menu", n] if std::env::var_os("OSMIA_NESTED").is_some() => {
                let n: i32 = n.parse().unwrap_or(40);
                let nodes = (0..n)
                    .map(|i| crate::tray::MenuNode { id: i + 1, label: format!("Country number {i}"), enabled: true, ..Default::default() })
                    .collect();
                self.on_tray_msg(crate::tray::Msg::Menu("demo".into(), nodes));
                "ok".into()
            }
            ["demo-tray", n] | ["demo-tray", n, _] if std::env::var_os("OSMIA_NESTED").is_some() => {
                let later = parts.len() > 2;
                let n: i32 = n.parse().unwrap_or(40);
                self.shared.tray.push(crate::tray::Item {
                    id: "demo".into(),
                    bus: "demo".into(),
                    path: "/".into(),
                    title: "Demo VPN".into(),
                    icon: None,
                    menu: Some("/menu".into()),
                    version: 0,
                    status: "Active".into(),
                    item_is_menu: true,
                    tip_title: String::new(),
                    tip_text: String::new(),
                    app_key: "demo".into(),
                });
                self.shared.tray_menu_loaded = false;
                self.shared.tray_menu = Vec::new();
                let anchor = self.shared.bar_rects.last().map(|r| r.1.center().x).unwrap_or(1200.0);
                self.open_overlay(&qh, Overlay::TrayMenu("demo".into()), anchor);
                if !later {
                    let nodes = (0..n)
                        .map(|i| crate::tray::MenuNode { id: i + 1, label: format!("Country number {i}"), enabled: true, ..Default::default() })
                        .collect();
                    self.on_tray_msg(crate::tray::Msg::Menu("demo".into(), nodes));
                }
                "ok".into()
            }
            ["settings"] | ["settings", _] => {
                self.shared.settings_goto = parts.get(1).map(|p| p.to_string());
                self.close_overlay();
                self.open_settings(&qh);
                "ok".into()
            }
            ["config", "reload"] => self.reload_config(&qh),
            ["config", "check"] => match std::fs::read_to_string(config::path()) {
                Ok(t) => match config::parse(&t) {
                    Ok((_, w)) if w.is_empty() => "ok".into(),
                    Ok((_, w)) => format!("ok\n{}", w.join("\n")),
                    Err(e) => format!("error: {e}"),
                },
                Err(_) => "ok (no config file, using defaults)".into(),
            },
            ["debug", "click", x, y, rest @ ..] if std::env::var_os("OSMIA_DEBUG").is_some() => {
                // Test hook: click at x,y in the top window (overlay first, then the bar).
                let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>()) else { return "error: bad coordinates".into() };
                let button = if rest.first() == Some(&"right") { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
                let target = self.wins.iter().position(|w| w.kind == Kind::Overlay).or_else(|| self.wins.iter().position(|w| w.kind == Kind::Bar));
                if let Some(i) = target {
                    let pos = egui::pos2(x, y);
                    let w = &mut self.wins[i];
                    w.events.push(egui::Event::PointerMoved(pos));
                    w.events.push(egui::Event::PointerButton { pos, button, pressed: true, modifiers: egui::Modifiers::NONE });
                    w.events.push(egui::Event::PointerButton { pos, button, pressed: false, modifiers: egui::Modifiers::NONE });
                    w.dirty = true;
                }
                "ok".into()
            }
            ["debug", "windows"] if std::env::var_os("OSMIA_DEBUG").is_some() => self
                .shared
                .toplevels
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{i} {:?} app={:?} minimized={} activated={} alert={}", t.title, t.app_id, t.minimized, t.activated, t.in_alert()))
                .collect::<Vec<_>>()
                .join("\n"),
            ["debug", "minimize", n] if std::env::var_os("OSMIA_DEBUG").is_some() => {
                if let Some(t) = n.parse::<usize>().ok().and_then(|i| self.shared.toplevels.get(i)) {
                    t.handle.set_minimized();
                }
                "ok".into()
            }
            ["debug", "tray"] if std::env::var_os("OSMIA_DEBUG").is_some() => {
                self.shared.tray.iter().map(|t| format!("{} {}", t.id, t.title)).collect::<Vec<_>>().join("\n")
            }
            ["status"] => self.status(json),
            ["version"] => format!("osmia {}", env!("CARGO_PKG_VERSION")),
            _ => format!("error: unknown command: {line}"),
        }
    }

    fn status(&self, json: bool) -> String {
        let monitors: Vec<String> = self
            .output_state
            .outputs()
            .filter_map(|o| self.output_state.info(&o).and_then(|i| i.name))
            .collect();
        let n = &self.shared.notifs;
        if json {
            serde_json::json!({
                "dnd": n.dnd,
                "popups": n.popups.len(),
                "history": n.history.len(),
                "monitors": monitors,
            })
            .to_string()
        } else {
            format!(
                "dnd: {}\npopups: {}\nhistory: {}\nmonitors: {}",
                if n.dnd { "on" } else { "off" },
                n.popups.len(),
                n.history.len(),
                monitors.join(", ")
            )
        }
    }

    fn anchor_for(&self, id: &str) -> f32 {
        self.shared
            .bar_rects
            .iter()
            .find(|(n, _)| n == id)
            .map(|(_, r)| r.center().x)
            .unwrap_or(f32::MAX)
    }

    fn qh(&self) -> QueueHandle<App> {
        self.qh_store.clone().expect("queue handle set")
    }

    fn clear_popups(&mut self) {
        let ids: Vec<u32> = self.shared.notifs.popups.iter().map(|p| p.id).collect();
        for id in ids {
            self.shared.notifs.remove_popup(id, false);
            self.emit_closed(id, notif::REASON_DISMISSED);
        }
        self.mark_kind(Kind::Popups);
        self.mark_kind(Kind::Bar);
    }

    // ---------- drawing ----------

    fn draw(&mut self, i: usize, qh: &QueueHandle<App>) {
        let App { gfx, wins, shared, compositor, output_state, .. } = self;
        let w = &mut wins[i];
        // Titles and messages can hold characters our fonts lack. Find fonts for them.
        {
            let titles = shared.toplevels.iter().flat_map(|t| [t.title.as_str(), t.app_id.as_str()]);
            let notifs = shared
                .notifs
                .popups
                .iter()
                .chain(shared.notifs.history.iter().take(60))
                .flat_map(|n| [n.summary.as_str(), n.body.as_str(), n.app_name.as_str()]);
            let tray = shared.tray.iter().flat_map(|t| [t.title.as_str(), t.tip_title.as_str(), t.tip_text.as_str()]);
            if crate::theme::ensure_glyphs(&w.ctx, titles.chain(notifs).chain(tray)) {
                w.dirty = true;
            }
        }
        let (lw, lh) = w.size;
        if lw == 0 || lh == 0 {
            return;
        }
        let px = w.px();
        if w.painter.is_none() {
            let ptr = NonNull::new(w.surf.wl_surface().id().as_ptr() as *mut _).unwrap();
            match gfx.make_painter(ptr, px.0, px.1) {
                Ok(p) => w.painter = Some(p),
                Err(e) => {
                    log::error!("cannot create painter: {e}");
                    w.dirty = false;
                    return;
                }
            }
        }

        let ppp = w.scale;
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(lw as f32, lh as f32))),
            time: Some(w.start.elapsed().as_secs_f64()),
            events: std::mem::take(&mut w.events),
            viewports: std::iter::once((
                egui::ViewportId::ROOT,
                egui::ViewportInfo { native_pixels_per_point: Some(ppp), ..Default::default() },
            ))
            .collect(),
            ..Default::default()
        };

        let now = chrono::Local::now();
        let mut actions: Vec<Action> = Vec::new();
        let mut input_rects: Option<Vec<egui::Rect>> = None;
        let mut hovered = false;
        let kind = w.kind;
        let bar_top = shared.cfg.bar.edge != "bottom";
        let out_h = lh as f32;
        // Which output a capture overlay window sits on, as an index into the shots.
        let cap_idx = if kind == Kind::Capture {
            let name = w.output.as_ref().and_then(|o| output_state.info(o)).and_then(|i| i.name);
            shared.capture.as_ref().and_then(|c| c.shots.iter().position(|s| Some(&s.name) == name.as_ref()))
        } else {
            None
        };
        let out = w.ctx.run_ui(raw, |ui| match kind {
            Kind::Bar => {
                let attention = attention_entries(&shared.toplevels);
                let mut rects = Vec::new();
                bar::show(
                    ui,
                    &bar::Input {
                        stats: &shared.stats,
                        clock_date: if shared.cfg.bar.date_format.is_empty() { String::new() } else { now.format(&shared.cfg.bar.date_format).to_string() },
                        clock_time: now.format(&shared.cfg.bar.clock_format).to_string(),
                        dnd: shared.notifs.dnd,
                        unread: shared.notifs.unread(),
                        notif_blocked: shared.notifs.blocked,
                        attention: &attention,
                        recording: recording_label(shared.recording.as_ref()),
                        media: shared.media.current().map(|p| (p.line(), p.status == crate::mpris::Status::Playing, p.bus.clone())),
                        layout: &shared.cfg.bar.layout,
                        tray: &shared.tray,
                        top: bar_top,
                    },
                    &mut actions,
                    &mut rects,
                );
                shared.bar_rects = rects;
            }
            Kind::Popups => {
                let bottom_up = !shared.cfg.notifications.corner.starts_with("top");
                let o = popups::show(ui, &shared.notifs.popups, out_h, bottom_up, &mut actions);
                hovered = o.hovered;
                input_rects = Some(o.input_rects);
            }
            Kind::Capture => {
                if let (Some(st), Some(i)) = (shared.capture.as_mut(), cap_idx) {
                    crate::capture::ui::show(ui, st, i, &mut actions);
                }
            }
            Kind::Settings => {
                let goto = shared.settings_goto.take();
                let env = ui::settings::Env {
                    goto: goto.as_deref(),
                    screen_timeout: shared.screen_timeout,
                    lock_timeout: self.idle.as_ref().map(|i| i.lock_minutes).unwrap_or(0),
                    binds: &shared.binds,
                    binds_error: shared.binds_error.as_deref(),
                    captured: shared.captured_key.as_deref(),
                    defaults: &shared.default_apps,
                };
                ui::settings::window(ui, &shared.cfg, &env, &mut actions);
            }
            Kind::Overlay => match shared.overlay.clone() {
                Some(Overlay::Launcher) => {
                    if let Some(data) = &shared.launch_data {
                        let infos: Vec<launch::WinInfo> = shared
                            .toplevels
                            .iter()
                            .enumerate()
                            .filter(|(_, t)| t.ready)
                            .map(|(index, t)| launch::WinInfo { index, title: t.title.clone(), app_id: t.app_id.clone() })
                            .collect();
                        launcher::show(ui, &mut shared.launcher, data, &infos, shared.cfg.launcher.width as f32, shared.cfg.launcher.max_rows.clamp(3, 20) as usize, &mut actions);
                    }
                }
                Some(ov) => {
                    let place = panels::Placement {
                        anchor_x: shared.overlay_anchor,
                        bar_height: shared.cfg.bar.height as f32,
                        bar_on_top: bar_top,
                    };
                    let win_list = if matches!(ov, Overlay::Windows) { window_list(shared) } else { ui::WinList::default() };
                    panels::show(ui, &ov, &place, &shared.notifs, &shared.tray, &shared.tray_menu, shared.tray_menu_loaded, &shared.wifi, shared.wifi_scanning, shared.screen_timeout, shared.stats.brightness_percent, &shared.audio, shared.battery.as_ref(), &shared.profiles, &shared.media, &win_list, &shared.procs, &mut actions);
                }
                None => {}
            },
        });
        let wanted_cursor = out.platform_output.cursor_icon;
        let over_this = self.pointer_on.as_ref().is_some_and(|(s, _)| s == w.surf.wl_surface());
        if kind == Kind::Popups && hovered != shared.popup_hover {
            shared.popup_hover = hovered;
        }

        w.pending_tex.append(out.textures_delta);
        let jobs = w.ctx.tessellate(out.shapes, out.pixels_per_point);
        let surface = w.surf.wl_surface().clone();
        if let Some((_, vp)) = &w.frac {
            // The buffer is in device pixels. The viewport maps it onto the logical size.
            vp.set_destination(lw as i32, lh as i32);
        } else {
            surface.set_buffer_scale(w.scale as i32);
        }
        if let Some(rects) = input_rects {
            // Only the cards take clicks. The rest of the surface lets them through.
            if let Ok(region) = Region::new(&*compositor) {
                for r in rects {
                    region.add(r.min.x as i32, r.min.y as i32, r.width().ceil() as i32, r.height().ceil() as i32);
                }
                surface.set_input_region(Some(region.wl_region()));
            }
        }
        surface.frame(qh, FrameCallbackData(surface.clone()));
        let painter = w.painter.as_mut().unwrap();
        if gfx.paint(painter, &w.pending_tex, &jobs, out.pixels_per_point) {
            w.pending_tex.clear();
            w.frame_pending = true;
            w.dirty = false;
        } else {
            w.surf.commit();
            w.frame_pending = true;
        }
        let delay = out
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.repaint_delay)
            .unwrap_or(Duration::MAX);
        w.schedule(delay);
        let _ = output_state;

        if over_this {
            self.apply_cursor(wanted_cursor);
        }
        for a in actions {
            self.handle_action(a, qh);
        }
    }

    /// Tell the compositor which cursor to show over our surfaces.
    fn apply_cursor(&mut self, icon: egui::CursorIcon) {
        use egui::CursorIcon as C;
        let shape = match icon {
            C::PointingHand => Shape::Pointer,
            C::Text | C::VerticalText => Shape::Text,
            C::ResizeHorizontal | C::ResizeColumn | C::ResizeEast | C::ResizeWest => Shape::EwResize,
            C::ResizeVertical | C::ResizeRow | C::ResizeNorth | C::ResizeSouth => Shape::NsResize,
            C::NotAllowed | C::NoDrop => Shape::NotAllowed,
            C::Grab => Shape::Grab,
            C::Grabbing => Shape::Grabbing,
            C::Wait => Shape::Wait,
            C::Progress => Shape::Progress,
            C::Crosshair => Shape::Crosshair,
            C::ResizeNwSe | C::ResizeNorthWest | C::ResizeSouthEast => Shape::NwseResize,
            C::ResizeNeSw | C::ResizeNorthEast | C::ResizeSouthWest => Shape::NeswResize,
            C::Help => Shape::Help,
            C::Move | C::AllScroll => Shape::Move,
            _ => Shape::Default,
        };
        if self.cursor_shape == Some(shape) {
            return;
        }
        if let (Some(dev), Some((_, serial))) = (&self.cursor_dev, &self.pointer_on) {
            dev.set_shape(*serial, shape);
            self.cursor_shape = Some(shape);
        }
    }

    fn handle_action(&mut self, a: Action, qh: &QueueHandle<App>) {
        match a {
            Action::ToggleOverlay(ov, anchor) => {
                if anchor.is_nan() {
                    // Change the open panel in place.
                    self.shared.overlay = Some(ov);
                    self.mark_kind(Kind::Overlay);
                } else if self.shared.overlay.as_ref().is_some_and(|c| c.same_kind(&ov)) {
                    self.close_overlay();
                } else {
                    self.open_overlay(qh, ov, anchor);
                }
            }
            Action::CloseOverlay => self.close_overlay(),
            Action::NotifClick(id) if self.shared.cap_notes.contains_key(&id) => {
                self.cap_note_action(id, "osmia:open");
                self.dismiss(id);
            }
            Action::NotifClick(id) => {
                let n = self
                    .shared
                    .notifs
                    .popups
                    .iter()
                    .chain(self.shared.notifs.history.iter())
                    .find(|n| n.id == id)
                    .cloned();
                if let Some(n) = n {
                    if n.default_action().is_some() {
                        if let Some(c) = &self.shared.notif_conn {
                            notif::emit_action(c, id, "default");
                        }
                    }
                    if let Some(i) = self.find_toplevel(&n) {
                        self.focus_toplevel(i);
                    }
                }
                self.dismiss(id);
            }
            Action::NotifDismiss(id) => self.dismiss(id),
            Action::NotifButton(id, key) if key.starts_with("osmia:") => {
                self.cap_note_action(id, &key);
                self.dismiss(id);
            }
            Action::NotifButton(id, key) => {
                if let Some(c) = &self.shared.notif_conn {
                    notif::emit_action(c, id, &key);
                }
                self.dismiss(id);
            }
            Action::ClearPopups => self.clear_popups(),
            Action::ClearHistory => {
                self.shared.notifs.history.clear();
                for t in &mut self.shared.toplevels {
                    t.alert_ids.clear();
                    t.refresh_attention();
                }
                self.mark_all();
            }
            Action::HistoryRemove(id) => {
                self.shared.notifs.history.retain(|h| h.id != id);
                self.clear_alert(id);
                self.mark_all();
            }
            Action::ToggleDnd => {
                self.shared.notifs.dnd = !self.shared.notifs.dnd;
                self.mark_all();
            }
            Action::Power(p) => {
                self.close_overlay();
                if std::env::var_os("OSMIA_DRY_RUN").is_some() {
                    log::info!("dry run: would do {p:?}");
                    return;
                }
                match p {
                    PowerAction::Lock => launch::spawn(&self.shared.cfg.session.lock.clone()),
                    PowerAction::Logout => launch::spawn(&self.shared.cfg.session.logout.clone()),
                    PowerAction::Suspend => sys::logind_call("Suspend"),
                    PowerAction::Reboot => sys::logind_call("Reboot"),
                    PowerAction::PowerOff => sys::logind_call("PowerOff"),
                }
            }
            Action::VolumeScroll(d) => {
                // Show the new value now. The real one follows from the system.
                if let Some(v) = &mut self.shared.stats.volume {
                    v.percent = (v.percent as i32 + d).clamp(0, 100) as u32;
                    let v = v.clone();
                    self.shared.sampler.set_volume(Some(v));
                    self.mark_kind(Kind::Bar);
                }
                sys::set_volume_step(d)
            }
            Action::AudioVolume(input, name, pct) => {
                self.shared.audio_set_at = Instant::now();
                if let Some(d) = self.audio_dev_mut(input, &name) {
                    d.percent = pct;
                    if d.muted && pct > 0 {
                        d.muted = false;
                    }
                }
                if !input {
                    if let Some(v) = &mut self.shared.stats.volume {
                        v.percent = pct;
                        let v = v.clone();
                        self.shared.sampler.set_volume(Some(v));
                        self.mark_kind(Kind::Bar);
                    }
                }
                let kind = if input { "source" } else { "sink" };
                sys::audio_run(vec![vec![format!("set-{kind}-volume"), name, format!("{pct}%")]], None);
                self.mark_kind(Kind::Overlay);
            }
            Action::AudioMute(input, name) => {
                if let Some(d) = self.audio_dev_mut(input, &name) {
                    d.muted = !d.muted;
                }
                let kind = if input { "source" } else { "sink" };
                sys::audio_run(vec![vec![format!("set-{kind}-mute"), name, "toggle".into()]], self.audio_tx.clone());
                self.mark_kind(Kind::Overlay);
            }
            Action::AudioSelect(input, name, port) => {
                let kind = if input { "source" } else { "sink" };
                let mut cmds = vec![vec![format!("set-default-{kind}"), name.clone()]];
                if let Some(p) = &port {
                    cmds.push(vec![format!("set-{kind}-port"), name.clone(), p.clone()]);
                }
                let list = if input { &mut self.shared.audio.sources } else { &mut self.shared.audio.sinks };
                for d in list.iter_mut() {
                    d.default = d.name == name;
                    if d.name == name && port.is_some() {
                        d.active_port = port.clone();
                    }
                }
                sys::audio_run(cmds, self.audio_tx.clone());
                self.mark_kind(Kind::Overlay);
                self.mark_kind(Kind::Bar);
                if let Some(p) = &self.slow_poke {
                    let _ = p.send(());
                }
            }
            Action::SetConfig(key, value) => match config::set(&key, value) {
                Ok(()) => {
                    self.reload_config(qh);
                    self.mark_all();
                }
                Err(e) => {
                    self.notify_self("Could not save the setting", &e);
                    self.mark_kind(Kind::Popups);
                }
            },
            Action::MediaPlayPause(b) => self.media_cmd(crate::mpris::Cmd::PlayPause(b)),
            Action::MediaNext(b) => self.media_cmd(crate::mpris::Cmd::Next(b)),
            Action::MediaPrev(b) => self.media_cmd(crate::mpris::Cmd::Previous(b)),
            Action::MediaRaise(b) => {
                self.open_player(&b);
                self.close_overlay();
            }
            Action::MediaSeek(b, pos) => {
                if let Some(p) = self.shared.media.players.iter_mut().find(|p| p.bus == b) {
                    let cmd = if p.track_id.is_empty() {
                        crate::mpris::Cmd::Seek(b.clone(), pos - p.position_now())
                    } else {
                        crate::mpris::Cmd::SetPosition(b.clone(), p.track_id.clone(), pos)
                    };
                    // Show the new place at once; the player confirms in a moment.
                    p.position_us = pos;
                    p.pos_at = Instant::now();
                    self.media_cmd(cmd);
                    self.mark_kind(Kind::Overlay);
                }
            }
            Action::CaptureOpen => {
                self.control_capture(qh, &[]);
            }
            Action::CaptureCancel => self.end_capture(),
            Action::CaptureGo(t) => self.capture_go(t),
            Action::StopRecording => self.stop_recording(),
            Action::OpenSettings => {
                self.close_overlay();
                self.open_settings(qh);
            }
            Action::SetDefaultApp(cat, id) => {
                crate::defaults::set(cat, &id);
                // Show the choice at once. The next time settings opens it is read again.
                if let Some(row) = self.shared.default_apps.rows.get_mut(cat) {
                    row.current = row.apps.iter().find(|a| a.id == id).cloned();
                }
                self.mark_kind(Kind::Settings);
            }
            Action::OpenConfigFile => {
                config::ensure_file();
                launch::spawn(&format!("xdg-open '{}'", config::path().display()));
            }
            Action::SetPowerProfile(name) => {
                self.shared.profiles.active = name.clone();
                if let Some(tx) = self.prof_tx.clone() {
                    sys::profiles_run(Some(name), tx);
                }
                self.mark_kind(Kind::Overlay);
            }
            Action::SetBrightness(p) => {
                self.shared.sampler.set_brightness(p);
                self.shared.stats.brightness_percent = Some(p);
                sys::set_brightness_percent(p);
                self.mark_kind(Kind::Bar);
                self.mark_kind(Kind::Overlay);
            }
            Action::SetLockTimeout(m) => {
                if let Some(i) = &mut self.idle {
                    i.set_lock(m);
                }
                self.mark_kind(Kind::Settings);
            }
            Action::ReloadConfig => {
                self.reload_config(qh);
                self.mark_all();
            }
            Action::ProcLock(l) => {
                self.shared.procs.lock = l;
                self.mark_kind(Kind::Overlay);
            }
            Action::SetProcLimit(name, lim) => {
                let pids: Vec<u32> = self.shared.procs.all.iter().find(|p| p.name == name).map(|p| p.members.iter().map(|m| m.0).collect()).unwrap_or_default();
                if let Err(e) = self.shared.limits.set(&name, lim, &pids) {
                    self.notify_self("Could not set the limit", &e);
                }
                self.shared.procs.limits = self.shared.limits.set.clone();
                self.mark_kind(Kind::Overlay);
            }
            Action::KillProcs(pids) => {
                for p in pids {
                    unsafe { libc::kill(p as i32, libc::SIGTERM) };
                }
                // Show the result on the next tick.
                self.next_tick = Instant::now() + Duration::from_millis(600);
            }
            Action::ShortcutRecord(on) => {
                self.shared.recording_key = on;
                self.shared.captured_key = None;
                self.set_inhibit(on, qh);
                self.mark_kind(Kind::Settings);
            }
            Action::ShortcutTaken => {
                self.set_inhibit(false, qh);
                self.shared.recording_key = false;
                self.shared.captured_key = None;
            }
            Action::ShortcutSetKey(i, k) => {
                let k = k.trim().to_owned();
                self.edit_binds(|t| crate::keybinds::set_key(t, i, &k));
            }
            Action::ShortcutSetCommand(i, c) => {
                self.edit_binds(|t| crate::keybinds::set_command(t, i, &c));
            }
            Action::ShortcutDelete(i) => {
                self.edit_binds(|t| crate::keybinds::delete(t, i));
            }
            Action::ShortcutAdd(k, c) => {
                let k = k.trim().to_owned();
                self.edit_binds(|t| crate::keybinds::add(t, &k, &c));
            }
            Action::ShortcutsRestore => {
                let shipped = crate::keybinds::shipped();
                self.edit_binds(|_| std::fs::read_to_string(&shipped).map_err(|e| format!("{}: {e}", shipped.display())));
                if self.shared.binds_error.is_none() {
                    self.notify_self("Shortcuts restored", "Your old file is saved as rc.xml.bak-osmia next to it.");
                }
            }
            Action::ResetConfig => {
                let p = config::path();
                let _ = std::fs::copy(&p, p.with_extension("toml.bak"));
                let _ = std::fs::remove_file(&p);
                self.reload_config(qh);
                self.notify_self("Settings reset", "Your old file is saved as config.toml.bak next to it.");
                self.mark_all();
            }
            Action::SetScreenTimeout(m) => {
                if let Some(i) = &mut self.idle {
                    i.set(m);
                }
                self.shared.screen_timeout = m;
                self.mark_kind(Kind::Overlay);
                self.mark_kind(Kind::Settings);
            }
            Action::BrightnessScroll(d) => {
                sys::set_brightness_step(d);
                // The new value shows on the next 2-second sample.
            }
            Action::ToggleMute => {
                if let Some(v) = &mut self.shared.stats.volume {
                    v.muted = !v.muted;
                    let v = v.clone();
                    self.shared.sampler.set_volume(Some(v));
                    self.mark_kind(Kind::Bar);
                }
                sys::toggle_mute()
            }
            Action::FocusWindow(i) => {
                self.focus_toplevel(i);
                self.close_overlay();
            }
            Action::GotoWorkspace(i) => {
                if let (Some(w), Some(m)) = (self.shared.workspaces.get(i), &self.workspace_mgr) {
                    w.handle.activate();
                    m.commit();
                }
                self.close_overlay();
            }
            Action::NetJoin(ssid, password) => {
                sys::join_wifi(&ssid, &password);
                self.close_overlay();
            }
            Action::NetRescan => {
                self.shared.wifi_scanning = true;
                if let Some(tx) = &self.wifi_tx {
                    sys::scan_wifi(tx.clone());
                }
                self.mark_kind(Kind::Overlay);
            }
            Action::NetConnect(ssid, saved) => {
                sys::connect_wifi(&ssid, saved);
                self.close_overlay();
            }
            Action::TrayActivate(id) => {
                self.close_overlay();
                // Many apps ignore Activate, so find the window ourselves first.
                if let Some(i) = self.tray_toplevel(&id) {
                    self.focus_toplevel(i);
                } else if let Some(tx) = &self.tray_cmd {
                    let _ = tx.send(crate::tray::Cmd::Activate(id, 0, 0));
                }
            }
            Action::TrayNativeMenu(id) => {
                if let Some(tx) = &self.tray_cmd {
                    let _ = tx.send(crate::tray::Cmd::NativeMenu(id));
                }
                self.close_overlay();
            }
            Action::TrayMenuOpen(id, anchor) => {
                let has_menu = self.shared.tray.iter().find(|i| i.id == id).is_some_and(|i| i.menu.is_some());
                if has_menu {
                    if let Some(tx) = &self.tray_cmd {
                        let _ = tx.send(crate::tray::Cmd::OpenMenu(id.clone()));
                    }
                }
                self.shared.tray_menu_loaded = !has_menu;
                self.shared.tray_menu = Vec::new();
                self.open_overlay(qh, Overlay::TrayMenu(id), anchor);
            }
            Action::TrayMenuClick(id, menu_id) => {
                if let Some(tx) = &self.tray_cmd {
                    let _ = tx.send(crate::tray::Cmd::ClickMenu(id, menu_id));
                }
                self.close_overlay();
            }
            Action::RunApp(id) => {
                let term = self.shared.cfg.launcher.terminal.clone();
                if let Some(data) = &mut self.shared.launch_data {
                    if let Some(app) = data.apps.iter().find(|a| a.id == id).cloned() {
                        launch::run_app(&app, &term);
                        data.record(&id);
                    }
                }
                self.close_overlay();
            }
            Action::RunCommand(c) => {
                launch::run_command(&c);
                self.close_overlay();
            }
            Action::CopyAndClose(text) => {
                copy_to_clipboard(&text);
                self.close_overlay();
            }
        }
    }

    // ---------- screenshots and recording ----------

    fn control_capture(&mut self, qh: &QueueHandle<App>, args: &[&str]) -> String {
        use crate::capture::ui::{Kind as K, Mode as M};
        // Pick a target in the open overlay from a script: pick screen | pick area X Y W H | pick window N.
        if let ["pick", what @ ..] = args {
            if self.shared.capture.is_none() {
                return "error: the capture overlay is not open".into();
            }
            let n = |i: usize| what.get(i).and_then(|v| v.parse::<f32>().ok());
            let t = match what {
                ["screen"] => crate::ui::Target::Screen(0),
                ["window", _] => crate::ui::Target::Window(n(1).unwrap_or(0.0) as usize),
                ["area", ..] => match (n(1), n(2), n(3), n(4)) {
                    (Some(x), Some(y), Some(w), Some(h)) => crate::ui::Target::Region(0, egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))),
                    _ => return "error: pick area X Y W H".into(),
                },
                _ => return "error: pick screen | area X Y W H | window N".into(),
            };
            self.capture_go(t);
            return "ok".into();
        }
        if self.shared.recording.is_some() {
            // Any capture key while recording stops it.
            if args.first() != Some(&"status") {
                self.stop_recording();
                return "stopped".into();
            }
            return "recording".into();
        }
        let (mode, kind) = match args {
            [] | ["area"] | ["region"] | ["photo"] => (M::Region, K::Photo),
            ["video"] | ["region-video"] => (M::Region, K::Video),
            ["window"] => (M::Window, K::Photo),
            ["window-video"] => (M::Window, K::Video),
            ["screen"] => (M::Screen, K::Photo),
            ["screen-video"] => (M::Screen, K::Video),
            ["now", "screen"] => return self.capture_now(false),
            ["now", "window"] => return self.capture_now(true),
            ["status"] => return "idle".into(),
            ["stop"] => return "not recording".into(),
            _ => return "error: capture [area|window|screen|video|region-video|window-video|screen-video|now screen|now window|stop]".into(),
        };
        self.start_capture(qh, mode, kind)
    }

    fn start_capture(&mut self, _qh: &QueueHandle<App>, mode: crate::capture::ui::Mode, kind: crate::capture::ui::Kind) -> String {
        if self.shared.capture.is_some() {
            return "already open".into();
        }
        let Some(tx) = self.cap_tx.clone() else { return "error: no capture channel".into() };
        crate::capture::gather(self.shared.cfg.capture.cursor, mode, kind, tx);
        "ok".into()
    }

    /// Take a picture of the whole screen or the active window right away, with no overlay.
    fn capture_now(&mut self, window: bool) -> String {
        let Some(tx) = self.cap_tx.clone() else { return "error: no capture channel".into() };
        let cfg = self.shared.cfg.capture.clone();
        let active = self.shared.toplevels.iter().find(|t| t.activated).map(|t| (t.app_id.clone(), t.title.clone()));
        let out_name = self.focused_output().and_then(|o| self.output_state.info(&o)).and_then(|i| i.name);
        std::thread::spawn(move || {
            let r = (|| -> Result<crate::capture::grab::Image, String> {
                let mut g = crate::capture::grab::Grabber::new()?;
                if window {
                    let (app, title) = active.ok_or("no window is focused")?;
                    let id = g
                        .windows()
                        .into_iter()
                        .find(|w| w.app_id == app && w.title == title)
                        .or_else(|| None)
                        .ok_or("could not find the focused window")?
                        .id;
                    g.grab(&crate::capture::grab::Source::Window(id), false)
                } else {
                    let name = out_name.or_else(|| g.output_names().into_iter().next()).unwrap_or_default();
                    g.grab(&crate::capture::grab::Source::Output(name), cfg.cursor)
                }
            })();
            match r {
                Ok(img) => crate::capture::save_photo(img, cfg, tx),
                Err(e) => {
                    let _ = tx.send(crate::capture::CapMsg::PhotoDone(Err(e)));
                }
            }
        });
        "ok".into()
    }

    fn end_capture(&mut self) {
        self.shared.capture = None;
        self.wins.retain(|w| w.kind != Kind::Capture);
    }

    fn output_logical(&self, name: &str) -> Option<(f32, f32)> {
        self.output_state
            .outputs()
            .filter_map(|o| self.output_state.info(&o))
            .find(|i| i.name.as_deref() == Some(name))
            .and_then(|i| i.logical_size)
            .map(|(w, h)| (w as f32, h as f32))
    }

    fn capture_go(&mut self, target: crate::ui::Target) {
        use crate::capture::{self, grab::Source, ui::Kind as K, video};
        use crate::ui::Target;
        let Some(st) = self.shared.capture.take() else { return };
        self.wins.retain(|w| w.kind != Kind::Capture);
        let cfg = self.shared.cfg.capture.clone();
        let Some(tx) = self.cap_tx.clone() else { return };
        // The source, a crop in buffer pixels, and the frozen picture if we have one.
        let (src, crop_px, frozen): (Source, Option<(u32, u32, u32, u32)>, Option<std::sync::Arc<capture::grab::Image>>) = match target {
            Target::Screen(o) => {
                let Some(s) = st.shots.get(o) else { return };
                (Source::Output(s.name.clone()), None, Some(s.image.clone()))
            }
            Target::Window(i) => {
                let Some(w) = st.windows.get(i) else { return };
                (Source::Window(w.id.clone()), None, Some(w.image.clone()))
            }
            Target::Region(o, r) => {
                let Some(s) = st.shots.get(o) else { return };
                let (lw, _) = self.output_logical(&s.name).unwrap_or((s.image.w as f32, s.image.h as f32));
                let k = s.image.w as f32 / lw;
                let (x, y) = ((r.min.x * k).round() as u32, (r.min.y * k).round() as u32);
                let (w, h) = (((r.width() * k).round() as u32).max(2), ((r.height() * k).round() as u32).max(2));
                (Source::Output(s.name.clone()), Some((x, y, w, h)), Some(s.image.clone()))
            }
        };
        match st.kind {
            K::Photo => {
                if st.delay == 0 {
                    if let Some(img) = frozen {
                        let img = match crop_px {
                            Some((x, y, w, h)) => capture::crop(&img, x, y, w, h),
                            None => (*img).clone(),
                        };
                        capture::save_photo(img, cfg, tx);
                    }
                } else {
                    // Wait, then take a fresh picture.
                    self.notify_self("Capturing soon", &format!("A screenshot in {} seconds.", st.delay));
                    self.mark_kind(Kind::Popups);
                    capture::photo_later(src, crop_px, st.cursor, st.delay, cfg, tx);
                }
            }
            K::Video => {
                let out = match capture::out::videos_path(&cfg) {
                    Ok(p) => p,
                    Err(e) => {
                        self.notify_self("Cannot record", &e);
                        self.mark_kind(Kind::Popups);
                        return;
                    }
                };
                let opts = video::Opts {
                    source: src,
                    crop: crop_px,
                    cursor: st.cursor,
                    fps: cfg.fps.clamp(10, 60),
                    audio: st.audio,
                    encoder: cfg.encoder.clone(),
                    quality: cfg.quality.clone(),
                    out,
                    delay: st.delay,
                };
                let done = tx.clone();
                let rec = video::start(opts, move |r| {
                    let _ = done.send(capture::CapMsg::VideoDone(r));
                });
                self.shared.recording = Some(RecState { rec, stopping: false });
                self.mark_kind(Kind::Bar);
            }
        }
    }

    fn stop_recording(&mut self) {
        if let Some(r) = &mut self.shared.recording {
            if !r.stopping {
                r.rec.stop();
                r.stopping = true;
            }
        }
        self.mark_kind(Kind::Bar);
    }

    /// A notification from the capture tool, with buttons that act on the file.
    fn notify_capture(&mut self, summary: &str, body: &str, icon: Option<&std::path::Path>, file: &std::path::Path, buttons: &[(&str, &str)]) {
        let id = next_self_id();
        let actions: Vec<(String, String)> = buttons.iter().map(|(k, l)| (format!("osmia:{k}"), (*l).to_owned())).collect();
        let mut actions = actions;
        actions.push(("default".into(), String::new()));
        let n = Notification {
            id,
            app_name: "osmia".into(),
            app_icon: icon.map(|p| p.display().to_string()).unwrap_or_default(),
            summary: summary.into(),
            body: body.into(),
            actions,
            urgency: 1,
            desktop_entry: None,
            transient: false,
            timeout: Some(Duration::from_secs(10)),
            born: Instant::now(),
            unread: false,
        };
        self.shared.cap_notes.insert(id, file.to_owned());
        self.shared.notifs.add(n, 0);
        self.mark_kind(Kind::Popups);
        self.mark_kind(Kind::Bar);
    }

    fn cap_note_action(&mut self, id: u32, key: &str) {
        use crate::capture::out;
        let Some(file) = self.shared.cap_notes.get(&id).cloned() else { return };
        match key {
            "osmia:open" => out::open(&file),
            "osmia:folder" => {
                if let Some(dir) = file.parent() {
                    out::open(dir);
                }
            }
            "osmia:gif" | "osmia:webm" => {
                let kind = if key.ends_with("gif") { out::Export::Gif } else { out::Export::Webm };
                if let Some(tx) = self.cap_tx.clone() {
                    self.notify_self("Converting", &format!("Making a {} copy.", kind.ext().to_uppercase()));
                    self.mark_kind(Kind::Popups);
                    std::thread::spawn(move || {
                        let _ = tx.send(crate::capture::CapMsg::ExportDone(out::export(&file, kind)));
                    });
                }
            }
            _ => {}
        }
    }

    fn on_cap_msg(&mut self, m: crate::capture::CapMsg) {
        use crate::capture::{out, CapMsg};
        match m {
            CapMsg::Ready { shots, mode, kind } => {
                if self.shared.capture.is_some() || self.shared.recording.is_some() {
                    return;
                }
                let c = &self.shared.cfg.capture;
                let st = crate::capture::ui::CaptureUi::new(shots, vec![], mode, kind, crate::capture::video::Audio::parse(&c.audio), c.cursor, c.delay.min(10));
                self.shared.capture = Some(st);
                if let Some(qh) = self.qh_store.clone() {
                    let outputs: Vec<_> = self.output_state.outputs().collect();
                    for o in outputs {
                        let surface = self.compositor.create_surface(&qh);
                        let layer = self.layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("osmia-capture"), Some(&o));
                        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
                        layer.set_size(0, 0);
                        layer.set_exclusive_zone(-1);
                        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
                        layer.commit();
                        let w = self.new_win(&qh, Kind::Capture, Surf::Layer(layer), Some(o));
                        self.wins.push(w);
                    }
                }
            }
            CapMsg::Windows(list) => {
                if let Some(st) = &mut self.shared.capture {
                    st.windows = list;
                    self.mark_kind(Kind::Capture);
                }
            }
            CapMsg::PhotoDone(Ok((path, thumb))) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let body = if self.shared.cfg.capture.copy { format!("{name}\nCopied to the clipboard.") } else { name };
                self.notify_capture("Screenshot saved", &body, thumb.as_deref(), &path, &[("open", "Open"), ("folder", "Folder")]);
            }
            CapMsg::PhotoDone(Err(e)) | CapMsg::Failed(e) => {
                self.notify_self("Screenshot failed", &e);
                self.mark_kind(Kind::Popups);
            }
            CapMsg::VideoDone(r) => {
                self.shared.recording = None;
                self.mark_kind(Kind::Bar);
                match r {
                    Ok(path) => {
                        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        self.notify_capture("Recording saved", &name, None, &path, &[("open", "Play"), ("folder", "Folder"), ("gif", "GIF"), ("webm", "WebM")]);
                        if let (Some(kind), Some(tx)) = (out::Export::parse(&self.shared.cfg.capture.export), self.cap_tx.clone()) {
                            std::thread::spawn(move || {
                                let _ = tx.send(CapMsg::ExportDone(out::export(&path, kind)));
                            });
                        }
                    }
                    Err(e) if e == "cancelled" => {}
                    Err(e) => {
                        self.notify_self("Recording failed", &e);
                        self.mark_kind(Kind::Popups);
                    }
                }
            }
            CapMsg::ExportDone(Ok(path)) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.notify_capture("Converted", &name, None, &path, &[("open", "Open"), ("folder", "Folder")]);
            }
            CapMsg::ExportDone(Err(e)) => {
                self.notify_self("Conversion failed", &e);
                self.mark_kind(Kind::Popups);
            }
        }
    }

    /// Keep the notification daemon name: take it back if it was lost, or try again if
    /// another daemon was in the way at start and has gone.
    fn check_notification_name(&mut self) {
        if std::env::var_os("OSMIA_NESTED").is_some() {
            return;
        }
        if let Some(c) = &self.shared.notif_conn {
            if !notif::reclaim(c) {
                log::warn!("another program owns the notification name");
            }
        } else if self.shared.notifs.blocked {
            if let Some(tx) = self.n_tx.clone() {
                if let Ok(c) = notif::start(tx, self.shared.cfg.notifications.default_timeout_ms) {
                    self.shared.notif_conn = Some(c);
                    self.shared.notifs.blocked = false;
                    self.mark_kind(Kind::Bar);
                }
            }
        }
    }

    fn media_cmd(&self, c: crate::mpris::Cmd) {
        if let Some(tx) = &self.mpris_cmd {
            let _ = tx.send(c);
        }
    }

    /// `media play-pause|pause|next|previous|status`, for keys and scripts.
    fn control_media(&mut self, args: &[&str]) -> String {
        use crate::mpris::{Cmd, Status};
        let Some(p) = self.shared.media.current().cloned() else { return "no player".into() };
        match args {
            ["play-pause"] | ["toggle"] => self.media_cmd(Cmd::PlayPause(p.bus)),
            ["pause"] => self.media_cmd(Cmd::Pause(p.bus)),
            ["next"] => self.media_cmd(Cmd::Next(p.bus)),
            ["previous"] | ["prev"] => self.media_cmd(Cmd::Previous(p.bus)),
            ["panel"] => {
                let qh = self.qh();
                let anchor = self.anchor_for("media");
                self.open_overlay(&qh, Overlay::Media, anchor);
                return "ok".into();
            }
            [] | ["status"] => {
                let s = match p.status {
                    Status::Playing => "playing",
                    Status::Paused => "paused",
                    Status::Stopped => "stopped",
                };
                return format!("{s}: {} ({})", p.line(), p.identity);
            }
            _ => return "error: media [play-pause|pause|next|previous|status]".into(),
        }
        "ok".into()
    }

    fn refresh_audio(&self) {
        sys::audio_run(vec![], self.audio_tx.clone());
    }

    fn audio_dev_mut(&mut self, input: bool, name: &str) -> Option<&mut sys::AudioDev> {
        let list = if input { &mut self.shared.audio.sources } else { &mut self.shared.audio.sinks };
        list.iter_mut().find(|d| d.name == name)
    }

    fn dismiss(&mut self, id: u32) {
        if self.shared.notifs.remove_popup(id, false).is_some() {
            self.emit_closed(id, notif::REASON_DISMISSED);
        }
        if let Some(h) = self.shared.notifs.history.iter_mut().find(|h| h.id == id) {
            h.unread = false;
        }
        self.clear_alert(id);
        self.mark_kind(Kind::Popups);
        self.mark_kind(Kind::Bar);
        if matches!(self.shared.overlay, Some(Overlay::History)) {
            self.mark_kind(Kind::Overlay);
        }
    }
}

fn attention_entries(tl: &[Toplevel]) -> Vec<bar::Entry> {
    let mut v: Vec<(Instant, bar::Entry)> = tl
        .iter()
        .enumerate()
        .filter(|(_, t)| t.ready && (t.minimized || t.in_alert()))
        .map(|(index, t)| {
            (
                t.attn_since.unwrap_or_else(Instant::now),
                bar::Entry { index, title: t.title.clone(), app_id: t.app_id.clone(), alert: t.in_alert() },
            )
        })
        .collect();
    v.sort_by_key(|(t, _)| *t);
    v.into_iter().map(|(_, e)| e).collect()
}

/// Copy text with wl-copy, which keeps serving the selection after we exit.
fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    match Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        Ok(mut c) => {
            if let Some(mut s) = c.stdin.take() {
                let _ = s.write_all(text.as_bytes());
            }
        }
        Err(e) => log::warn!("wl-copy is not available: {e}"),
    }
}

// ---------- Wayland handlers ----------

impl wayland_client::Dispatch<WpFractionalScaleManagerV1, ()> for App {
    fn event(_: &mut Self, _: &WpFractionalScaleManagerV1, _: <WpFractionalScaleManagerV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl wayland_client::Dispatch<WpCursorShapeManagerV1, ()> for App {
    fn event(_: &mut Self, _: &WpCursorShapeManagerV1, _: <WpCursorShapeManagerV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl wayland_client::Dispatch<WpCursorShapeDeviceV1, ()> for App {
    fn event(_: &mut Self, _: &WpCursorShapeDeviceV1, _: <WpCursorShapeDeviceV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl wayland_client::Dispatch<WpViewporter, ()> for App {
    fn event(_: &mut Self, _: &WpViewporter, _: <WpViewporter as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl wayland_client::Dispatch<WpViewport, ()> for App {
    fn event(_: &mut Self, _: &WpViewport, _: <WpViewport as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl wayland_client::Dispatch<WpFractionalScaleV1, wl_surface::WlSurface> for App {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        surface: &wl_surface::WlSurface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            let Some(i) = state.win_index(surface) else { return };
            let w = &mut state.wins[i];
            // The value is in 120ths.
            w.scale = (scale as f32 / 120.0).max(1.0);
            let px = w.px();
            if let Some(p) = w.painter.as_mut() {
                state.gfx.resize(p, px.0, px.1);
            }
            w.dirty = true;
        }
    }
}


impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, new_factor: i32) {
        if let Some(i) = self.win_index(surface) {
            let gfx = &self.gfx;
            let w = &mut self.wins[i];
            if w.frac.is_none() {
                w.scale = new_factor.max(1) as f32;
                let px = w.px();
                if let Some(p) = w.painter.as_mut() {
                    gfx.resize(p, px.0, px.1);
                }
                w.dirty = true;
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, _: u32) {
        if let Some(i) = self.win_index(surface) {
            self.wins[i].frame_pending = false;
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.qh_store = Some(qh.clone());
        self.add_bar(qh, output);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.wins.retain(|w| w.output.as_ref() != Some(&output));
        if self.shared.last_output.as_ref() == Some(&output) {
            self.shared.last_output = None;
        }
    }
}

impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        self.wins.retain(|w| !w.surf.is_window(window));
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window, configure: WindowConfigure, _: u32) {
        let gfx = &self.gfx;
        if let Some(w) = self.wins.iter_mut().find(|w| w.surf.is_window(window)) {
            let (cw, ch) = configure.new_size;
            let size = (
                cw.map(|v| v.get()).unwrap_or(if w.size.0 == 0 { 880 } else { w.size.0 }),
                ch.map(|v| v.get()).unwrap_or(if w.size.1 == 0 { 640 } else { w.size.1 }),
            );
            if size != w.size {
                w.size = size;
                let px = w.px();
                if let Some(p) = w.painter.as_mut() {
                    gfx.resize(p, px.0, px.1);
                }
            }
            w.configured = true;
            w.dirty = true;
        }
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.wins.retain(|w| !w.surf.is_layer(layer));
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, configure: LayerSurfaceConfigure, _: u32) {
        let gfx = &self.gfx;
        if let Some(w) = self.wins.iter_mut().find(|w| w.surf.is_layer(layer)) {
            let (cw, ch) = configure.new_size;
            let size = (if cw == 0 { w.size.0.max(1) } else { cw }, if ch == 0 { w.size.1.max(1) } else { ch });
            if size != w.size {
                w.size = size;
                let px = w.px();
                if let Some(p) = w.painter.as_mut() {
                    gfx.resize(p, px.0, px.1);
                }
            }
            w.configured = true;
            w.dirty = true;
        }
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        if self.seat.is_none() {
            self.seat = Some(seat);
        }
    }
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if self.seat.is_none() {
            self.seat = Some(seat.clone());
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
            if let (Some(m), Some(p)) = (&self.cursor_mgr, &self.pointer) {
                self.cursor_dev = Some(m.get_pointer(p, qh, ()));
            }
        }
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self
                .seat_state
                .get_keyboard_with_repeat(
                    qh,
                    &seat,
                    None,
                    self.loop_handle.clone(),
                    Box::new(|app: &mut App, _kb, ev| app.on_key(&ev, true, true)),
                )
                .ok();
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for App {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        self.kb_focus = Some(surface.clone());
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {
        self.kb_focus = None;
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        // A Super tap toggles the launcher. While it is open it has the keyboard,
        // so the second tap arrives here and closes it. Closing on press is safe
        // when the compositor opens on press, because only the release comes after.
        if matches!(event.keysym, Keysym::Super_L | Keysym::Super_R)
            && matches!(self.shared.overlay, Some(Overlay::Launcher))
        {
            self.close_overlay();
            return;
        }
        self.on_key(&event, true, false);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.on_key(&event, true, true);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.on_key(&event, false, false);
    }
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        self.mods = egui::Modifiers { alt: m.alt, ctrl: m.ctrl, shift: m.shift, mac_cmd: false, command: m.ctrl };
        self.logo = m.logo;
    }
}

impl App {
    fn on_key(&mut self, ev: &KeyEvent, pressed: bool, repeat: bool) {
        let Some(focus) = self.kb_focus.clone() else { return };
        let Some(i) = self.win_index(&focus) else { return };
        // Naming a shortcut: the next real key press is the answer, not input.
        if self.shared.recording_key && self.wins[i].kind == Kind::Settings {
            if pressed && !repeat {
                let raw = ev.keysym.raw();
                if raw == 0xff1b && !(self.mods.alt || self.mods.ctrl || self.mods.shift || self.logo) {
                    self.shared.captured_key = Some(String::new());
                } else if let Some(name) = crate::keybinds::key_name(raw, self.logo, self.mods.alt, self.mods.ctrl, self.mods.shift) {
                    self.shared.captured_key = Some(name);
                }
                self.wins[i].dirty = true;
            }
            return;
        }
        let mods = self.mods;
        let w = &mut self.wins[i];
        if let Some(key) = keysym_to_key(ev.keysym) {
            w.events.push(egui::Event::Key { key, physical_key: Some(key), pressed, repeat, modifiers: mods });
        }
        if pressed && !mods.ctrl && !mods.alt {
            if let Some(t) = &ev.utf8 {
                if !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                    w.events.push(egui::Event::Text(t.clone()));
                }
            }
        }
        w.dirty = true;
    }
}

fn keysym_to_key(k: Keysym) -> Option<egui::Key> {
    use egui::Key;
    Some(match k {
        Keysym::Return | Keysym::KP_Enter => Key::Enter,
        Keysym::Escape => Key::Escape,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Delete => Key::Delete,
        Keysym::Tab => Key::Tab,
        Keysym::Left => Key::ArrowLeft,
        Keysym::Right => Key::ArrowRight,
        Keysym::Up => Key::ArrowUp,
        Keysym::Down => Key::ArrowDown,
        Keysym::Home => Key::Home,
        Keysym::End => Key::End,
        Keysym::space => Key::Space,
        other => {
            let c = other.key_char()?;
            if c.is_ascii_alphanumeric() {
                Key::from_name(&c.to_ascii_uppercase().to_string())?
            } else {
                return None;
            }
        }
    })
}

impl PointerHandler for App {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            let Some(i) = self.win_index(&ev.surface) else { continue };
            if let PointerEventKind::Enter { serial } = ev.kind {
                // The compositor may still show the cursor of whatever was under the
                // pointer before, such as a window border. Say which cursor to use.
                self.pointer_on = Some((ev.surface.clone(), serial));
                self.cursor_shape = None;
                self.apply_cursor(egui::CursorIcon::Default);
                if let Some(i) = self.win_index(&ev.surface) {
                    self.wins[i].dirty = true;
                }
            }
            if matches!(ev.kind, PointerEventKind::Leave { .. }) {
                self.pointer_on = None;
            }
            if matches!(ev.kind, PointerEventKind::Enter { .. }) {
                if let Some(o) = self.wins[i].output.clone() {
                    self.shared.last_output = Some(o);
                }
            }
            let mods = self.mods;
            let w = &mut self.wins[i];
            let pos = egui::pos2(ev.position.0 as f32, ev.position.1 as f32);
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => w.events.push(egui::Event::PointerMoved(pos)),
                PointerEventKind::Leave { .. } => w.events.push(egui::Event::PointerGone),
                PointerEventKind::Press { button, .. } | PointerEventKind::Release { button, .. } => {
                    let pressed = matches!(ev.kind, PointerEventKind::Press { .. });
                    let b = match button {
                        0x110 => egui::PointerButton::Primary,
                        0x111 => egui::PointerButton::Secondary,
                        0x112 => egui::PointerButton::Middle,
                        _ => continue,
                    };
                    w.events.push(egui::Event::PointerButton { pos, button: b, pressed, modifiers: mods });
                }
                PointerEventKind::Axis { vertical, horizontal, .. } => {
                    w.events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        source: egui::MouseWheelSource::Wheel,
                        delta: egui::vec2(-horizontal.absolute as f32, -vertical.absolute as f32),
                        phase: egui::TouchPhase::Move,
                        modifiers: mods,
                    });
                }
            }
            w.dirty = true;
        }
    }
}

delegate_registry!(App);
smithay_client_toolkit::delegate_dispatch2!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

#[allow(dead_code)]
fn _unused(_: &ui::Overlay) {}

/// Ask the running labwc to read rc.xml again.
fn reconfigure_labwc() {
    if std::env::var_os("OSMIA_NESTED").is_some() {
        return;
    }
    let _ = std::process::Command::new("labwc")
        .arg("--reconfigure")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|mut c| std::thread::spawn(move || {
            let _ = c.wait();
        }));
}

/// Open windows with readable app names, and the workspaces, for the window menu.
fn window_list(shared: &Shared) -> ui::WinList {
    let apps = shared.launch_data.as_ref().map(|d| d.apps.as_slice()).unwrap_or(&[]);
    let pretty = |app_id: &str| -> String {
        let norm = |s: &str| s.trim_end_matches(".desktop").to_lowercase();
        if let Some(a) = apps.iter().find(|a| norm(&a.id) == norm(app_id)) {
            return a.name.clone();
        }
        let last = app_id.rsplit('.').next().unwrap_or(app_id);
        let mut c = last.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => "Window".to_owned(),
        }
    };
    let wins = shared
        .toplevels
        .iter()
        .enumerate()
        .filter(|(_, t)| t.ready)
        .map(|(index, t)| ui::WinEntry {
            index,
            title: if t.title.is_empty() { pretty(&t.app_id) } else { t.title.clone() },
            app_id: t.app_id.clone(),
            app_name: pretty(&t.app_id),
            minimized: t.minimized,
            active: t.activated,
            alert: t.in_alert(),
            ws: t.ws.clone(),
        })
        .collect();
    let spaces = shared
        .workspaces
        .iter()
        .enumerate()
        .map(|(index, w)| ui::WsEntry { index, name: w.label(index), active: w.active })
        .collect();
    ui::WinList { wins, spaces }
}
