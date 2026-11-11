//! The discrete graphics card, through the gpu-vfio-control service.
//!
//! The service (root, system bus) owns every driver bind and safety check.
//! The shell only reads its JSON and sends its fixed verbs; polkit decides
//! whether the user may. See spec/GraphicsCard.pi.

use std::time::Duration;

use futures_lite::StreamExt;
use serde::Deserialize;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use zbus::{message::Type, Connection, MatchRule, MessageStream};

const BUS: &str = "org.local.GpuVfioControl";
const PATH: &str = "/org/local/GpuVfioControl";
const IFACE: &str = "org.local.GpuVfioControl";
/// How long to wait before asking again after the service could not be reached.
const RETRY: Duration = Duration::from_secs(60);

// ---------- the wire ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    Host,
    Vfio,
    VmRunning,
    Switching,
    Error,
    #[default]
    Unknown,
}

impl<'de> Deserialize<'de> for Phase {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "linux" => Phase::Host,
            "vfio" => Phase::Vfio,
            "windows_running" => Phase::VmRunning,
            "transitioning" => Phase::Switching,
            "error" => Phase::Error,
            _ => Phase::Unknown,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Holder {
    pub pid: i32,
    pub name: String,
    /// Absent in a gpu_busy reply, where every listed process blocks.
    #[serde(default = "yes")]
    pub blocking: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub state: Phase,
    #[serde(default)]
    pub gpu_driver: Option<String>,
    #[serde(default)]
    pub vm_state: String,
    #[serde(default)]
    pub vm_name: String,
    #[serde(default)]
    pub processes: Vec<Holder>,
    #[serde(default)]
    pub preferred_state: Option<String>,
    #[serde(default)]
    pub preference_pending: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Reply {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub status: Option<Status>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub processes: Vec<Holder>,
    #[serde(default)]
    pub rolled_back: Option<bool>,
}

impl Reply {
    fn failure(text: String) -> Reply {
        Reply { error: Some("internal_error".into()), message: Some(text), ..Default::default() }
    }

    fn parse(json: &str) -> Reply {
        serde_json::from_str(json).unwrap_or_else(|e| Reply::failure(format!("unreadable reply from the service: {e}")))
    }

    /// One sentence on what went wrong. Names the holders when that is the reason.
    pub fn error_text(&self) -> String {
        let mut t = match (self.error.as_deref(), self.message.as_deref()) {
            (Some("gpu_busy"), _) if !self.processes.is_empty() => format!("In use by {}.", holders_text(&self.processes)),
            (Some("permission_denied"), _) => "Not allowed. The password prompt was cancelled or refused.".into(),
            (_, Some(m)) => m.to_owned(),
            (Some(code), None) => code.replace('_', " "),
            (None, None) => "Unknown error.".into(),
        };
        if self.rolled_back == Some(false) {
            t.push_str(" The card was left half switched. Run gpu-vfio-control status.");
        }
        t
    }
}

pub fn holders_text(list: &[Holder]) -> String {
    list.iter().filter(|h| h.blocking).map(|h| format!("{} ({})", h.name, h.pid)).collect::<Vec<_>>().join(", ")
}

// ---------- what the shell can ask ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    ToVms,
    ToHost,
    StartVm,
    StopVm,
    ForceOff,
    /// Hand the card over as soon as nothing holds it.
    PreferVfio,
    /// Stop waiting for that.
    PreferNone,
}

impl Cmd {
    fn call(self) -> (&'static str, Arg) {
        match self {
            Cmd::ToVms => ("AttachVfio", Arg::Force(false)),
            Cmd::ToHost => ("AttachLinux", Arg::Force(false)),
            Cmd::StartVm => ("StartVm", Arg::Force(false)),
            Cmd::StopVm => ("StopVm", Arg::Force(false)),
            Cmd::ForceOff => ("StopVm", Arg::Force(true)),
            Cmd::PreferVfio => ("SetPreferredState", Arg::State("vfio")),
            Cmd::PreferNone => ("SetPreferredState", Arg::State("none")),
        }
    }

    /// What a notification says when it worked.
    pub fn done_text(self, vm: &str) -> String {
        match self {
            Cmd::ToVms => "Ready for VMs. The card is on vfio-pci.".into(),
            Cmd::ToHost => "Back on the host.".into(),
            Cmd::StartVm => format!("{vm} is starting with the card."),
            Cmd::StopVm => format!("{vm} was asked to shut down."),
            Cmd::ForceOff => format!("{vm} was powered off."),
            Cmd::PreferVfio => "The card goes to VMs as soon as nothing holds it.".into(),
            Cmd::PreferNone => "No longer waiting to hand the card over.".into(),
        }
    }
}

enum Arg {
    Force(bool),
    State(&'static str),
}

// ---------- state the UI draws ----------

#[derive(Default)]
pub struct State {
    /// None until the service answers, and while it cannot be reached.
    pub status: Option<Status>,
    /// The request this shell is waiting on.
    pub pending: Option<Cmd>,
    /// The last request failed, and why. Cleared by the next one.
    pub error: Option<String>,
    /// Force off was clicked once and waits for the second click.
    pub confirm_force: bool,
}

impl State {
    /// The word for `osmia gpu status`.
    pub fn word(&self) -> &'static str {
        match self.status.as_ref().map(|s| s.state) {
            None => "unavailable",
            Some(Phase::Host) => "host",
            Some(Phase::Vfio) => "vfio",
            Some(Phase::VmRunning) => "vm",
            Some(Phase::Switching) => "switching",
            Some(Phase::Error) => "error",
            Some(Phase::Unknown) => "unknown",
        }
    }
}

pub enum Msg {
    /// New state from the service, pushed or asked for.
    Status(Status),
    /// The service cannot be reached.
    Gone,
    /// The answer to a request.
    Done(Cmd, Reply),
}

// ---------- the worker ----------

/// Start watching the service. Returns a sender for requests.
pub fn start(tx: Sender<Msg>) -> std::sync::mpsc::Sender<Cmd> {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<Cmd>();
    {
        let tx = tx.clone();
        std::thread::spawn(move || loop {
            if let Err(e) = pollster::block_on(watch(&tx)) {
                log::debug!("graphics card service: {e}");
            }
            if tx.send(Msg::Gone).is_err() {
                return;
            }
            std::thread::sleep(RETRY);
        });
    }
    std::thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            // Each on its own thread: a polkit prompt can wait as long as the user likes.
            let tx = tx.clone();
            std::thread::spawn(move || {
                let reply = pollster::block_on(request(cmd)).unwrap_or_else(|e| Reply::failure(format!("The service did not answer: {e}")));
                let _ = tx.send(Msg::Done(cmd, reply));
            });
        }
    });
    cmd_tx
}

