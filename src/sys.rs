//! Readers for the machine state the bar shows. All of them are cheap file
//! reads or one small command, and the shell calls them only on its slow tick.

use std::fs;
use std::process::Command;

#[derive(Default, Clone, PartialEq)]
pub struct Stats {
    pub cpu_percent: u32,
    pub ram_percent: u32,
    pub battery: Option<Battery>,
    pub volume: Option<Volume>,
    pub brightness_percent: Option<u32>,
    pub network: Option<Net>,
}

#[derive(Clone, PartialEq)]
pub enum Net {
    Wifi { ssid: String, signal: u32 },
    Wired,
    Offline,
}

#[derive(Clone, PartialEq)]
pub struct Battery {
    pub percent: u32,
    pub charging: bool,
}

#[derive(Clone, PartialEq)]
pub struct Volume {
    pub percent: u32,
    pub muted: bool,
}

pub struct Sampler {
    prev_total: u64,
    prev_idle: u64,
    tick: u64,
    last: Stats,
    brightness_set: Option<(u32, std::time::Instant)>,
}

impl Sampler {
    pub fn new() -> Self {
        let (t, i) = read_cpu().unwrap_or((0, 0));
        Sampler {
            prev_total: t,
            prev_idle: i,
            tick: 0,
            last: Stats::default(),
            brightness_set: None,
        }
    }

    /// Read the cheap sources: files in /proc and /sys only.
    /// Volume and network come from a background thread. Keep the latest.
    pub fn set_slow(&mut self, volume: Option<Volume>, network: Option<Net>) {
        self.last.volume = volume;
        self.last.network = network;
    }

    pub fn set_brightness(&mut self, percent: u32) {
        self.brightness_set = Some((percent, std::time::Instant::now()));
        self.last.brightness_percent = Some(percent);
    }

    pub fn set_volume(&mut self, volume: Option<Volume>) {
        self.last.volume = volume;
    }

    pub fn sample(&mut self) -> Stats {
        let mut s = self.last.clone();
        if let Some((total, idle)) = read_cpu() {
            let dt = total.saturating_sub(self.prev_total);
            let di = idle.saturating_sub(self.prev_idle);
            if dt > 0 {
                s.cpu_percent = (100 * dt.saturating_sub(di) / dt) as u32;
            }
            self.prev_total = total;
            self.prev_idle = idle;
        }
        s.ram_percent = read_ram().unwrap_or(0);
        s.battery = read_battery();
        s.brightness_percent = read_brightness();
        // Right after a slider change the file may not show the new value yet.
        if let Some((v, at)) = self.brightness_set {
            if at.elapsed() < std::time::Duration::from_millis(1500) {
                s.brightness_percent = Some(v);
            } else {
                self.brightness_set = None;
            }
        }
        self.tick += 1;
        self.last = s.clone();
        s
    }
}

fn read_cpu() -> Option<(u64, u64)> {
    let text = fs::read_to_string("/proc/stat").ok()?;
    let line = text.lines().next()?;
    let v: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse().ok())
        .collect();
    if v.len() < 5 {
        return None;
    }
    let idle = v[3] + v.get(4).copied().unwrap_or(0);
    Some((v.iter().sum(), idle))
}

fn read_ram() -> Option<u32> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = 0u64;
    let mut avail = 0u64;
    for l in text.lines() {
        let mut it = l.split_whitespace();
        match it.next()? {
            "MemTotal:" => total = it.next()?.parse().ok()?,
            "MemAvailable:" => avail = it.next()?.parse().ok()?,
            _ => {}
        }
    }
    if total == 0 {
        return None;
    }
    Some((100 * (total - avail) / total) as u32)
}

