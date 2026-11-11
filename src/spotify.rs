//! Liking Spotify songs. The desktop app has no Like over MPRIS, so this uses
//! Spotify's Web API with the user's own app (a Client ID from developer.spotify.com)
//! and the Authorization Code with PKCE flow: no client secret.
//!
//! HTTPS goes through curl, as the shell already does for wpctl and nmcli. Tokens are
//! passed on curl's stdin, never on its command line, so other users cannot see them.
//! The tokens are kept in ~/.local/state/osmia/spotify.json, readable only by the user.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use smithay_client_toolkit::reexports::calloop::channel::Sender;

/// The redirect URI the user registers in their Spotify app. Spotify allows plain
/// http only for a loopback IP literal, and never for "localhost".
pub const REDIRECT_PORT: u16 = 8898;
pub fn redirect_uri() -> String {
    format!("http://127.0.0.1:{REDIRECT_PORT}/callback")
}
const SCOPES: &str = "user-library-read user-library-modify";
const API: &str = "https://api.spotify.com/v1";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
/// How long the browser sign-in may take.
const LOGIN_WAIT: Duration = Duration::from_secs(300);

pub enum Cmd {
    /// Sign in through the browser with this Client ID.
    Login(String),
    Logout,
    /// Ask whether these tracks are liked.
    Check(String, Vec<String>),
    /// Like or unlike a track.
    Set(String, String, bool),
}

pub enum Msg {
    SignedIn(bool),
    Liked(String, bool),
    /// A check or change failed. The track and what went wrong.
    Failed(Option<String>, String),
}

/// What the shell knows, for the media card.
#[derive(Default)]
pub struct Likes {
    pub signed_in: bool,
    /// Track URI to liked.
    pub liked: HashMap<String, bool>,
    /// Tracks asked about and not yet answered.
    pub pending: HashSet<String>,
    /// Tracks whose check failed. They are not asked again on their own, so a
    /// failing check does not repeat every frame. A click still works.
    pub failed: HashSet<String>,
    /// A sign-in is waiting for the browser.
    pub signing_in: bool,
    /// A like asked for before signing in, done once signed in.
    pub after_login: Option<(String, bool)>,
}

impl Likes {
    pub fn get(&self, uri: &str) -> Option<bool> {
        self.liked.get(uri).copied()
    }
}

/// The Spotify track URI of an MPRIS track id, such as /com/spotify/track/4QZs0LVBrwxrSEE3UCz1Zz.
/// Ads and local files have other ids and get None.
pub fn track_uri(bus: &str, track_id: &str) -> Option<String> {
    if !bus.to_lowercase().contains("spotify") {
        return None;
    }
    let id = track_id.strip_prefix("/com/spotify/track/").or_else(|| track_id.strip_prefix("spotify:track:"))?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| format!("spotify:track:{id}"))
}

pub fn has_tokens() -> bool {
    load().is_some()
}

pub fn start(tx: Sender<Msg>) -> mpsc::Sender<Cmd> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
    std::thread::spawn(move || {
        let mut client_id;
        let _ = tx.send(Msg::SignedIn(has_tokens()));
        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                Cmd::Login(id) => {
                    client_id = id;
                    match login(&client_id) {
                        Ok(()) => {
                            let _ = tx.send(Msg::SignedIn(true));
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::Failed(None, e));
                            let _ = tx.send(Msg::SignedIn(has_tokens()));
                        }
                    }
                }
                Cmd::Logout => {
                    let _ = std::fs::remove_file(state_path());
                    let _ = tx.send(Msg::SignedIn(false));
                }
                Cmd::Check(id, uris) => {
                    client_id = id;
                    match contains(&client_id, &uris) {
                        Ok(v) => {
                            for (u, liked) in uris.into_iter().zip(v) {
                                let _ = tx.send(Msg::Liked(u, liked));
                            }
                        }
                        Err(e) => {
                            for u in uris {
                                let _ = tx.send(Msg::Failed(Some(u), e.clone()));
                            }
                        }
                    }
                }
                Cmd::Set(id, uri, like) => {
                    client_id = id;
                    let method = if like { "PUT" } else { "DELETE" };
                    match api(&client_id, method, &format!("/me/library?uris={}", enc(&uri))) {
                        Ok(_) => {
                            let _ = tx.send(Msg::Liked(uri, like));
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::Failed(Some(uri), e));
                        }
                    }
                }
            }
            if !has_tokens() {
                let _ = tx.send(Msg::SignedIn(false));
            }
        }
    });
    cmd_tx
}

// ---------- tokens ----------

#[derive(Serialize, Deserialize, Clone)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    /// Unix seconds.
    expires_at: u64,
}

fn state_path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("osmia/spotify.json")
}

