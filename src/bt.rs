//! Bluetooth, through BlueZ on the system bus.
//!
//! The shell reads adapters and devices from BlueZ's object manager, follows
//! its signals, and asks it to power, scan, pair, connect and forget. It is also
//! the pairing agent: BlueZ asks it to confirm a code or type a PIN, and the
//! shell asks the user in the Bluetooth card. See spec/Bluetooth.pi.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_lite::StreamExt;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use zbus::{ObjectPath, OwnedObjectPath, OwnedVariant as OwnedValue, Variant as Value};
use zbus::{message::Type, Connection, MatchRule, MessageStream};

const BUS: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";
const AGENT_PATH: &str = "/org/osmia/BluetoothAgent";
/// How long to wait before looking again after BlueZ could not be reached.
const RETRY: Duration = Duration::from_secs(30);

// ---------- what the UI draws ----------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Adapter {
    pub path: String,
    pub powered: bool,
    /// Turned off by a radio switch (rfkill), not only by BlueZ.
    pub blocked: bool,
    pub discovering: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Device {
    pub path: String,
    pub address: String,
    /// The name the device gives itself, if it gave one.
    pub name: Option<String>,
    /// The name to show: the user's alias, else the name, else the address.
    pub alias: String,
    /// BlueZ's icon name, such as audio-headset or input-mouse.
    pub icon: String,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    pub battery: Option<u8>,
    pub rssi: Option<i16>,
}

impl Device {
    /// A device that has only an address, such as an unnamed beacon, is not worth listing.
    pub fn named(&self) -> bool {
        self.paired || self.name.is_some()
    }

    /// A word for the kind of device, from its icon.
    pub fn kind(&self) -> &'static str {
        match self.icon.as_str() {
            "audio-headset" | "audio-headphones" => "Headphones",
            "audio-card" | "audio-speakers" => "Speaker",
            "input-mouse" => "Mouse",
            "input-keyboard" => "Keyboard",
            "input-gaming" => "Controller",
            "input-tablet" => "Tablet",
            "phone" => "Phone",
            "computer" => "Computer",
            "camera-video" | "camera-photo" => "Camera",
            "printer" => "Printer",
            _ => "",
        }
    }
}

/// What BlueZ asks while pairing.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Does the device show this number? Yes or no.
    Confirm(u32),
    /// May this device pair? It has no screen or keys, so there is no code.
    Authorize,
    /// May this paired device use this service?
    Service(String),
    /// Type the PIN the device shows or uses (older devices, often 0000).
    Pin,
    /// Type the six-digit number the device shows.
    Passkey,
    /// Type this PIN on the device. Nothing to answer here.
    ShowPin(String),
    /// Type this number on the device, then Enter. The second value counts the keys typed so far.
    ShowPasskey(u32, u16),
}