fn read_battery() -> Option<Battery> {
    for entry in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = entry.path();
        if fs::read_to_string(p.join("type")).ok()?.trim() != "Battery" {
            continue;
        }
        // Skip batteries of attached devices, such as a wireless keyboard.
        if fs::read_to_string(p.join("scope")).is_ok_and(|s| s.trim() == "Device") {
            continue;
        }
        let percent = fs::read_to_string(p.join("capacity")).ok()?.trim().parse().ok()?;
        let status = fs::read_to_string(p.join("status")).unwrap_or_default();
        return Some(Battery {
            percent,
            charging: matches!(status.trim(), "Charging" | "Full"),
        });
    }
    None
}

fn read_brightness() -> Option<u32> {
    let entry = fs::read_dir("/sys/class/backlight").ok()?.flatten().next()?;
    let p = entry.path();
    let cur: f64 = fs::read_to_string(p.join("brightness")).ok()?.trim().parse().ok()?;
    let max: f64 = fs::read_to_string(p.join("max_brightness")).ok()?.trim().parse().ok()?;
    if max <= 0.0 {
        return None;
    }
    Some((100.0 * cur / max).round() as u32)
}

fn read_volume() -> Option<Volume> {
    let out = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    // Example: "Volume: 0.64" or "Volume: 0.64 [MUTED]"
    let v: f64 = text.split_whitespace().nth(1)?.parse().ok()?;
    Some(Volume {
        percent: (v * 100.0).round() as u32,
        muted: text.contains("MUTED"),
    })
}

static VOLUME_POKE: std::sync::OnceLock<std::sync::Mutex<std::sync::mpsc::Sender<()>>> = std::sync::OnceLock::new();

fn poke_volume() {
    if let Some(m) = VOLUME_POKE.get() {
        if let Ok(t) = m.lock() {
            let _ = t.send(());
        }
    }
}

pub fn set_volume_step(delta_percent: i32) {
    let arg = if delta_percent >= 0 {
        format!("{}%+", delta_percent)
    } else {
        format!("{}%-", -delta_percent)
    };
    std::thread::spawn(move || {
        let _ = Command::new("wpctl")
            .args(["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", &arg])
            .status();
        poke_volume();
    });
}

pub fn toggle_mute() {
    std::thread::spawn(|| {
        let _ = Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
            .status();
        poke_volume();
    });
}

fn read_network() -> Option<Net> {
    let mut wired = false;
    let mut wifi_up = false;
    for e in fs::read_dir("/sys/class/net").ok()?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        let p = e.path();
        let up = fs::read_to_string(p.join("operstate")).is_ok_and(|s| s.trim() == "up");
        if !up {
            continue;
        }
        if p.join("wireless").exists() {
            wifi_up = true;
        } else if p.join("device").exists() {
            wired = true;
        }
    }
    if wifi_up {
        let out = Command::new("nmcli")
            .args(["-t", "-f", "active,ssid,signal", "dev", "wifi"])
            .output()
            .ok();
        if let Some(out) = out {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let mut it = line.splitn(3, ':');
                if it.next() == Some("yes") {
                    let ssid = it.next().unwrap_or("Wi-Fi").to_owned();
                    let signal = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    return Some(Net::Wifi { ssid, signal });
                }
            }
        }
        return Some(Net::Wifi { ssid: "Wi-Fi".into(), signal: 0 });
    }
    Some(if wired { Net::Wired } else { Net::Offline })
}

/// Change the backlight through logind, so no root rights are needed.
static BRIGHTNESS_TX: std::sync::OnceLock<std::sync::mpsc::Sender<u32>> = std::sync::OnceLock::new();

/// Set the backlight to a percentage. One worker applies only the newest value,
/// so dragging a slider never queues old values behind new ones.
pub fn set_brightness_percent(percent: u32) {
    let tx = BRIGHTNESS_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        std::thread::spawn(move || {
            while let Ok(mut p) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    p = newer;
                }
                set_brightness_to(p);
            }
        });
        tx
    });
    let _ = tx.send(percent);
}