fn load() -> Option<Tokens> {
    serde_json::from_str(&std::fs::read_to_string(state_path()).ok()?).ok()
}

fn save(t: &Tokens) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let p = state_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string(t).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
}

/// A valid access token, refreshed when it is about to run out.
fn access(client_id: &str, force_refresh: bool) -> Result<String, String> {
    let t = load().ok_or("not signed in to Spotify")?;
    if !force_refresh && t.expires_at > now() + 60 {
        return Ok(t.access_token);
    }
    if client_id.is_empty() {
        return Err("no Spotify Client ID in the config".into());
    }
    let (code, body) = token_post(&[("grant_type", "refresh_token"), ("refresh_token", &t.refresh_token), ("client_id", client_id)])?;
    if code == 400 || code == 401 {
        // The refresh token was revoked or belongs to another app. Sign in again.
        let _ = std::fs::remove_file(state_path());
        return Err("the Spotify sign-in ran out. Sign in again".into());
    }
    if code != 200 {
        return Err(format!("Spotify refused the token refresh ({code})"));
    }
    let r: TokenReply = serde_json::from_str(&body).map_err(|e| format!("bad token reply: {e}"))?;
    // The reply may leave out the refresh token. Then the old one stays good.
    let t = Tokens { refresh_token: r.refresh_token.unwrap_or(t.refresh_token), access_token: r.access_token, expires_at: now() + r.expires_in };
    save(&t)?;
    Ok(t.access_token)
}

// ---------- HTTP through curl ----------

/// Runs curl with `stdin` on its standard input. Returns the status code and body.
fn curl(args: &[&str], stdin: &str) -> Result<(u16, String), String> {
    let mut child = Command::new("curl")
        .args(["-sS", "--max-time", "15", "-w", "\n%{http_code}"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if let Some(mut i) = child.stdin.take() {
        let _ = i.write_all(stdin.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    match code.trim().parse::<u16>() {
        Ok(c) if c != 0 => Ok((c, body.to_owned())),
        _ => Err(format!("cannot reach Spotify: {}", String::from_utf8_lossy(&out.stderr).trim())),
    }
}

fn token_post(form: &[(&str, &str)]) -> Result<(u16, String), String> {
    let body: Vec<String> = form.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect();
    curl(&["-X", "POST", TOKEN_URL, "-H", "Content-Type: application/x-www-form-urlencoded", "--data-binary", "@-"], &body.join("&"))
}

/// One Web API call. An expired token is refreshed and the call made once more.
fn api(client_id: &str, method: &str, path: &str) -> Result<String, String> {
    let url = format!("{API}{path}");
    let mut token = access(client_id, false)?;
    for attempt in 0..2 {
        let header = format!("Authorization: Bearer {token}\n");
        let mut args = vec!["-X", method, url.as_str(), "-H", "@-"];
        if method != "GET" {
            // PUT and DELETE carry no body, but Spotify wants a length.
            args.extend(["-H", "Content-Length: 0"]);
        }
        let (code, body) = curl(&args, &header)?;
        match code {
            200..=299 => return Ok(body),
            401 if attempt == 0 => token = access(client_id, true)?,
            401 => return Err("Spotify did not accept the sign-in. Sign in again".into()),
            403 => return Err("Spotify refused it (403). The Client ID's app must be owned by a Premium account, and your account added under User Management".into()),
            429 => return Err("Spotify asked to slow down. Try again in a moment".into()),
            c => return Err(format!("Spotify answered {c}: {}", short(&body))),
        }
    }
    unreachable!()
}

fn contains(client_id: &str, uris: &[String]) -> Result<Vec<bool>, String> {
    let q: Vec<String> = uris.iter().map(|u| enc(u)).collect();
    let body = api(client_id, "GET", &format!("/me/library/contains?uris={}", q.join(",")))?;
    let v: Vec<bool> = serde_json::from_str(&body).map_err(|e| format!("bad reply from Spotify: {e}"))?;
    if v.len() != uris.len() {
        return Err("bad reply from Spotify".into());
    }
    Ok(v)
}

fn short(s: &str) -> String {
    let s = s.trim();
    // Spotify errors look like {"error":{"status":400,"message":"..."}}.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        if let Some(m) = v.pointer("/error/message").and_then(|m| m.as_str()) {
            return m.to_owned();
        }
        if let Some(m) = v.get("error_description").and_then(|m| m.as_str()) {
            return m.to_owned();
        }
    }
    s.chars().take(120).collect()
}

/// Percent-encodes everything but the unreserved characters.
pub fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut o = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    o.push(v);
                    i += 3;
                    continue;
                }
                o.push(b'%');
            }
            b'+' => o.push(b' '),
            c => o.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}

// ---------- sign-in ----------