impl Ask {
    /// Only shown; BlueZ does not wait for an answer.
    pub fn shown_only(&self) -> bool {
        matches!(self, Ask::ShowPin(_) | Ask::ShowPasskey(..))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: u64,
    pub device: String,
    pub name: String,
    pub ask: Ask,
}

#[derive(Debug, Clone)]
pub enum Answer {
    Yes,
    No,
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    Power(bool),
    Scan(bool),
    Connect(String),
    Disconnect(String),
    /// Pair, trust, then connect.
    Pair(String),
    Forget(String),
    /// Stop a pairing that shows a code to type on the device.
    CancelPair(String),
}

impl Cmd {
    /// The device the command is about, if any.
    pub fn device(&self) -> Option<&str> {
        match self {
            Cmd::Connect(p) | Cmd::Disconnect(p) | Cmd::Pair(p) | Cmd::Forget(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct State {
    /// None while BlueZ is not running or there is no adapter.
    pub adapter: Option<Adapter>,
    pub devices: Vec<Device>,
    /// Device commands waiting for an answer, by device path.
    pub pending: Vec<Cmd>,
    /// What the pairing agent is asking the user, newest last.
    pub asks: Vec<Request>,
    /// The last failure, and why. Cleared by the next command.
    pub error: Option<String>,
    /// The shell started the scan (when the card opened), so it stops it too.
    pub scan_ours: bool,
}

impl State {
    pub fn connected(&self) -> Vec<&Device> {
        self.devices.iter().filter(|d| d.connected).collect()
    }

    pub fn device(&self, path: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.path == path)
    }

    pub fn busy(&self, path: &str) -> Option<&Cmd> {
        self.pending.iter().find(|c| c.device() == Some(path))
    }

    /// The words for `osmia bluetooth status`.
    pub fn word(&self) -> String {
        match &self.adapter {
            None => "unavailable".into(),
            Some(a) if !a.powered => "off".into(),
            Some(_) => {
                let c = self.connected();
                if c.is_empty() { "on".into() } else { c.iter().map(|d| d.alias.as_str()).collect::<Vec<_>>().join("\n") }
            }
        }
    }
}

pub enum Msg {
    /// The adapter and devices, read afresh.
    State(Option<Adapter>, Vec<Device>),
    /// BlueZ asks the user something.
    Ask(Request),
    /// The request with this id is over: answered, cancelled, or timed out. None ends all.
    AskEnd(Option<u64>),
    /// A command finished. Err holds the reason in words.
    Done(Cmd, Result<(), String>),
}

pub struct Handle {
    tx: std::sync::mpsc::Sender<Cmd>,
    agent: Arc<AgentInner>,
}

impl Handle {
    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn answer(&self, id: u64, a: Answer) {
        if let Some(w) = self.agent.waiting.lock().unwrap().remove(&id) {
            let _ = w.send(a);
        }
        let _ = self.agent.tx.send(Msg::AskEnd(Some(id)));
    }
}

// ---------- reading BlueZ ----------

type Props = HashMap<String, OwnedValue>;
type Ifaces = HashMap<String, Props>;
type Objects = HashMap<OwnedObjectPath, Ifaces>;

/// Every BlueZ object and its properties, kept up to date from signals.
#[derive(Default)]
struct Model {
    objects: BTreeMap<String, Ifaces>,
}

fn get<'a, T>(p: &'a Props, key: &str) -> Option<T>
where
    T: TryFrom<&'a Value<'a>>,
    <T as TryFrom<&'a Value<'a>>>::Error: Into<zbus::Error>,
{
    p.get(key).and_then(|v| v.downcast_ref::<T>().ok())
}

impl Model {
    fn snapshot(&self) -> (Option<Adapter>, Vec<Device>) {
        // The first adapter. More than one is out of scope.
        let adapter = self.objects.iter().find_map(|(path, i)| {
            let p = i.get(ADAPTER)?;
            let state: Option<String> = get(p, "PowerState");
            Some(Adapter {
                path: path.clone(),
                powered: get(p, "Powered").unwrap_or(false),
                blocked: state.as_deref() == Some("off-blocked"),
                discovering: get(p, "Discovering").unwrap_or(false),
            })
        });
        let mut devices: Vec<Device> = self
            .objects
            .iter()
            .filter(|(path, _)| adapter.as_ref().is_some_and(|a| path.starts_with(&format!("{}/", a.path))))
            .filter_map(|(path, i)| {
                let p = i.get(DEVICE)?;
                let address: String = get(p, "Address").unwrap_or_default();
                let name: Option<String> = get::<String>(p, "Name").filter(|n| !n.trim().is_empty());
                let alias = get::<String>(p, "Alias").filter(|a| !a.is_empty()).unwrap_or_else(|| address.clone());
                Some(Device {
                    path: path.clone(),
                    alias,
                    name,
                    address,
                    icon: get(p, "Icon").unwrap_or_default(),
                    paired: get::<bool>(p, "Paired").unwrap_or(false) || get::<bool>(p, "Bonded").unwrap_or(false),
                    trusted: get(p, "Trusted").unwrap_or(false),
                    connected: get(p, "Connected").unwrap_or(false),
                    battery: i.get(BATTERY).and_then(|b| get(b, "Percentage")),
                    rssi: get(p, "RSSI"),
                })
            })
            .collect();
        sort_devices(&mut devices);
        (adapter, devices)
    }

    fn alias(&self, path: &str) -> String {
        self.objects
            .get(path)
            .and_then(|i| i.get(DEVICE))
            .and_then(|p| get::<String>(p, "Alias").or_else(|| get(p, "Address")))
            .unwrap_or_else(|| "A device".into())
    }
}

/// Connected first, then paired, then nearby by signal, each by name.
pub fn sort_devices(list: &mut [Device]) {
    list.sort_by(|a, b| {
        (!a.connected, !a.paired, -(a.rssi.unwrap_or(-127) as i32) / 10, a.alias.to_lowercase())
            .cmp(&(!b.connected, !b.paired, -(b.rssi.unwrap_or(-127) as i32) / 10, b.alias.to_lowercase()))
    });
}

// ---------- the pairing agent ----------

struct AgentInner {
    tx: Sender<Msg>,
    model: Arc<Mutex<Model>>,
    waiting: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Answer>>>,
    next: AtomicU64,
}

impl AgentInner {
    fn request(&self, device: &ObjectPath<'_>, ask: Ask) -> Request {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let name = self.model.lock().unwrap().alias(device.as_str());
        Request { id, device: device.to_string(), name, ask }
    }

    /// Ask the user and wait for the answer. BlueZ calls Cancel if it gives up first.
    async fn ask(&self, device: &ObjectPath<'_>, ask: Ask) -> Option<Answer> {
        let req = self.request(device, ask);
        let id = req.id;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.waiting.lock().unwrap().insert(id, tx);
        let _ = self.tx.send(Msg::Ask(req));
        let a = rx.await.ok();
        self.waiting.lock().unwrap().remove(&id);
        a
    }

    fn show(&self, device: &ObjectPath<'_>, ask: Ask) {
        let _ = self.tx.send(Msg::Ask(self.request(device, ask)));
    }
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected(String),
    Canceled(String),
}

fn refused(a: Option<Answer>) -> AgentError {
    match a {
        None => AgentError::Canceled("cancelled".into()),
        _ => AgentError::Rejected("refused by the user".into()),
    }
}

struct Agent(Arc<AgentInner>);

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    fn release(&self) {
        let _ = self.0.tx.send(Msg::AskEnd(None));
    }

    async fn request_pin_code(&self, device: OwnedObjectPath) -> Result<String, AgentError> {
        match self.0.ask(&device, Ask::Pin).await {
            Some(Answer::Text(t)) if !t.is_empty() && t.len() <= 16 => Ok(t),
            a => Err(refused(a)),
        }
    }

    fn display_pin_code(&self, device: OwnedObjectPath, pincode: String) {
        self.0.show(&device, Ask::ShowPin(pincode));
    }

    async fn request_passkey(&self, device: OwnedObjectPath) -> Result<u32, AgentError> {
        match self.0.ask(&device, Ask::Passkey).await {
            Some(Answer::Text(t)) => t.trim().parse::<u32>().ok().filter(|n| *n <= 999_999).ok_or_else(|| refused(Some(Answer::No))),
            a => Err(refused(a)),
        }
    }

    fn display_passkey(&self, device: OwnedObjectPath, passkey: u32, entered: u16) {
        self.0.show(&device, Ask::ShowPasskey(passkey, entered));
    }

    async fn request_confirmation(&self, device: OwnedObjectPath, passkey: u32) -> Result<(), AgentError> {
        match self.0.ask(&device, Ask::Confirm(passkey)).await {
            Some(Answer::Yes) => Ok(()),
            a => Err(refused(a)),
        }
    }

    async fn request_authorization(&self, device: OwnedObjectPath) -> Result<(), AgentError> {
        match self.0.ask(&device, Ask::Authorize).await {
            Some(Answer::Yes) => Ok(()),
            a => Err(refused(a)),
        }
    }

    async fn authorize_service(&self, device: OwnedObjectPath, uuid: String) -> Result<(), AgentError> {
        match self.0.ask(&device, Ask::Service(uuid)).await {
            Some(Answer::Yes) => Ok(()),
            a => Err(refused(a)),
        }
    }

    fn cancel(&self) {
        // Dropping the senders ends every wait with Canceled.
        self.0.waiting.lock().unwrap().clear();
        let _ = self.0.tx.send(Msg::AskEnd(None));
    }
}

// ---------- the worker ----------

/// Start watching BlueZ and serving the agent. Returns a handle for commands and answers.
pub fn start(tx: Sender<Msg>) -> Handle {
    let model: Arc<Mutex<Model>> = Arc::default();
    let agent = Arc::new(AgentInner { tx: tx.clone(), model: model.clone(), waiting: Mutex::default(), next: AtomicU64::new(1) });
    let conn_cell: Arc<Mutex<Option<Connection>>> = Arc::default();
    {
        let (tx, agent, conn_cell) = (tx.clone(), agent.clone(), conn_cell.clone());
        std::thread::spawn(move || loop {
            if let Err(e) = pollster::block_on(watch(&tx, &agent, &model, &conn_cell)) {
                log::debug!("bluetooth: {e}");
            }
            *conn_cell.lock().unwrap() = None;
            model.lock().unwrap().objects.clear();
            agent.waiting.lock().unwrap().clear();
            let _ = tx.send(Msg::AskEnd(None));
            if tx.send(Msg::State(None, Vec::new())).is_err() {
                return;
            }
            std::thread::sleep(RETRY);
        });
    }
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<Cmd>();
    std::thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            let Some(conn) = conn_cell.lock().unwrap().clone() else {
                let _ = tx.send(Msg::Done(cmd, Err("Bluetooth is not running.".into())));
                continue;
            };
            // Power and scan are quick, and run in order so a fast open and close cannot swap.
            // The rest get their own thread: pairing can wait for the user, connecting for the device.
            if matches!(cmd, Cmd::Power(_) | Cmd::Scan(_)) {
                let r = pollster::block_on(run(&conn, &cmd)).map_err(|e| error_text(&e));
                let _ = tx.send(Msg::Done(cmd, r));
                continue;
            }
            let tx = tx.clone();
            std::thread::spawn(move || {
                let r = pollster::block_on(run(&conn, &cmd)).map_err(|e| error_text(&e));
                let _ = tx.send(Msg::Done(cmd, r));
            });
        }
    });
    Handle { tx: cmd_tx, agent }
}