fn set_brightness_to(percent: u32) {
    let Some(entry) = fs::read_dir("/sys/class/backlight").ok().and_then(|mut d| d.next()).and_then(|e| e.ok()) else {
        return;
    };
    let p = entry.path();
    let name = entry.file_name().to_string_lossy().into_owned();
    let max: i64 = fs::read_to_string(p.join("max_brightness")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    if max <= 0 {
        return;
    }
    // Never go fully dark.
    let new = (max * percent.min(100) as i64 / 100).clamp((max / 100).max(1), max) as u32;
    let call = async move {
        let conn = zbus::Connection::system().await?;
        conn.call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1/session/auto",
            Some("org.freedesktop.login1.Session"),
            "SetBrightness",
            &("backlight", name.as_str(), new),
        )
        .await
        .map(|_| ())
    };
    if let Err(e) = pollster::block_on(call) {
        log::warn!("cannot set brightness: {e}");
    }
}

pub fn set_brightness_step(delta_percent: i32) {
    let Some(entry) = fs::read_dir("/sys/class/backlight").ok().and_then(|mut d| d.next()).and_then(|e| e.ok()) else {
        return;
    };
    let p = entry.path();
    let name = entry.file_name().to_string_lossy().into_owned();
    let cur: i64 = fs::read_to_string(p.join("brightness")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let max: i64 = fs::read_to_string(p.join("max_brightness")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    if max <= 0 {
        return;
    }
    let step = (max * delta_percent as i64 / 100).abs().max(1) * delta_percent.signum() as i64;
    // Never go fully dark.
    let new = (cur + step).clamp((max / 100).max(1), max) as u32;
    let call = async move {
        let conn = zbus::Connection::system().await?;
        conn.call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1/session/auto",
            Some("org.freedesktop.login1.Session"),
            "SetBrightness",
            &("backlight", name.as_str(), new),
        )
        .await
        .map(|_| ())
    };
    std::thread::spawn(move || {
        if let Err(e) = pollster::block_on(call) {
            log::warn!("cannot set brightness: {e}");
        }
    });
}

/// Suspend, reboot and power off go through logind on D-Bus.
pub fn logind_call(method: &str) {
    let method = method.to_owned();
    let call = async move {
        let conn = zbus::Connection::system().await?;
        conn.call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            Some("org.freedesktop.login1.Manager"),
            method.as_str(),
            &(true,),
        )
        .await
        .map(|_| ())
    };
    std::thread::spawn(move || {
        if let Err(e) = pollster::block_on(call) {
            log::warn!("logind call failed: {e}");
        }
    });
}

/// Whether logind says this machine can hibernate (it needs enough swap and kernel
/// support). Asked once in the background at startup, so it is false until that
/// answer arrives.
pub fn can_hibernate() -> bool {
    static ANSWER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static ASKED: std::sync::Once = std::sync::Once::new();
    if let Some(a) = ANSWER.get() {
        return *a;
    }
    ASKED.call_once(|| {
        std::thread::spawn(|| {
            let ask = async {
                let conn = zbus::Connection::system().await?;
                let r = conn
                    .call_method(Some("org.freedesktop.login1"), "/org/freedesktop/login1", Some("org.freedesktop.login1.Manager"), "CanHibernate", &())
                    .await?;
                zbus::Result::Ok(r.body().deserialize::<String>()?)
            };
            let yes = pollster::block_on(ask).map(|s| s == "yes" || s == "challenge").unwrap_or(false);
            let _ = ANSWER.set(yes);
        });
    });
    false
}

pub struct Slow {
    pub volume: Option<Volume>,
    pub network: Option<Net>,
    /// True when only the volume was read, so the network value is not new.
    pub volume_only: bool,
}

/// Make a long-running helper exit when osmia does. Without this, every
/// restart leaves a `pactl subscribe` behind, and enough of them make
/// pipewire-pulse refuse new clients, which silences all audio.
fn die_with_parent(cmd: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    cmd
}