async fn call(conn: &Connection, method: &str, arg: Option<&Arg>) -> zbus::Result<String> {
    let msg = match arg {
        None => conn.call_method(Some(BUS), PATH, Some(IFACE), method, &()).await?,
        Some(Arg::Force(f)) => conn.call_method(Some(BUS), PATH, Some(IFACE), method, &(*f,)).await?,
        Some(Arg::State(s)) => conn.call_method(Some(BUS), PATH, Some(IFACE), method, &(*s,)).await?,
    };
    msg.body().deserialize::<String>()
}

async fn request(cmd: Cmd) -> zbus::Result<Reply> {
    let conn = Connection::system().await?;
    let (method, arg) = cmd.call();
    Ok(Reply::parse(&call(&conn, method, Some(&arg)).await?))
}

/// Read the state, then follow StateChanged until the service goes away.
async fn watch(tx: &Sender<Msg>) -> zbus::Result<()> {
    let conn = Connection::system().await?;
    // Listen first, then ask, so no change falls between the two.
    let rule = MatchRule::builder().msg_type(Type::Signal).interface(IFACE).member("StateChanged").build()?;
    let signals = MessageStream::for_match_rule(rule, &conn, None).await?;
    let owner = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface("org.freedesktop.DBus")
        .member("NameOwnerChanged")
        .arg(0, BUS)
        .build()?;
    let owners = MessageStream::for_match_rule(owner, &conn, None).await?;
    // The first call also starts a D-Bus activated service, and fails if there is none.
    send_status(tx, &call(&conn, "GetStatus", None).await?);
    let mut merged = signals.or(owners);
    while let Some(Ok(m)) = merged.next().await {
        let hdr = m.header();
        match hdr.member().map(|m| m.as_str()) {
            Some("StateChanged") => {
                if let Ok(json) = m.body().deserialize::<String>() {
                    send_status(tx, &json);
                }
            }
            Some("NameOwnerChanged") => {
                let (_, _, new): (String, String, String) = m.body().deserialize()?;
                if new.is_empty() {
                    return Err(zbus::Error::Failure("the service stopped".into()));
                }
                // A restarted service: read it afresh.
                send_status(tx, &call(&conn, "GetStatus", None).await?);
            }
            _ => {}
        }
    }
    Err(zbus::Error::Failure("the bus connection closed".into()))
}

fn send_status(tx: &Sender<Msg>, json: &str) {
    // StateChanged carries the status object itself, a reply wraps it.
    let st = serde_json::from_str::<Reply>(json)
        .ok()
        .and_then(|r| r.status)
        .or_else(|| serde_json::from_str::<Status>(json).ok());
    match st {
        Some(s) => {
            let _ = tx.send(Msg::Status(s));
        }
        None => log::warn!("graphics card service: unreadable state {json:.200}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_status_reply() {
        let json = r#"{"success":true,"action":"status","status":{"state":"linux","gpu_driver":"nvidia","audio_driver":"snd_hda_intel","vm_running":false,"vm_state":"shut_off","vm_name":"win11","safe_to_detach":false,"blocked_by":"in use by labwc","processes":[{"pid":1,"name":"systemd","blocking":false,"detail":"/dev/dri/card0"},{"pid":3358,"name":"labwc","blocking":true,"detail":"/dev/nvidia0"}],"preferred_state":"linux","preference_pending":false,"new_field":1}}"#;
        let r = Reply::parse(json);
        let s = r.status.unwrap();
        assert_eq!(s.state, Phase::Host);
        assert_eq!(s.vm_name, "win11");
        assert_eq!(holders_text(&s.processes), "labwc (3358)");
    }

    #[test]
    fn unknown_state_does_not_fail() {
        let s: Status = serde_json::from_str(r#"{"state":"something_new"}"#).unwrap();
        assert_eq!(s.state, Phase::Unknown);
    }

    #[test]
    fn busy_reply_names_holders() {
        let r = Reply::parse(r#"{"success":false,"error":"gpu_busy","message":"GPU is in use by blender","processes":[{"pid":1234,"name":"blender","detail":"/dev/nvidia0"}]}"#);
        assert_eq!(r.error_text(), "In use by blender (1234).");
        let r = Reply::parse(r#"{"success":false,"error":"vfio_bind_failed","message":"bind failed","rolled_back":false}"#);
        assert!(r.error_text().starts_with("bind failed The card was left"));
    }

    #[test]
    fn bad_json_is_a_failure() {
        let r = Reply::parse("not json");
        assert!(!r.success);
        assert!(r.error_text().contains("unreadable"));
    }
}