async fn watch(tx: &Sender<Msg>, agent: &Arc<AgentInner>, model: &Mutex<Model>, conn_cell: &Mutex<Option<Connection>>) -> zbus::Result<()> {
    let conn = Connection::system().await?;
    // Listen first, then read, so no change falls between the two.
    let props = MatchRule::builder().msg_type(Type::Signal).sender(BUS).interface("org.freedesktop.DBus.Properties").member("PropertiesChanged").build()?;
    let objs = MatchRule::builder().msg_type(Type::Signal).sender(BUS).interface("org.freedesktop.DBus.ObjectManager").build()?;
    let owner = MatchRule::builder().msg_type(Type::Signal).interface("org.freedesktop.DBus").member("NameOwnerChanged").arg(0, BUS).build()?;
    let s1 = MessageStream::for_match_rule(props, &conn, None).await?;
    let s2 = MessageStream::for_match_rule(objs, &conn, None).await?;
    let s3 = MessageStream::for_match_rule(owner, &conn, None).await?;
    let mut merged = s1.or(s2).or(s3);

    let reply = conn.call_method(Some(BUS), "/", Some("org.freedesktop.DBus.ObjectManager"), "GetManagedObjects", &()).await?;
    let objects: Objects = reply.body().deserialize()?;
    model.lock().unwrap().objects = objects.into_iter().map(|(p, i)| (p.to_string(), i)).collect();
    send_state(tx, model);

    conn.object_server().at(AGENT_PATH, Agent(agent.clone())).await?;
    register_agent(&conn).await;
    *conn_cell.lock().unwrap() = Some(conn.clone());

    while let Some(Ok(m)) = merged.next().await {
        let hdr = m.header();
        let path = hdr.path().map(|p| p.to_string()).unwrap_or_default();
        match hdr.member().map(|m| m.as_str()) {
            Some("PropertiesChanged") => {
                let (iface, changed, gone): (String, Props, Vec<String>) = m.body().deserialize()?;
                let mut md = model.lock().unwrap();
                let Some(obj) = md.objects.get_mut(&path) else { continue };
                let p = obj.entry(iface).or_default();
                p.extend(changed);
                for k in gone {
                    p.remove(&k);
                }
            }
            Some("InterfacesAdded") => {
                let (obj, ifaces): (OwnedObjectPath, Ifaces) = m.body().deserialize()?;
                model.lock().unwrap().objects.entry(obj.to_string()).or_default().extend(ifaces);
            }
            Some("InterfacesRemoved") => {
                let (obj, ifaces): (OwnedObjectPath, Vec<String>) = m.body().deserialize()?;
                let mut md = model.lock().unwrap();
                let key = obj.to_string();
                if let Some(o) = md.objects.get_mut(&key) {
                    for i in ifaces {
                        o.remove(&i);
                    }
                    if o.is_empty() {
                        md.objects.remove(&key);
                    }
                }
            }
            Some("NameOwnerChanged") => {
                let (_, _, new): (String, String, String) = m.body().deserialize()?;
                if new.is_empty() {
                    return Err(zbus::Error::Failure("bluetoothd stopped".into()));
                }
                // A restarted bluetoothd: read it again and register the agent again.
                let reply = conn.call_method(Some(BUS), "/", Some("org.freedesktop.DBus.ObjectManager"), "GetManagedObjects", &()).await?;
                let objects: Objects = reply.body().deserialize()?;
                model.lock().unwrap().objects = objects.into_iter().map(|(p, i)| (p.to_string(), i)).collect();
                register_agent(&conn).await;
            }
            _ => continue,
        }
        send_state(tx, model);
    }
    Err(zbus::Error::Failure("the bus connection closed".into()))
}

