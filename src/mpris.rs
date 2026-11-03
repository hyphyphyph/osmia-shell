//! Media players, through MPRIS: what is playing, and the controls to change it.
//! Works for any player that follows the standard, such as Spotify, browsers and mpv.

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use futures_lite::StreamExt;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use zbus::{message::Type, Connection, MatchRule, MessageStream, OwnedVariant as OwnedValue};

use crate::capture::grab::Image;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const ROOT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Debug)]
pub struct Player {
    /// The well-known bus name, such as org.mpris.MediaPlayer2.spotify.
    pub bus: String,
    pub identity: String,
    pub desktop_entry: String,
    pub status: Status,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: String,
    pub length_us: i64,
    pub position_us: i64,
    /// When `position_us` was read, so the bar can keep counting between reads.
    pub pos_at: Instant,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_toggle: bool,
    pub can_seek: bool,
    pub track_id: String,
    /// When this player last changed, so the most recent one can be the default.
    pub touched: Instant,
}

impl Player {
    /// The play position now, counting on from the last read while it plays.
    pub fn position_now(&self) -> i64 {
        let mut p = self.position_us;
        if self.status == Status::Playing {
            p += self.pos_at.elapsed().as_micros() as i64;
        }
        if self.length_us > 0 {
            p = p.min(self.length_us);
        }
        p.max(0)
    }

    /// "Artist – Title", or whatever there is.
    pub fn line(&self) -> String {
        match (self.artist.is_empty(), self.title.is_empty()) {
            (false, false) => format!("{} \u{2013} {}", self.artist, self.title),
            (true, false) => self.title.clone(),
            (false, true) => self.artist.clone(),
            _ => self.identity.clone(),
        }
    }

    /// Does this player belong to an app with these names, such as a tray item's id and title?
    /// Names match when equal or when one starts with the other, so "spotify-client" finds "spotify".
    pub fn is_app(&self, names: &[String]) -> bool {
        let short = self.bus.strip_prefix(PREFIX).unwrap_or(&self.bus);
        // Browsers add ".instance1234" to the name.
        let short = short.split(".instance").next().unwrap_or(short);
        let mine = [short.to_lowercase(), self.identity.to_lowercase(), self.desktop_entry.trim_end_matches(".desktop").to_lowercase()];
        names.iter().any(|n| {
            let n = n.trim_end_matches(".desktop").to_lowercase();
            n.len() >= 4 && mine.iter().any(|m| m.len() >= 4 && (*m == n || m.starts_with(&n) || n.starts_with(m.as_str())))
        })
    }
}

pub enum Msg {
    Upsert(Player),
    Remove(String),
    /// The cover picture for a URL, already fetched.
    Art(String, Image),
}

pub enum Cmd {
    PlayPause(String),
    Pause(String),
    Next(String),
    Previous(String),
    /// Move by this many microseconds.
    Seek(String, i64),
    /// Jump to a position in the current track.
    SetPosition(String, String, i64),
    Raise(String),
    /// Read the positions of playing players again.
    Poll,
}

type Known = Arc<Mutex<HashMap<String, Player>>>;

fn text(v: &OwnedValue) -> Option<String> {
    <&str>::try_from(v).ok().map(str::to_owned)
}

fn number(v: &OwnedValue) -> Option<i64> {
    i64::try_from(v).ok().or_else(|| u64::try_from(v).ok().map(|n| n as i64)).or_else(|| i32::try_from(v).ok().map(i64::from))
}

fn strings(v: &OwnedValue) -> Vec<String> {
    v.try_clone().ok().and_then(|c| Vec::<String>::try_from(c).ok()).unwrap_or_default()
}