/// Volume and network are read by external commands that can be slow, so they
/// run on their own threads. `pactl subscribe` and `nmcli monitor` print a line
/// when something changes, so nothing polls. A one-minute timer is a safety net.
/// Returns a handle that asks for a refresh now.
pub fn start_slow(tx: smithay_client_toolkit::reexports::calloop::channel::Sender<Slow>) -> std::sync::mpsc::Sender<()> {
    let tx_volume = tx.clone();
    use std::io::{BufRead, BufReader};
    use std::time::Duration;
    let (poke_tx, poke_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || loop {
        let slow = Slow { volume: read_volume(), network: read_network(), volume_only: false };
        if tx.send(slow).is_err() {
            break;
        }
        match poke_rx.recv_timeout(Duration::from_secs(60)) {
            Ok(()) => {
                // Let a burst of events settle, then read once.
                std::thread::sleep(Duration::from_millis(300));
                while poke_rx.try_recv().is_ok() {}
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    });
    // Volume has its own path with no settle delay, so the bar follows the keys at once.
    let (vol_tx, vol_rx) = std::sync::mpsc::channel::<()>();
    let vtx = tx_volume;
    std::thread::spawn(move || {
        while vol_rx.recv().is_ok() {
            while vol_rx.try_recv().is_ok() {}
            let slow = Slow { volume: read_volume(), network: None, volume_only: true };
            if vtx.send(slow).is_err() {
                break;
            }
        }
    });
    std::thread::spawn({
        let vol_tx = vol_tx.clone();
        move || {
            let Ok(mut child) = die_with_parent(Command::new("pactl").arg("subscribe"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
            else {
                return;
            };
            let Some(out) = child.stdout.take() else { return };
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if line.contains("sink") || line.contains("server") {
                    let _ = vol_tx.send(());
                }
            }
        }
    });
    VOLUME_POKE.get_or_init(|| std::sync::Mutex::new(vol_tx));
    for (cmd, args, filter) in [("nmcli", vec!["monitor"], "")] {
        let poke = poke_tx.clone();
        std::thread::spawn(move || {
            let Ok(mut child) = die_with_parent(Command::new(cmd).args(&args))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
            else {
                return;
            };
            let Some(out) = child.stdout.take() else { return };
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if filter.is_empty() || line.contains(filter) {
                    let _ = poke.send(());
                }
            }
        });
    }
    poke_tx
}

#[derive(Clone, Debug)]
pub struct WifiNet {
    pub ssid: String,
    pub signal: u32,
    pub secure: bool,
    pub saved: bool,
    pub active: bool,
}

/// Split an nmcli terse line. Colons inside values are escaped as "\:".
fn nm_fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut esc = false;
    for c in line.chars() {
        if esc {
            out.last_mut().unwrap().push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ':' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    out
}

/// Read nearby and saved Wi-Fi networks. Runs on its own thread.
pub fn scan_wifi(tx: smithay_client_toolkit::reexports::calloop::channel::Sender<Vec<WifiNet>>) {
    std::thread::spawn(move || {
        let saved: Vec<String> = Command::new("nmcli")
            .args(["-t", "-f", "NAME,TYPE", "connection", "show"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(nm_fields)
                    .filter(|f| f.get(1).is_some_and(|t| t.contains("wireless")))
                    .map(|f| f[0].clone())
                    .collect()
            })
            .unwrap_or_default();
        let list = Command::new("nmcli")
            .args(["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let mut nets: Vec<WifiNet> = Vec::new();
        for line in list.lines() {
            let f = nm_fields(line);
            if f.len() < 4 || f[1].is_empty() {
                continue;
            }
            let net = WifiNet {
                ssid: f[1].clone(),
                signal: f[2].parse().unwrap_or(0),
                secure: !f[3].trim().is_empty() && f[3].trim() != "--",
                saved: saved.contains(&f[1]),
                active: f[0] == "*",
            };
            // Keep the strongest entry for a name seen on several access points.
            match nets.iter_mut().find(|n| n.ssid == net.ssid) {
                Some(old) => {
                    if net.active || (!old.active && net.signal > old.signal) {
                        *old = net;
                    }
                }
                None => nets.push(net),
            }
        }
        nets.sort_by(|a, b| b.active.cmp(&a.active).then(b.saved.cmp(&a.saved)).then(b.signal.cmp(&a.signal)));
        let _ = tx.send(nets);
    });
}

fn say(summary: &str, body: &str) {
    let _ = Command::new("notify-send").args(["-a", "Wi-Fi", summary, body]).status();
}

fn nm_error(out: &std::process::Output) -> String {
    let e = String::from_utf8_lossy(&out.stderr);
    let e = e.trim().trim_start_matches("Error: ").to_owned();
    if e.is_empty() { "NetworkManager gave no reason.".into() } else { e }
}

pub fn connect_wifi(ssid: &str, saved: bool) {
    let ssid = ssid.to_owned();
    std::thread::spawn(move || {
        let args: Vec<&str> = if saved {
            vec!["connection", "up", "id", &ssid]
        } else {
            vec!["dev", "wifi", "connect", &ssid]
        };
        match Command::new("nmcli").args(args).output() {
            Ok(o) if !o.status.success() => say(&format!("Could not join {ssid}"), &nm_error(&o)),
            Err(e) => say("Wi-Fi", &format!("nmcli did not run: {e}")),
            _ => {}
        }
    });
}

/// Join a network that needs a password. The password goes in on stdin, so it
/// never shows in the process list. A wrong one leaves no saved profile behind.
pub fn join_wifi(ssid: &str, password: &str) {
    let (ssid, password) = (ssid.to_owned(), password.to_owned());
    std::thread::spawn(move || {
        use std::io::Write;
        use std::process::Stdio;
        let child = Command::new("nmcli")
            .args(["--ask", "dev", "wifi", "connect", &ssid])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let Ok(mut child) = child else {
            say("Wi-Fi", "nmcli did not run.");
            return;
        };
        if let Some(mut si) = child.stdin.take() {
            let _ = writeln!(si, "{password}");
        }
        match child.wait_with_output() {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let _ = Command::new("nmcli").args(["connection", "delete", "id", &ssid]).output();
                say(&format!("Could not join {ssid}"), &nm_error(&o));
            }
            Err(e) => say("Wi-Fi", &format!("nmcli failed: {e}")),
        }
    });
}

// ---------- audio devices ----------

#[derive(Clone, Debug, PartialEq)]
pub struct AudioPort {
    pub name: String,
    pub desc: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioDev {
    pub name: String,
    pub desc: String,
    pub percent: u32,
    pub muted: bool,
    pub default: bool,
    /// Ports that can be used right now. Empty when the device has none.
    pub ports: Vec<AudioPort>,
    pub active_port: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioInfo {
    pub sinks: Vec<AudioDev>,
    pub sources: Vec<AudioDev>,
}

fn pactl_json(what: &str) -> Vec<serde_json::Value> {
    Command::new("pactl")
        .args(["-f", "json", "list", what])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Vec<serde_json::Value>>(&o.stdout).ok())
        .unwrap_or_default()
}

fn pactl_line(args: &[&str]) -> String {
    Command::new("pactl")
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

fn parse_devs(list: Vec<serde_json::Value>, default: &str, skip_monitors: bool) -> Vec<AudioDev> {
    list.into_iter()
        .filter_map(|d| {
            let name = d["name"].as_str()?.to_owned();
            if skip_monitors && (name.ends_with(".monitor") || d["properties"]["device.class"] == "monitor") {
                return None;
            }
            let percent = d["volume"]
                .as_object()
                .and_then(|m| m.values().next())
                .and_then(|c| c["value_percent"].as_str())
                .and_then(|s| s.trim_end_matches('%').parse().ok())
                .unwrap_or(0);
            let ports = d["ports"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|p| p["availability"] != "not available")
                        .filter_map(|p| {
                            Some(AudioPort { name: p["name"].as_str()?.to_owned(), desc: p["description"].as_str()?.to_owned() })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(AudioDev {
                desc: d["description"].as_str().unwrap_or(&name).to_owned(),
                default: name == default,
                name,
                percent,
                muted: d["mute"].as_bool().unwrap_or(false),
                ports,
                active_port: d["active_port"].as_str().map(str::to_owned),
            })
        })
        .collect()
}

pub fn read_audio() -> AudioInfo {
    let ds = pactl_line(&["get-default-sink"]);
    let dsrc = pactl_line(&["get-default-source"]);
    AudioInfo {
        sinks: parse_devs(pactl_json("sinks"), &ds, false),
        sources: parse_devs(pactl_json("sources"), &dsrc, true),
    }
}

/// Run pactl commands one after another on a thread, then read the devices again.
pub fn audio_run(
    cmds: Vec<Vec<String>>,
    done: Option<smithay_client_toolkit::reexports::calloop::channel::Sender<AudioInfo>>,
) {
    std::thread::spawn(move || {
        for c in cmds {
            let _ = Command::new("pactl").args(&c).status();
        }
        if let Some(tx) = done {
            let _ = tx.send(read_audio());
        }
    });
}

#[cfg(test)]
mod audio_tests {
    #[test]
    fn reads_devices() {
        let a = super::read_audio();
        eprintln!("{a:#?}");
        assert!(a.sinks.iter().all(|d| !d.name.ends_with(".monitor")));
    }
}

// ---------- battery details and power profiles ----------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatteryDetail {
    pub percent: u32,
    pub status: String,
    pub ac_online: bool,
    /// Watts, positive.
    pub power_w: f64,
    pub energy_wh: f64,
    pub full_wh: f64,
    pub design_wh: f64,
    pub cycles: Option<u32>,
    pub voltage_v: f64,
    pub charge_limit: Option<u32>,
    pub model: String,
    /// Minutes until empty or full, when it can be told.
    pub eta_minutes: Option<u32>,
}

impl BatteryDetail {
    pub fn health_percent(&self) -> Option<u32> {
        (self.design_wh > 0.0 && self.full_wh > 0.0).then(|| (100.0 * self.full_wh / self.design_wh).round() as u32)
    }
}

fn read_f(p: &std::path::Path, name: &str) -> Option<f64> {
    fs::read_to_string(p.join(name)).ok()?.trim().parse().ok()
}

pub fn read_battery_detail() -> Option<BatteryDetail> {
    let mut ac_online = false;
    let mut bat = None;
    for e in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = e.path();
        let ty = fs::read_to_string(p.join("type")).unwrap_or_default();
        match ty.trim() {
            "Mains" => {
                if read_f(&p, "online") == Some(1.0) {
                    ac_online = true;
                }
            }
            "Battery" if bat.is_none() => {
                if !fs::read_to_string(p.join("scope")).is_ok_and(|s| s.trim() == "Device") {
                    bat = Some(p);
                }
            }
            _ => {}
        }
    }
    let p = bat?;
    let uwh = |name: &str| read_f(&p, name).map(|v| v / 1e6).unwrap_or(0.0);
    let status = fs::read_to_string(p.join("status")).unwrap_or_default().trim().to_owned();
    let power_w = uwh("power_now").abs();
    let (energy_wh, full_wh) = (uwh("energy_now"), uwh("energy_full"));
    let eta_minutes = if power_w > 0.5 {
        match status.as_str() {
            "Discharging" => Some((60.0 * energy_wh / power_w).round() as u32),
            "Charging" => Some((60.0 * (full_wh - energy_wh).max(0.0) / power_w).round() as u32),
            _ => None,
        }
    } else {
        None
    };
    Some(BatteryDetail {
        percent: read_f(&p, "capacity").unwrap_or(0.0) as u32,
        status,
        ac_online,
        power_w,
        energy_wh,
        full_wh,
        design_wh: uwh("energy_full_design"),
        cycles: read_f(&p, "cycle_count").map(|c| c as u32).filter(|c| *c > 0),
        voltage_v: uwh("voltage_now"),
        charge_limit: read_f(&p, "charge_control_end_threshold").map(|c| c as u32).filter(|c| *c < 100),
        model: [fs::read_to_string(p.join("manufacturer")).unwrap_or_default(), fs::read_to_string(p.join("model_name")).unwrap_or_default()]
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        eta_minutes,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowerProfiles {
    pub active: String,
    pub available: Vec<String>,
}

pub fn read_profiles() -> PowerProfiles {
    let out = Command::new("powerprofilesctl").arg("list").output();
    let mut pp = PowerProfiles::default();
    if let Ok(o) = out {
        for l in String::from_utf8_lossy(&o.stdout).lines() {
            let t = l.trim_end();
            let active = t.starts_with("* ");
            if (active || t.starts_with("  ")) && t.ends_with(':') && !t.trim_start_matches("* ").trim().contains(char::is_whitespace) {
                let name = t.trim_start_matches("* ").trim().trim_end_matches(':').to_owned();
                if active {
                    pp.active = name.clone();
                }
                pp.available.push(name);
            }
        }
    }
    pp
}

pub fn profiles_run(
    set: Option<String>,
    done: smithay_client_toolkit::reexports::calloop::channel::Sender<PowerProfiles>,
) {
    std::thread::spawn(move || {
        if let Some(s) = set {
            let _ = Command::new("powerprofilesctl").args(["set", &s]).status();
        }
        let _ = done.send(read_profiles());
    });
}

#[cfg(test)]
mod power_tests {
    #[test]
    fn reads_power() {
        eprintln!("{:#?}\n{:#?}", super::read_battery_detail(), super::read_profiles());
    }
}

// ---------- what is using the CPU and memory ----------

#[derive(Clone, PartialEq, Default)]
pub struct Proc {
    pub name: String,
    /// Percent of one core. Can pass 100 for a program that uses several.
    pub cpu: f32,
    pub mem_kb: u64,
    pub count: u32,
    /// Each process in the group: pid, CPU percent, memory in kB.
    pub members: Vec<(u32, f32, u64)>,
}

#[derive(Clone, PartialEq, Default)]
pub struct ProcList {
    /// False until two samples exist, since CPU use is a difference.
    pub ready: bool,
    /// Every program, unsorted. The panel ranks them.
    pub all: Vec<Proc>,
    /// Names whose order is held in place, when the list is locked.
    pub lock: Option<Vec<String>>,
    /// Limits set on programs, by name.
    pub limits: std::collections::HashMap<String, crate::limits::Limit>,
    pub cores: u32,
    pub mem_total_kb: u64,
    pub mem_used_kb: u64,
}

#[derive(Default)]
pub struct ProcSampler {
    prev: std::collections::HashMap<u32, u64>,
    at: Option<std::time::Instant>,
}

impl ProcSampler {
    pub fn reset(&mut self) {
        self.prev.clear();
        self.at = None;
    }

    /// Read every process, group them by program name and rank them.
    pub fn sample(&mut self) -> ProcList {
        let now = std::time::Instant::now();
        let dt = self.at.map(|a| now.duration_since(a).as_secs_f32());
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f32;
        let page_kb = (unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64) / 1024;
        let mut groups: std::collections::HashMap<String, Proc> = Default::default();
        let mut cur: std::collections::HashMap<u32, u64> = Default::default();
        if let Ok(dir) = fs::read_dir("/proc") {
            for e in dir.flatten() {
                let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
                let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
                let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else { continue };
                let name = friendly_name(pid, &stat[open + 1..close]);
                let f: Vec<&str> = stat[close + 1..].split_whitespace().collect();
                let ticks = f.get(11).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
                    + f.get(12).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                cur.insert(pid, ticks);
                let mem_kb = fs::read_to_string(format!("/proc/{pid}/statm"))
                    .ok()
                    .and_then(|t| {
                        let mut it = t.split_whitespace().skip(1);
                        let res: u64 = it.next()?.parse().ok()?;
                        let shared: u64 = it.next()?.parse().ok()?;
                        Some(res.saturating_sub(shared) * page_kb)
                    })
                    .unwrap_or(0);
                let cpu = match (dt, self.prev.get(&pid)) {
                    (Some(dt), Some(p)) if dt > 0.0 => ticks.saturating_sub(*p) as f32 / hz / dt * 100.0,
                    _ => 0.0,
                };
                let g = groups.entry(name.clone()).or_insert_with(|| Proc { name, ..Default::default() });
                g.cpu += cpu;
                g.mem_kb += mem_kb;
                g.count += 1;
                g.members.push((pid, cpu, mem_kb));
            }
        }
        self.prev = cur;
        let ready = dt.is_some();
        self.at = Some(now);
        let all: Vec<Proc> = groups.into_values().filter(|p| p.mem_kb > 0 || p.cpu > 0.0).collect();
        let (mut total, mut avail) = (0u64, 0u64);
        if let Ok(t) = fs::read_to_string("/proc/meminfo") {
            for l in t.lines() {
                let mut it = l.split_whitespace();
                match it.next() {
                    Some("MemTotal:") => total = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                    Some("MemAvailable:") => avail = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                    _ => {}
                }
            }
        }
        let cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1);
        ProcList { ready, all, lock: None, limits: Default::default(), cores, mem_total_kb: total, mem_used_kb: total.saturating_sub(avail) }
    }
}

/// The kernel's name for a process is often just the interpreter ("node-MainThread",
/// "python3", "bash"). For those, name it after the script or tool it runs.
fn friendly_name(pid: u32, comm: &str) -> String {
    const GENERIC: &[&str] = &[
        "node-MainThread", "MainThread", "node", "nodejs", "python", "python3", "bash", "sh", "zsh", "fish",
        "java", "ruby", "perl", "deno", "bun", "dash",
    ];
    if !GENERIC.contains(&comm) && !comm.starts_with("python3.") {
        return comm.to_owned();
    }
    let Ok(raw) = fs::read(format!("/proc/{pid}/cmdline")) else { return comm.to_owned() };
    let args: Vec<String> = raw.split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        if matches!(a.as_str(), "--import" | "-r" | "--require" | "--loader" | "-m" | "-e" | "-p" | "--eval") {
            it.next();
            continue;
        }
        if a.starts_with('-') {
            continue;
        }
        let first = a.split_whitespace().next().unwrap_or(a);
        let base = first.rsplit('/').next().unwrap_or(first);
        if !base.is_empty() {
            return base.to_owned();
        }
    }
    comm.to_owned()
}

/// The command line of one process, for telling members of a group apart.
pub fn cmdline(pid: u32) -> String {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let text: Vec<String> = raw.split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    text.join(" ")
}

#[cfg(test)]
mod hibernate_tests {
    /// Asks the real logind: `cargo test -- --ignored hibernate`.
    #[test]
    #[ignore]
    fn asks_logind() {
        super::can_hibernate();
        std::thread::sleep(std::time::Duration::from_millis(800));
        let yes = super::can_hibernate();
        let out = std::process::Command::new("busctl").args(["call", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "CanHibernate"]).output().unwrap();
        let want = String::from_utf8_lossy(&out.stdout).contains("\"yes\"");
        assert_eq!(yes, want);
    }
}
