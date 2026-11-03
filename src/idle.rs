//! Screen timeout. osmia runs swayidle itself so the bar can change the timeout.
//! The choice is kept in ~/.local/state/osmia/screen-timeout (minutes, 0 = never).

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Choices shown in the menu, in minutes. 0 means never.
pub const CHOICES: [u32; 8] = [1, 2, 5, 10, 15, 30, 60, 0];
pub const DEFAULT_MINUTES: u32 = 5;

pub fn label(minutes: u32) -> String {
    match minutes {
        0 => "Never".into(),
        1 => "1 minute".into(),
        60 => "1 hour".into(),
        m => format!("{m} minutes"),
    }
}

fn path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("osmia").join("screen-timeout")
}

pub fn load() -> u32 {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(DEFAULT_MINUTES)
}

fn save(minutes: u32) {
    save_to("screen-timeout", minutes);
}

fn load_lock() -> u32 {
    std::fs::read_to_string(path().with_file_name("lock-timeout"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn save_to(name: &str, minutes: u32) {
    let p = path().with_file_name(name);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, format!("{minutes}\n"));
}

pub struct Idle {
    child: Option<Child>,
    pub minutes: u32,
    /// Lock the screen after this many idle minutes. 0 means never.
    pub lock_minutes: u32,
}

impl Idle {
    /// Take over from any swayidle the session started, then run our own.
    pub fn start() -> Idle {
        // A nested test session must not touch the real session's idle handling.
        if std::env::var_os("OSMIA_NO_IDLE").is_some() {
            return Idle { child: None, minutes: load(), lock_minutes: load_lock() };
        }
        let _ = Command::new("pkill").args(["-x", "swayidle"]).status();
        let mut idle = Idle { child: None, minutes: load(), lock_minutes: load_lock() };
        idle.spawn();
        idle
    }

    pub fn set_lock(&mut self, minutes: u32) {
        self.lock_minutes = minutes;
        save_to("lock-timeout", minutes);
        self.spawn();
    }

    pub fn set(&mut self, minutes: u32) {
        self.minutes = minutes;
        save(minutes);
        self.spawn();
    }

    fn spawn(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        let mut cmd = Command::new("swayidle");
        cmd.args(["-w", "lock", "swaylock -f", "before-sleep", "loginctl lock-session"]);
        if self.lock_minutes > 0 {
            cmd.args(["timeout", &(self.lock_minutes * 60).to_string(), "swaylock -f"]);
        }
        if self.minutes > 0 {
            cmd.args([
                "timeout",
                &(self.minutes * 60).to_string(),
                "wlopm --off '*'",
                "resume",
                "wlopm --on '*'",
            ]);
        }
        self.child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| log::warn!("swayidle did not start: {e}"))
            .ok();
    }
}

impl Drop for Idle {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
        }
    }
}