async fn read_player(conn: &Connection, bus: &str, old: Option<&Player>) -> Option<Player> {
    let root = zbus::Proxy::new(conn, bus.to_owned(), ROOT, "org.mpris.MediaPlayer2").await.ok()?;
    let p = zbus::Proxy::new(conn, bus.to_owned(), ROOT, PLAYER).await.ok()?;
    let status = match p.get_property::<String>("PlaybackStatus").await.ok()?.as_str() {
        "Playing" => Status::Playing,
        "Paused" => Status::Paused,
        _ => Status::Stopped,
    };
    let meta: HashMap<String, OwnedValue> = p.get_property("Metadata").await.unwrap_or_default();
    let get = |k: &str| meta.get(k);
    let mut artist = get("xesam:artist").map(strings).unwrap_or_default().join(", ");
    if artist.is_empty() {
        artist = get("xesam:albumArtist").map(strings).unwrap_or_default().join(", ");
    }
    let track_id = get("mpris:trackid")
        .and_then(|v| text(v).or_else(|| zbus::OwnedObjectPath::try_from(v.try_clone().ok()?).ok().map(|o| o.to_string())))
        .unwrap_or_default();
    Some(Player {
        bus: bus.to_owned(),
        identity: root.get_property::<String>("Identity").await.unwrap_or_default(),
        desktop_entry: root.get_property::<String>("DesktopEntry").await.unwrap_or_default(),
        status,
        title: get("xesam:title").and_then(text).unwrap_or_default(),
        artist,
        album: get("xesam:album").and_then(text).unwrap_or_default(),
        art_url: get("mpris:artUrl").and_then(text).unwrap_or_default(),
        length_us: get("mpris:length").and_then(number).unwrap_or(0),
        position_us: p.get_property::<i64>("Position").await.unwrap_or(0),
        pos_at: Instant::now(),
        can_next: p.get_property::<bool>("CanGoNext").await.unwrap_or(false),
        can_prev: p.get_property::<bool>("CanGoPrevious").await.unwrap_or(false),
        can_toggle: p.get_property::<bool>("CanPause").await.unwrap_or(true) || p.get_property::<bool>("CanPlay").await.unwrap_or(true),
        can_seek: p.get_property::<bool>("CanSeek").await.unwrap_or(false),
        track_id,
        touched: if old.is_some_and(|o| o.status == status && o.title == get("xesam:title").and_then(text).unwrap_or_default()) {
            old.map(|o| o.touched).unwrap_or_else(Instant::now)
        } else {
            Instant::now()
        },
    })
}

/// Fetch a cover picture and decode it. Remote pictures go through curl.
fn fetch_art(url: &str) -> Option<Image> {
    let bytes = if let Some(path) = url.strip_prefix("file://") {
        std::fs::read(path).ok()?
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let out = std::process::Command::new("curl").args(["-fsSL", "--max-time", "8", url]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        out.stdout
    } else {
        return None;
    };
    let img = decode_image(&bytes)?;
    Some(crate::capture::shrink(&img, 160))
}

fn decode_image(bytes: &[u8]) -> Option<Image> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let data = &buf[..info.buffer_size()];
        let rgba: Vec<u8> = match info.color_type {
            png::ColorType::Rgba => data.to_vec(),
            png::ColorType::Rgb => data.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            png::ColorType::Grayscale => data.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
            png::ColorType::GrayscaleAlpha => data.chunks_exact(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
            _ => return None,
        };
        return Some(Image { w: info.width, h: info.height, rgba });
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
        let px = dec.decode().ok()?;
        let info = dec.info()?;
        let rgba: Vec<u8> = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => px.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            jpeg_decoder::PixelFormat::L8 => px.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
            _ => return None,
        };
        return Some(Image { w: info.width as u32, h: info.height as u32, rgba });
    }
    None
}

/// Start watching for players. Returns a sender for commands.
pub fn start(tx: Sender<Msg>) -> mpsc::Sender<Cmd> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
    let known: Known = Arc::new(Mutex::new(HashMap::new()));
    let conn_cell: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
    {
        let (known, tx, conn_cell) = (known.clone(), tx.clone(), conn_cell.clone());
        std::thread::spawn(move || {
            if let Err(e) = pollster::block_on(run(tx, known, conn_cell)) {
                log::warn!("media players are off: {e}");
            }
        });
    }
    // Positions are not announced when they change, so read them now and then.
    {
        let poke = cmd_tx.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(4));
            if poke.send(Cmd::Poll).is_err() {
                break;
            }
        });
    }
    std::thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            let conn = conn_cell.lock().unwrap().clone();
            let Some(conn) = conn else { continue };
            pollster::block_on(handle_cmd(&conn, &known, &tx, cmd));
        }
    });
    cmd_tx
}