async fn register_agent(conn: &Connection) {
    let path = ObjectPath::from_static_str_unchecked(AGENT_PATH);
    // KeyboardDisplay: the shell can show a code and take one typed in.
    if let Err(e) = conn.call_method(Some(BUS), "/org/bluez", Some("org.bluez.AgentManager1"), "RegisterAgent", &(&path, "KeyboardDisplay")).await {
        log::warn!("bluetooth: could not register the pairing agent: {e}");
        return;
    }
    // Take over from blueman or bluetoothctl, so the prompts come here.
    if let Err(e) = conn.call_method(Some(BUS), "/org/bluez", Some("org.bluez.AgentManager1"), "RequestDefaultAgent", &(&path,)).await {
        log::warn!("bluetooth: could not become the default agent: {e}");
    }
}

fn send_state(tx: &Sender<Msg>, model: &Mutex<Model>) {
    let (a, d) = model.lock().unwrap().snapshot();
    let _ = tx.send(Msg::State(a, d));
}

async fn set_prop(conn: &Connection, path: &str, iface: &str, name: &str, v: Value<'_>) -> zbus::Result<()> {
    conn.call_method(Some(BUS), path, Some("org.freedesktop.DBus.Properties"), "Set", &(iface, name, v)).await?;
    Ok(())
}

