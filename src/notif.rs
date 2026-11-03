//! The notification daemon (org.freedesktop.Notifications) and its state.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay_client_toolkit::reexports::calloop::channel::Sender;
use zbus::OwnedVariant as OwnedValue;

static DEFAULT_TIMEOUT_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(5000);

/// The popup time for notifications that do not ask for their own. Changes apply to new ones.
pub fn set_default_timeout(ms: u64) {
    DEFAULT_TIMEOUT_MS.store(ms, Ordering::Relaxed);
}

pub const URGENCY_CRITICAL: u8 = 2;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    /// (key, label)
    pub actions: Vec<(String, String)>,
    pub urgency: u8,
    pub desktop_entry: Option<String>,
    pub transient: bool,
    pub timeout: Option<Duration>,
    pub born: Instant,
    pub unread: bool,
}

impl Notification {
    /// Names to try for this app's icon, best first.
    pub fn icon_key(&self) -> String {
        [self.app_icon.as_str(), self.desktop_entry.as_deref().unwrap_or(""), self.app_name.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn default_action(&self) -> Option<&str> {
        self.actions.iter().find(|(k, _)| k == "default").map(|(k, _)| k.as_str())
    }
    pub fn buttons(&self) -> impl Iterator<Item = &(String, String)> {
        self.actions.iter().filter(|(k, _)| k != "default")
    }
}

pub enum Event {
    Notify(Notification, u32 /* replaces_id */),
    Close(u32),
}

pub struct Server {
    tx: Sender<Event>,
    next_id: Arc<AtomicU32>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<String> {
        ["body", "body-markup", "actions", "icon-static"].map(String::from).to_vec()
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        };
        let urgency = hints
            .get("urgency")
            .and_then(|v| u8::try_from(v).ok())
            .unwrap_or(1);
        let desktop_entry = hints
            .get("desktop-entry")
            .and_then(|v| <&str>::try_from(v).ok())
            .map(str::to_owned);
        let transient = hints
            .get("transient")
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false);
        let timeout = match expire_timeout {
            0 => None,
            t if t > 0 => Some(Duration::from_millis(t as u64)),
            _ => Some(Duration::from_millis(DEFAULT_TIMEOUT_MS.load(Ordering::Relaxed))),
        };
        // A critical notification never expires by itself.
        let timeout = if urgency == URGENCY_CRITICAL { None } else { timeout };
        let actions = actions
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (c[0].clone(), c[1].clone()))
            .collect();
        let n = Notification {
            id,
            app_name,
            app_icon,
            summary,
            body,
            actions,
            urgency,
            desktop_entry,
            transient,
            timeout,
            born: Instant::now(),
            unread: false,
        };
        let _ = self.tx.send(Event::Notify(n, replaces_id));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.tx.send(Event::Close(id));
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        ("osmia".into(), "osmia".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

/// Start the daemon. Returns the connection for emitting signals, or an
/// error when another daemon already owns the name.
pub fn start(tx: Sender<Event>, default_timeout_ms: u64) -> Result<zbus::Connection, String> {
    set_default_timeout(default_timeout_ms);
    let server = Server {
        tx,
        next_id: Arc::new(AtomicU32::new(1)),
    };
    let build = async {
        zbus::connection::Builder::session()
            .name("org.freedesktop.Notifications")
            .serve_at("/org/freedesktop/Notifications", server)
            .build()
            .await
    };
    pollster::block_on(build).map_err(|e| e.to_string())
}

/// Make sure we still own the notification name. Another shell that starts later can
/// take it, and when that one exits nobody owns it. Ask for it again in that case.
/// Returns true when we own it afterwards.
pub fn reclaim(conn: &zbus::Connection) -> bool {
    pollster::block_on(async {
        let me = conn.unique_name().map(|n| n.to_string());
        let name = "org.freedesktop.Notifications";
        let dbus = zbus::fdo::DBusProxy::new(conn).await.ok()?;
        let bus_name = zbus::names::BusName::try_from(name).ok()?;
        match dbus.get_name_owner(bus_name).await {
            Ok(owner) => Some(Some(owner.to_string()) == me),
            Err(_) => conn.request_name(name).await.ok().map(|_| true),
        }
    })
    .unwrap_or(false)
}

pub fn emit_closed(conn: &zbus::Connection, id: u32, reason: u32) {
    let _ = pollster::block_on(conn.emit_signal(
        None::<&str>,
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        "NotificationClosed",
        &(id, reason),
    ));
}

pub fn emit_action(conn: &zbus::Connection, id: u32, key: &str) {
    let _ = pollster::block_on(conn.emit_signal(
        None::<&str>,
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        "ActionInvoked",
        &(id, key),
    ));
}

/// Close reasons from the specification.
pub const REASON_EXPIRED: u32 = 1;
pub const REASON_DISMISSED: u32 = 2;
pub const REASON_CLOSED_BY_APP: u32 = 3;

pub struct Notifs {
    /// Popups on screen, oldest first. The newest is last.
    pub popups: Vec<Notification>,
    /// Kept notifications, newest first.
    pub history: VecDeque<Notification>,
    pub history_cap: usize,
    pub dnd: bool,
    /// True when another daemon owned the name at start.
    pub blocked: bool,
}

impl Notifs {
    pub fn new(history_cap: usize) -> Self {
        Notifs {
            popups: Vec::new(),
            history: VecDeque::new(),
            history_cap,
            dnd: false,
            blocked: false,
        }
    }

    pub fn unread(&self) -> usize {
        self.history.iter().filter(|n| n.unread).count()
    }

    /// Add a notification. Returns true if a popup should show.
    pub fn add(&mut self, mut n: Notification, replaces: u32) -> bool {
        // Replace in place when the id is already on screen or in history.
        if replaces != 0 {
            self.popups.retain(|p| p.id != replaces);
            self.history.retain(|h| h.id != replaces);
        }
        let show = !self.dnd || n.urgency == URGENCY_CRITICAL;
        if !show {
            n.unread = true;
        }
        if !n.transient {
            let mut kept = n.clone();
            kept.unread = !show;
            self.history.push_front(kept);
            while self.history.len() > self.history_cap {
                self.history.pop_back();
            }
        }
        if show {
            self.popups.push(n);
        }
        show
    }

    /// Take a popup off the screen. `to_history_unread` marks the kept copy unread.
    pub fn remove_popup(&mut self, id: u32, mark_unread: bool) -> Option<Notification> {
        let pos = self.popups.iter().position(|p| p.id == id)?;
        let n = self.popups.remove(pos);
        if mark_unread {
            if let Some(h) = self.history.iter_mut().find(|h| h.id == id) {
                h.unread = true;
            }
        }
        Some(n)
    }

    /// Popups whose timer ran out. Critical ones have no timeout.
    pub fn expired(&self, now: Instant, hover_pause: bool) -> Vec<u32> {
        if hover_pause {
            return vec![];
        }
        self.popups
            .iter()
            .filter(|p| p.timeout.is_some_and(|t| now.duration_since(p.born) >= t))
            .map(|p| p.id)
            .collect()
    }

    pub fn next_expiry(&self) -> Option<Instant> {
        self.popups
            .iter()
            .filter_map(|p| p.timeout.map(|t| p.born + t))
            .min()
    }
}

/// Keep only bold, italic and underline markup; strip every other tag.
pub fn strip_markup(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(id: u32, urgency: u8, transient: bool, timeout_ms: Option<u64>) -> Notification {
        Notification {
            id,
            app_name: "app".into(),
            app_icon: String::new(),
            summary: "s".into(),
            body: "b".into(),
            actions: vec![],
            urgency,
            desktop_entry: None,
            transient,
            timeout: timeout_ms.map(Duration::from_millis),
            born: Instant::now(),
            unread: false,
        }
    }

    #[test]
    fn dnd_hides_normal_but_not_critical() {
        let mut n = Notifs::new(100);
        n.dnd = true;
        assert!(!n.add(make(1, 1, false, Some(5000)), 0));
        assert!(n.add(make(2, URGENCY_CRITICAL, false, None), 0));
        assert_eq!(n.popups.len(), 1);
        assert_eq!(n.history.len(), 2);
        assert_eq!(n.unread(), 1);
    }

    #[test]
    fn transient_is_not_kept_and_history_is_capped() {
        let mut n = Notifs::new(3);
        n.add(make(1, 1, true, Some(5000)), 0);
        assert!(n.history.is_empty());
        for i in 2..8 {
            n.add(make(i, 1, false, Some(5000)), 0);
        }
        assert_eq!(n.history.len(), 3);
        assert_eq!(n.history.front().unwrap().id, 7);
    }

    #[test]
    fn replace_by_id_updates_in_place() {
        let mut n = Notifs::new(100);
        n.add(make(5, 1, false, Some(5000)), 0);
        n.add(make(5, 1, false, Some(5000)), 5);
        assert_eq!(n.popups.len(), 1);
        assert_eq!(n.history.len(), 1);
    }

    #[test]
    fn critical_never_expires_and_timers_expire() {
        let mut n = Notifs::new(100);
        n.add(make(1, 1, false, Some(0)), 0);
        n.add(make(2, URGENCY_CRITICAL, false, None), 0);
        let later = Instant::now() + Duration::from_secs(60);
        assert_eq!(n.expired(later, false), vec![1]);
        assert!(n.expired(later, true).is_empty());
    }

    #[test]
    fn markup_is_stripped() {
        assert_eq!(strip_markup("<b>Hi</b> &amp; <a href=\"x\">bye</a>"), "Hi & bye");
    }
}