fn random_string(n: usize) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = vec![0u8; n];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut bytes);
    }
    // 62 symbols: the small bias of the modulo does not matter for a one-time verifier.
    bytes.iter().map(|b| ABC[*b as usize % ABC.len()] as char).collect()
}

/// base64url without padding, for the PKCE challenge.
fn b64url(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut o = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            o.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    o
}

pub fn challenge(verifier: &str) -> String {
    use sha2::Digest;
    b64url(&sha2::Sha256::digest(verifier.as_bytes()))
}

/// Opens the browser at Spotify's sign-in page, waits for it to come back to the
/// loopback address, and trades the code for tokens.
fn login(client_id: &str) -> Result<(), String> {
    if client_id.is_empty() {
        return Err("no Spotify Client ID in the config".into());
    }
    let listener = TcpListener::bind(("127.0.0.1", REDIRECT_PORT)).map_err(|e| format!("cannot listen on port {REDIRECT_PORT} for the sign-in: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let verifier = random_string(96);
    let state = random_string(24);
    let url = format!(
        "https://accounts.spotify.com/authorize?response_type=code&client_id={}&scope={}&redirect_uri={}&code_challenge_method=S256&code_challenge={}&state={}",
        enc(client_id),
        enc(SCOPES),
        enc(&redirect_uri()),
        challenge(&verifier),
        state
    );
    Command::new("xdg-open").arg(&url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("cannot open the browser: {e}"))?;
    let deadline = Instant::now() + LOGIN_WAIT;
    let code = loop {
        if Instant::now() > deadline {
            return Err("the Spotify sign-in timed out".into());
        }
        let mut s = match listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let Some(query) = read_callback(&mut s) else {
            reply(&mut s, "404 Not Found", "Not here.");
            continue;
        };
        let get = |k: &str| query.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        if get("state").as_deref() != Some(state.as_str()) {
            reply(&mut s, "400 Bad Request", "This sign-in does not match the one osmia started.");
            continue;
        }
        if let Some(e) = get("error") {
            reply(&mut s, "200 OK", "Spotify sign-in was cancelled. You can close this tab.");
            return Err(format!("Spotify sign-in failed: {e}"));
        }
        match get("code") {
            Some(c) => {
                reply(&mut s, "200 OK", "osmia is signed in to Spotify. You can close this tab.");
                break c;
            }
            None => reply(&mut s, "400 Bad Request", "No code in the reply."),
        }
    };
    let redirect = redirect_uri();
    let (status, body) = token_post(&[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect),
        ("client_id", client_id),
        ("code_verifier", &verifier),
    ])?;
    if status != 200 {
        return Err(format!("Spotify refused the sign-in: {}", short(&body)));
    }
    let r: TokenReply = serde_json::from_str(&body).map_err(|e| format!("bad token reply: {e}"))?;
    let refresh_token = r.refresh_token.ok_or("Spotify sent no refresh token")?;
    save(&Tokens { access_token: r.access_token, refresh_token, expires_at: now() + r.expires_in })
}

/// Reads one request. Returns the query of GET /callback.
fn read_callback(s: &mut TcpStream) -> Option<Vec<(String, String)>> {
    let _ = s.set_nonblocking(false);
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0u8; 4096];
    let n = s.read(&mut buf).ok()?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let target = req.lines().next()?.strip_prefix("GET ")?.split(' ').next()?;
    let query = target.strip_prefix("/callback?")?;
    Some(
        query
            .split('&')
            .filter_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                Some((dec(k), dec(v)))
            })
            .collect(),
    )
}

fn reply(s: &mut TcpStream, status: &str, text: &str) {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>osmia</title><body style=\"font:16px sans-serif;background:#1e2126;color:#d6dae0;display:grid;place-items:center;height:90vh\"><p>{text}</p>"
    );
    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spotify_track_ids_become_uris() {
        let bus = "org.mpris.MediaPlayer2.spotify";
        assert_eq!(track_uri(bus, "/com/spotify/track/4QZs0LVBrwxrSEE3UCz1Zz").as_deref(), Some("spotify:track:4QZs0LVBrwxrSEE3UCz1Zz"));
        assert_eq!(track_uri(bus, "/com/spotify/ad/123"), None);
        assert_eq!(track_uri(bus, ""), None);
        assert_eq!(track_uri("org.mpris.MediaPlayer2.vlc", "/com/spotify/track/abc"), None);
    }

    #[test]
    fn pkce_challenge_matches_the_rfc_example() {
        // RFC 7636, appendix B.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn encoding_round_trips() {
        assert_eq!(enc("spotify:track:x y"), "spotify%3Atrack%3Ax%20y");
        assert_eq!(dec("a%3Ab+c"), "a:b c");
        assert_eq!(dec("100%"), "100%");
    }
}