async fn call(conn: &Connection, path: &str, iface: &str, method: &str) -> zbus::Result<()> {
    conn.call_method(Some(BUS), path, Some(iface), method, &()).await?;
    Ok(())
}

async fn adapter_path(conn: &Connection) -> zbus::Result<String> {
    let reply = conn.call_method(Some(BUS), "/", Some("org.freedesktop.DBus.ObjectManager"), "GetManagedObjects", &()).await?;
    let objects: Objects = reply.body().deserialize()?;
    let mut paths: Vec<String> = objects.into_iter().filter(|(_, i)| i.contains_key(ADAPTER)).map(|(p, _)| p.to_string()).collect();
    paths.sort();
    paths.into_iter().next().ok_or_else(|| zbus::Error::Failure("There is no Bluetooth adapter.".into()))
}

async fn run(conn: &Connection, cmd: &Cmd) -> zbus::Result<()> {
    match cmd {
        Cmd::Power(on) => {
            let a = adapter_path(conn).await?;
            match set_prop(conn, &a, ADAPTER, "Powered", Value::from(*on)).await {
                // A radio switch holds it off: lift that first. /dev/rfkill is the user's to write.
                Err(e) if *on && e.to_string().contains("Blocked") => {
                    let ok = std::process::Command::new("rfkill").args(["unblock", "bluetooth"]).status().map(|s| s.success()).unwrap_or(false);
                    if !ok {
                        return Err(zbus::Error::Failure("The radio is switched off (rfkill), and rfkill unblock did not work.".into()));
                    }
                    std::thread::sleep(Duration::from_millis(500));
                    set_prop(conn, &a, ADAPTER, "Powered", Value::from(true)).await
                }
                r => r,
            }
        }
        Cmd::Scan(on) => {
            let a = adapter_path(conn).await?;
            if *on {
                // Classic and LE alike, and the same device each time with a new signal reading.
                let mut filter: HashMap<&str, Value> = HashMap::new();
                filter.insert("Transport", Value::from("auto"));
                filter.insert("DuplicateData", Value::from(false));
                let _ = conn.call_method(Some(BUS), a.as_str(), Some(ADAPTER), "SetDiscoveryFilter", &(filter,)).await;
                match call(conn, &a, ADAPTER, "StartDiscovery").await {
                    Err(e) if e.to_string().contains("InProgress") => Ok(()),
                    r => r,
                }
            } else {
                match call(conn, &a, ADAPTER, "StopDiscovery").await {
                    // Not ours, or already stopped.
                    Err(e) if e.to_string().contains("NotAuthorized") || e.to_string().contains("Failed") => Ok(()),
                    r => r,
                }
            }
        }
        Cmd::Connect(p) => call(conn, p, DEVICE, "Connect").await,
        Cmd::Disconnect(p) => call(conn, p, DEVICE, "Disconnect").await,
        Cmd::Pair(p) => {
            match call(conn, p, DEVICE, "Pair").await {
                Err(e) if e.to_string().contains("AlreadyExists") => {}
                r => r?,
            }
            // Trusted devices reconnect on their own and need no prompt for services.
            set_prop(conn, p, DEVICE, "Trusted", Value::from(true)).await?;
            match call(conn, p, DEVICE, "Connect").await {
                // Some devices (a phone, a keyboard still pairing) connect by themselves later.
                Err(e) if e.to_string().contains("AlreadyConnected") || e.to_string().contains("InProgress") => Ok(()),
                r => r,
            }
        }
        Cmd::CancelPair(p) => call(conn, p, DEVICE, "CancelPairing").await,
        Cmd::Forget(p) => {
            let a = p.rsplit_once('/').map(|(a, _)| a.to_owned()).unwrap_or_default();
            let dev = ObjectPath::try_from(p.as_str()).map_err(|e| zbus::Error::Failure(e.to_string()))?;
            conn.call_method(Some(BUS), a.as_str(), Some(ADAPTER), "RemoveDevice", &(&dev,)).await?;
            Ok(())
        }
    }
}