async fn refresh(conn: &Connection, known: &Known, tx: &Sender<Msg>, bus: &str) {
    let old = known.lock().unwrap().get(bus).cloned();
    let Some(p) = read_player(conn, bus, old.as_ref()).await else { return };
    let art_changed = old.as_ref().map(|o| o.art_url != p.art_url).unwrap_or(true);
    known.lock().unwrap().insert(bus.to_owned(), p.clone());
    let url = p.art_url.clone();
    let _ = tx.send(Msg::Upsert(p));
    if art_changed && !url.is_empty() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            if let Some(img) = fetch_art(&url) {
                let _ = tx.send(Msg::Art(url, img));
            }
        });
    }
}

async fn run(tx: Sender<Msg>, known: Known, conn_cell: Arc<Mutex<Option<Connection>>>) -> zbus::Result<()> {
    let conn = Connection::session().await?;
    *conn_cell.lock().unwrap() = Some(conn.clone());
    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
    // Listen first, then list, so nothing is missed.
    let rule_props = MatchRule::builder().msg_type(Type::Signal).interface("org.freedesktop.DBus.Properties").member("PropertiesChanged").build()?;
    let rule_seek = MatchRule::builder().msg_type(Type::Signal).interface(PLAYER).member("Seeked").build()?;
    let rule_owner = MatchRule::builder().msg_type(Type::Signal).interface("org.freedesktop.DBus").member("NameOwnerChanged").build()?;
    let mut merged = MessageStream::for_match_rule(rule_props, &conn, None)
        .await?
        .or(MessageStream::for_match_rule(rule_seek, &conn, None).await?)
        .or(MessageStream::for_match_rule(rule_owner, &conn, None).await?);
    // Map unique names to well-known names so signals can be matched to a player.
    let mut owners: HashMap<String, String> = HashMap::new();
    for name in dbus.list_names().await? {
        let name = name.to_string();
        if name.starts_with(PREFIX) && !name.ends_with("playerctld") {
            if let Ok(o) = dbus.get_name_owner(zbus::names::BusName::try_from(name.as_str())?).await {
                owners.insert(o.to_string(), name.clone());
            }
            refresh(&conn, &known, &tx, &name).await;
        }
    }
    while let Some(Ok(msg)) = merged.next().await {
        let hdr = msg.header();
        let member = hdr.member().map(|m| m.as_str().to_owned()).unwrap_or_default();
        match member.as_str() {
            "NameOwnerChanged" => {
                let body = msg.body();
                if let Ok((name, _old, new)) = body.deserialize::<(&str, &str, &str)>() {
                    if name.starts_with(PREFIX) && !name.ends_with("playerctld") {
                        let name = name.to_owned();
                        if new.is_empty() {
                            owners.retain(|_, v| *v != name);
                            known.lock().unwrap().remove(&name);
                            let _ = tx.send(Msg::Remove(name));
                        } else {
                            owners.insert(new.to_owned(), name.clone());
                            refresh(&conn, &known, &tx, &name).await;
                        }
                    }
                }
            }
            "PropertiesChanged" | "Seeked" => {
                let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
                if let Some(name) = owners.get(&sender).cloned() {
                    refresh(&conn, &known, &tx, &name).await;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

async fn handle_cmd(conn: &Connection, known: &Known, tx: &Sender<Msg>, cmd: Cmd) {
    let call = |bus: String, iface: &'static str, method: &'static str| {
        let conn = conn.clone();
        async move { conn.call_method(Some(bus.as_str()), ROOT, Some(iface), method, &()).await }
    };
    match cmd {
        Cmd::PlayPause(b) => {
            // Browsers can ignore PlayPause. Ask for the state we want instead.
            let now = match zbus::Proxy::new(conn, b.clone(), ROOT, PLAYER).await {
                Ok(p) => p.get_property::<String>("PlaybackStatus").await.ok(),
                Err(_) => None,
            };
            let method = match now.as_deref() {
                Some("Playing") => "Pause",
                Some(_) => "Play",
                None => "PlayPause",
            };
            if let Err(e) = call(b.clone(), PLAYER, method).await {
                log::warn!("{b}: {method} failed: {e}");
            }
        }
        Cmd::Pause(b) => {
            let _ = call(b, PLAYER, "Pause").await;
        }
        Cmd::Next(b) => {
            let _ = call(b, PLAYER, "Next").await;
        }
        Cmd::Previous(b) => {
            let _ = call(b, PLAYER, "Previous").await;
        }
        Cmd::Raise(b) => {
            let _ = call(b, "org.mpris.MediaPlayer2", "Raise").await;
        }
        Cmd::Seek(b, off) => {
            let _ = conn.call_method(Some(b.as_str()), ROOT, Some(PLAYER), "Seek", &(off,)).await;
        }
        Cmd::SetPosition(b, track, pos) => {
            if let Ok(path) = zbus::ObjectPath::try_from(track.as_str()) {
                let _ = conn.call_method(Some(b.as_str()), ROOT, Some(PLAYER), "SetPosition", &(path, pos)).await;
            }
        }
        Cmd::Poll => {
            let playing: Vec<String> = known.lock().unwrap().values().filter(|p| p.status == Status::Playing).map(|p| p.bus.clone()).collect();
            for bus in playing {
                refresh(conn, known, tx, &bus).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(bus: &str, identity: &str, desktop: &str) -> Player {
        Player {
            bus: bus.into(),
            identity: identity.into(),
            desktop_entry: desktop.into(),
            status: Status::Paused,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_url: String::new(),
            length_us: 0,
            position_us: 0,
            pos_at: Instant::now(),
            can_next: true,
            can_prev: true,
            can_toggle: true,
            can_seek: true,
            track_id: String::new(),
            touched: Instant::now(),
        }
    }

    #[test]
    fn tray_names_find_their_player() {
        let spotify = player("org.mpris.MediaPlayer2.spotify", "Spotify", "spotify");
        assert!(spotify.is_app(&["spotify-client".into(), "spotify".into()]));
        assert!(!spotify.is_app(&["telegram".into()]));
        let chrome = player("org.mpris.MediaPlayer2.chromium.instance123", "Helium", "helium");
        assert!(chrome.is_app(&["helium".into()]));
        assert!(!chrome.is_app(&["abc".into()]));
    }

    #[test]
    fn position_counts_on_only_while_playing() {
        let mut p = player("org.mpris.MediaPlayer2.x", "X", "");
        p.position_us = 5_000_000;
        p.length_us = 6_000_000;
        p.pos_at = Instant::now() - Duration::from_secs(3);
        assert_eq!(p.position_now(), 5_000_000);
        p.status = Status::Playing;
        assert_eq!(p.position_now(), 6_000_000, "stops at the end of the track");
    }

    #[test]
    fn reads_the_running_players() {
        // Needs a session bus. With no players running it just finds nothing.
        let conn = pollster::block_on(Connection::session());
        let Ok(conn) = conn else { return };
        let dbus = pollster::block_on(zbus::fdo::DBusProxy::new(&conn)).unwrap();
        let names = pollster::block_on(dbus.list_names()).unwrap();
        for n in names.iter().map(|n| n.to_string()).filter(|n| n.starts_with(PREFIX) && !n.ends_with("playerctld")) {
            let p = pollster::block_on(read_player(&conn, &n, None));
            eprintln!("{n}: {:?}", p.map(|p| (p.identity.clone(), p.status, p.line(), p.length_us, p.art_url.clone())));
        }
    }
}

/// Everything the shell knows about media players.
#[derive(Default)]
pub struct State {
    /// Most recently changed first.
    pub players: Vec<Player>,
    pub art: HashMap<String, Arc<Image>>,
}

impl State {
    pub fn upsert(&mut self, p: Player) {
        // A browser with no media open reports Stopped, no track and no Play.
        // There is nothing to control, so leave it out until it has something.
        if p.status == Status::Stopped && p.title.is_empty() && !p.can_toggle {
            self.remove(&p.bus);
            return;
        }
        match self.players.iter_mut().find(|x| x.bus == p.bus) {
            Some(slot) => *slot = p,
            None => self.players.push(p),
        }
        self.players.sort_by_key(|p| std::cmp::Reverse((p.status == Status::Playing, p.touched)));
    }

    pub fn remove(&mut self, bus: &str) {
        self.players.retain(|p| p.bus != bus);
    }

    /// The player that keys and the bar item act on: one that is playing, else the latest to change.
    pub fn current(&self) -> Option<&Player> {
        self.players.iter().find(|p| p.status == Status::Playing).or_else(|| self.players.iter().find(|p| p.status != Status::Stopped))
    }

    pub fn for_app(&self, names: &[String]) -> Option<&Player> {
        self.players.iter().find(|p| p.is_app(names))
    }
}