/// BlueZ's error, in words.
fn error_text(e: &zbus::Error) -> String {
    let (name, msg) = match e {
        zbus::Error::MethodError(name, msg, _) => (name.as_str().to_owned(), msg.clone().unwrap_or_default()),
        other => return other.to_string(),
    };
    let short = name.rsplit('.').next().unwrap_or(&name);
    match short {
        "AuthenticationFailed" => "The codes did not match, or the device refused.".into(),
        "AuthenticationCanceled" | "Canceled" => "Pairing was cancelled.".into(),
        "AuthenticationRejected" | "Rejected" => "Pairing was refused.".into(),
        "AuthenticationTimeout" | "ConnectionAttemptFailed" => "The device did not answer. Make sure it is on and in pairing mode.".into(),
        "NotReady" => "Bluetooth is off.".into(),
        "InProgress" => "Already in progress.".into(),
        "DoesNotExist" => "The device is gone.".into(),
        "NotAvailable" | "NotSupported" => "The device does not offer anything this computer can use.".into(),
        "NoReply" => "The device did not answer in time.".into(),
        _ if msg.contains("br-connection-profile-unavailable") => "The device does not offer anything this computer can use.".into(),
        _ if msg.contains("page-timeout") || msg.contains("le-connection-abort") => "The device did not answer. Make sure it is on and nearby.".into(),
        _ if !msg.is_empty() => msg,
        _ => short.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(alias: &str, connected: bool, paired: bool, rssi: Option<i16>) -> Device {
        Device { alias: alias.into(), connected, paired, rssi, path: alias.into(), ..Default::default() }
    }

    #[test]
    fn connected_then_paired_then_nearby_by_signal() {
        let mut l = vec![dev("far", false, false, Some(-90)), dev("mouse", false, true, None), dev("buds", true, true, None), dev("near", false, false, Some(-40))];
        sort_devices(&mut l);
        let names: Vec<_> = l.iter().map(|d| d.alias.as_str()).collect();
        assert_eq!(names, ["buds", "mouse", "near", "far"]);
    }

    #[test]
    fn reads_a_device_with_its_battery() {
        let mut m = Model::default();
        let mut ad = Props::new();
        ad.insert("Powered".into(), OwnedValue::from(true));
        ad.insert("PowerState".into(), OwnedValue::try_from(Value::from("on")).unwrap());
        m.objects.insert("/org/bluez/hci0".into(), HashMap::from([(ADAPTER.to_owned(), ad)]));
        let mut d = Props::new();
        d.insert("Address".into(), OwnedValue::try_from(Value::from("00:11:22:33:44:55")).unwrap());
        d.insert("Alias".into(), OwnedValue::try_from(Value::from("WF-1000XM5")).unwrap());
        d.insert("Name".into(), OwnedValue::try_from(Value::from("WF-1000XM5")).unwrap());
        d.insert("Icon".into(), OwnedValue::try_from(Value::from("audio-headset")).unwrap());
        d.insert("Paired".into(), OwnedValue::from(true));
        d.insert("Connected".into(), OwnedValue::from(true));
        let mut b = Props::new();
        b.insert("Percentage".into(), OwnedValue::from(80u8));
        m.objects.insert("/org/bluez/hci0/dev_00_11_22_33_44_55".into(), HashMap::from([(DEVICE.to_owned(), d), (BATTERY.to_owned(), b)]));
        // An unnamed beacon is read but not listed.
        let mut u = Props::new();
        u.insert("Address".into(), OwnedValue::try_from(Value::from("AA:BB:CC:DD:EE:FF")).unwrap());
        m.objects.insert("/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF".into(), HashMap::from([(DEVICE.to_owned(), u)]));

        let (a, devs) = m.snapshot();
        let a = a.unwrap();
        assert!(a.powered && !a.blocked);
        assert_eq!(devs.len(), 2);
        let d = &devs[0];
        assert_eq!((d.alias.as_str(), d.battery, d.kind(), d.connected), ("WF-1000XM5", Some(80), "Headphones", true));
        assert!(!devs[1].named());
        assert_eq!(devs[1].alias, "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn errors_read_as_words() {
        let e = zbus::Error::Failure("x".into());
        assert_eq!(error_text(&e), e.to_string());
    }
}
