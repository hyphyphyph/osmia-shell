//! File dialogs: which xdg-desktop-portal backend draws the file picker.
//! The choice lives in osmia-portals.conf in the user's xdg-desktop-portal
//! config folder. Osmia is the first desktop name in the session, so that file
//! wins over the system's labwc one. See spec/FileDialogs.pi.

use std::fs;
use std::path::{Path, PathBuf};

const FILE_CHOOSER: &str = "org.freedesktop.impl.portal.FileChooser";
const OUR_FILE: &str = "osmia-portals.conf";

#[derive(Clone, Debug, PartialEq)]
pub struct Picker {
    /// The .portal file name without the extension, such as gtk. This is the name the config uses.
    pub id: String,
    pub name: String,
    /// A picker whose portal is not set up yet: the command that sets it up.
    pub setup: Option<Vec<String>>,
}

#[derive(Clone, Default)]
pub struct FileDialogs {
    /// Installed backends that offer a file chooser, by name.
    pub pickers: Vec<Picker>,
    /// The backend chosen in our file. None is Automatic.
    pub current: Option<String>,
    /// GTK apps are told to ask the portal too.
    pub gtk: bool,
    /// paru or yay, for pickers from the AUR.
    pub aur_helper: Option<String>,
    /// The package whose install terminal is open.
    pub installing: Option<String>,
}

/// A picker the page can install, with the Arch package that brings it.
pub struct Offer {
    /// The portal name it will have once installed.
    pub id: &'static str,
    pub about: &'static str,
    pub pkg: &'static str,
    /// From the AUR, built with paru or yay. Otherwise from the official repos.
    pub aur: bool,
}

pub const OFFERS: &[Offer] = &[
    Offer { id: "gtk", about: "The common GTK dialog.", pkg: "xdg-desktop-portal-gtk", aur: false },
    Offer { id: "kde", about: "The KDE dialog, with Dolphin's views.", pkg: "xdg-desktop-portal-kde", aur: false },
    Offer { id: "gnome", about: "The GNOME Files dialog, in GTK 4.", pkg: "xdg-desktop-portal-gnome", aur: false },
    Offer { id: "lxqt", about: "A light Qt dialog.", pkg: "xdg-desktop-portal-lxqt", aur: false },
    Offer { id: "cosmic", about: "The COSMIC desktop's dialog.", pkg: "xdg-desktop-portal-cosmic", aur: false },
    Offer { id: "strata", about: "A keyboard-first file manager that can be the picker.", pkg: "strata-bin", aur: true },
    Offer { id: "pikeru", about: "Thumbnails, and search by what is in a picture.", pkg: "pikeru", aur: true },
    Offer { id: "termfilechooser", about: "A terminal file manager such as yazi. Needs one, and its own config.", pkg: "xdg-desktop-portal-termfilechooser", aur: true },
    Offer { id: "shana", about: "Sends open and save dialogs on to other pickers. Needs its own config.", pkg: "xdg-desktop-portal-shana", aur: true },
    Offer { id: "tui", about: "A terminal file browser. Needs kitty.", pkg: "xdg-desktop-portal-tui", aur: true },
    Offer { id: "adw", about: "A Libadwaita dialog.", pkg: "xdg-desktop-portal-adw", aur: true },
];

impl FileDialogs {
    /// The pickers that can be installed: those not installed yet.
    pub fn offers(&self) -> impl Iterator<Item = &'static Offer> + '_ {
        OFFERS.iter().filter(|o| !self.pickers.iter().any(|p| p.id == o.id))
    }
}

/// The shell command that installs `o`, or None with no AUR helper for an AUR package.
pub fn install_command(o: &Offer, aur_helper: Option<&str>) -> Option<String> {
    if o.aur {
        aur_helper.map(|h| format!("{h} -S --needed {}", o.pkg))
    } else {
        Some(format!("sudo pacman -S --needed {}", o.pkg))
    }
}

/// The terminal to install in: the configured one if it is installed, else the
/// first known one that is. Each takes -e and a command.
pub fn pick_terminal(configured: &str) -> Option<String> {
    let first = configured.split_whitespace().next().unwrap_or("");
    if !first.is_empty() && (Path::new(first).is_file() || on_path(first).is_some()) {
        return Some(configured.to_owned());
    }
    ["alacritty", "foot", "kitty", "wezterm", "konsole", "qterminal", "xfce4-terminal", "xterm"].iter().find(|t| on_path(t).is_some()).map(|t| t.to_string())
}

/// Run `cmd` in a terminal window and wait for the window to close. Keeps the
/// window open at the end so the user can read what happened.
pub fn run_in_terminal(terminal: &str, title: &str, cmd: &str) -> Result<(), String> {
    let script = format!(
        "printf '%s\\n\\n' {t}; {cmd}; s=$?; echo; if [ $s -eq 0 ]; then echo Done.; else echo \"It failed ($s).\"; fi; printf 'Press Enter to close. '; read _",
        t = sh_quote(title)
    );
    let mut parts = terminal.split_whitespace();
    let prog = parts.next().ok_or("No terminal")?;
    // foot runs its arguments as the command and has no -e.
    let mut c = std::process::Command::new(prog);
    c.args(parts);
    if Path::new(prog).file_name().and_then(|n| n.to_str()) != Some("foot") {
        c.arg("-e");
    }
    c.args(["sh", "-c", &script]);
    let mut child = c.spawn().map_err(|e| format!("{prog}: {e}"))?;
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl FileDialogs {
    /// The name shown for the current choice.
    pub fn current_label(&self) -> String {
        match &self.current {
            None => "Automatic".to_owned(),
            Some(id) => match self.pickers.iter().find(|p| &p.id == id) {
                Some(p) => p.name.clone(),
                None => format!("{} (not installed)", friendly(id)),
            },
        }
    }
}

pub fn friendly_name(id: &str) -> String {
    friendly(id)
}

fn friendly(id: &str) -> String {
    match id {
        "gtk" => "GTK".into(),
        "kde" => "KDE".into(),
        "gnome" => "GNOME".into(),
        "lxqt" => "LXQt".into(),
        "cosmic" => "COSMIC".into(),
        "pikeru" => "Pikeru".into(),
        "strata" => "Strata".into(),
        "termfilechooser" => "Terminal file manager".into(),
        "shana" => "Shana".into(),
        "tui" => "Terminal browser (TUI)".into(),
        "adw" => "Libadwaita".into(),
        "gtk4" => "GTK 4".into(),
        "phosh" => "Phosh".into(),
        "dde" => "Deepin".into(),
        other => {
            let mut c = other.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
    }
}

/// Whether a .portal file says it offers a file chooser.
fn offers_file_chooser(text: &str) -> bool {
    text.lines().filter_map(|l| l.trim().strip_prefix("Interfaces=")).any(|v| v.split(';').any(|i| i.trim() == FILE_CHOOSER))
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// The data folders, such as /usr/share.
fn data_dirs() -> Vec<PathBuf> {
    crate::launch::data_dirs().into_iter().filter_map(|d| d.parent().map(Path::to_path_buf)).collect()
}

fn our_file() -> Option<PathBuf> {
    config_home().map(|c| c.join("xdg-desktop-portal").join(OUR_FILE))
}

/// The labwc environment file the session reads, in the shell's config folder.
fn env_file() -> Option<PathBuf> {
    config_home().map(|c| c.join("osmia/labwc/environment"))
}

/// Pickers that are apps with a portal they install for the user when asked:
/// the command name, which is also the portal name, and the arguments that set it up.
const SETUP: &[(&str, &[&str])] = &[("strata", &["--install-portal"])];

fn on_path(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(cmd)).find(|p| p.is_file())
}

pub fn load() -> FileDialogs {
    let mut pickers: Vec<Picker> = Vec::new();
    for d in data_dirs() {
        let Ok(rd) = fs::read_dir(d.join("xdg-desktop-portal/portals")) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("portal") {
                continue;
            }
            let Some(id) = p.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else { continue };
            if pickers.iter().any(|x| x.id == id) {
                continue;
            }
            if fs::read_to_string(&p).is_ok_and(|t| offers_file_chooser(&t)) {
                pickers.push(Picker { name: friendly(&id), id, setup: None });
            }
        }
    }
    // Pickers that bring their own portal but set it up per user, on request.
    for (id, args) in SETUP {
        if pickers.iter().any(|p| p.id == *id) {
            continue;
        }
        if let Some(cmd) = on_path(id) {
            let mut v = vec![cmd.to_string_lossy().into_owned()];
            v.extend(args.iter().map(|a| a.to_string()));
            pickers.push(Picker { id: id.to_string(), name: friendly(id), setup: Some(v) });
        }
    }
    pickers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let current = our_file().and_then(|f| fs::read_to_string(f).ok()).and_then(|t| chooser_of(&t));
    let gtk = env_file().and_then(|f| fs::read_to_string(f).ok()).is_some_and(|t| gtk_on(&t));
    let aur_helper = ["paru", "yay"].iter().find(|h| on_path(h).is_some()).map(|h| h.to_string());
    FileDialogs { pickers, current, gtk, aur_helper, installing: None }
}

/// The first backend named for the file chooser under [preferred].
fn chooser_of(text: &str) -> Option<String> {
    let mut in_pref = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_pref = line == "[preferred]";
            continue;
        }
        if in_pref {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == FILE_CHOOSER {
                    return v.split(';').map(str::trim).find(|s| !s.is_empty()).map(str::to_owned);
                }
            }
        }
    }
    None
}

/// The value for the file chooser line: the choice, then the one before it as a
/// fallback, which the portal uses if the choice cannot start.
fn chooser_value(id: &str, before: Option<&str>) -> String {
    match before {
        Some(b) if b != id => format!("{id};{b}"),
        _ => id.to_owned(),
    }
}

/// Set (or with None, remove) the file chooser line under [preferred], keeping every other line.
fn with_chooser(text: &str, id: Option<&str>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_pref = false;
    let mut seen_pref = false;
    let mut done = false;
    let new_line = id.map(|i| format!("{FILE_CHOOSER}={i}"));
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if in_pref && !done {
                // Put it at the end of [preferred], before any blank lines.
                if let Some(l) = &new_line {
                    let at = out.iter().rposition(|x| !x.trim().is_empty()).map(|i| i + 1).unwrap_or(out.len());
                    out.insert(at, l.clone());
                }
                done = true;
            }
            in_pref = t == "[preferred]";
            seen_pref |= in_pref;
            out.push(line.to_owned());
            continue;
        }
        if in_pref && t.split_once('=').is_some_and(|(k, _)| k.trim() == FILE_CHOOSER) {
            if !done {
                if let Some(l) = &new_line {
                    out.push(l.clone());
                }
                done = true;
            }
            continue;
        }
        out.push(line.to_owned());
    }
    if !done {
        if let Some(l) = new_line {
            if !seen_pref {
                if out.last().is_some_and(|l| !l.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push("[preferred]".into());
            }
            out.push(l);
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// The file the portal used before ours existed: the first labwc or wlroots
/// file (the session's other desktop names), folder by folder, then portals.conf.
fn seed_text(desktops: &[String], dirs: &[PathBuf]) -> String {
    for d in dirs {
        for name in desktops.iter().map(|n| format!("{n}-portals.conf")).chain(std::iter::once("portals.conf".to_owned())) {
            if let Ok(t) = fs::read_to_string(d.join(&name)) {
                return t;
            }
        }
    }
    "[preferred]\ndefault=*\n".to_owned()
}

fn seed() -> String {
    let mut desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .map(|d| d.trim().to_lowercase())
        .filter(|d| !d.is_empty() && d != "osmia")
        .collect();
    if desktops.is_empty() {
        desktops = vec!["labwc".into(), "wlroots".into()];
    }
    let mut dirs: Vec<PathBuf> = config_home().into_iter().collect();
    let cdirs = std::env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".into());
    dirs.extend(cdirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    dirs.push(PathBuf::from("/etc"));
    dirs.extend(data_dirs());
    let dirs: Vec<PathBuf> = dirs.into_iter().map(|d| d.join("xdg-desktop-portal")).collect();
    seed_text(&desktops, &dirs)
}

/// Choose the file picker backend, or None for Automatic, and restart the portal.
/// A picker that is not set up yet is set up first. That waits for its command,
/// so call this off the drawing thread.
pub fn set_picker(id: Option<&str>, setup: Option<&[String]>) -> Result<(), String> {
    let path = our_file().ok_or("No config folder")?;
    // What was chosen before, read before any setup command edits the file.
    let before = fs::read_to_string(&path).ok().and_then(|t| chooser_of(&t));
    if let Some(cmd) = setup.filter(|c| !c.is_empty()) {
        let out = std::process::Command::new(&cmd[0]).args(&cmd[1..]).output().map_err(|e| format!("{}: {e}", cmd[0]))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let line = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("it failed");
            return Err(format!("Could not set up the picker: {line}"));
        }
    }
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        // Automatic with no file of ours: nothing to undo.
        Err(_) if id.is_none() => return Ok(()),
        Err(_) => seed(),
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let value = id.map(|i| chooser_value(i, before.as_deref()));
    fs::write(&path, with_chooser(&text, value.as_deref())).map_err(|e| e.to_string())?;
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "restart", "xdg-desktop-portal.service"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    Ok(())
}

/// Ask the portal for an open-file dialog, to see the picker. The answer is ignored.
///
/// The asking connection stays open until the dialog answers. The portal closes a
/// request whose caller has left the bus, so a one-shot `gdbus call` made pickers
/// that honour that, such as Strata, close at once.
pub fn try_picker() {
    std::thread::spawn(|| {
        if let Err(e) = pollster::block_on(open_and_wait()) {
            log::warn!("try file picker: {e}");
            let _ = std::process::Command::new("notify-send").args(["-a", "Osmia", "The file picker did not open", &e.to_string()]).spawn();
        }
    });
}

async fn open_and_wait() -> zbus::Result<()> {
    use futures_lite::StreamExt;
    use zbus::{OwnedObjectPath, Variant as Value};
    let conn = zbus::Connection::session().await?;
    // Listen before asking, so a quick answer is not missed.
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Request")
        .member("Response")
        .build()?;
    let mut answers = zbus::MessageStream::for_match_rule(rule, &conn, None).await?;
    let token = format!("osmia_try_{}", std::process::id());
    let mut opts: std::collections::HashMap<&str, Value> = std::collections::HashMap::new();
    opts.insert("handle_token", Value::from(token.as_str()));
    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.FileChooser"),
            "OpenFile",
            &("", "Try the file picker", opts),
        )
        .await?;
    let handle: OwnedObjectPath = reply.body().deserialize()?;
    while let Some(m) = answers.next().await {
        let m = m?;
        if m.header().path().map(|p| p.as_str()) == Some(handle.as_str()) {
            let (code, _): (u32, std::collections::HashMap<String, zbus::OwnedVariant>) = m.body().deserialize()?;
            // 0 is a pick, 1 is cancelled; 2 means the picker failed.
            if code == 2 {
                return Err(zbus::Error::Failure("The picker could not show a dialog.".into()));
            }
            return Ok(());
        }
    }
    Ok(())
}

fn env_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let l = line.trim();
    let l = l.strip_prefix("export ").unwrap_or(l);
    l.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().trim_matches('"').trim_matches('\''))
}

fn gtk_on(text: &str) -> bool {
    text.lines().filter_map(|l| env_value(l, "GTK_USE_PORTAL")).any(|v| v == "1")
}

/// Turn the GTK lines on or off, keeping every other line and GDK_DEBUG flag.
fn with_gtk(text: &str, on: bool) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut had_debug = false;
    for line in text.lines() {
        if env_value(line, "GTK_USE_PORTAL").is_some() {
            continue;
        }
        if let Some(v) = env_value(line, "GDK_DEBUG") {
            had_debug = true;
            let mut flags: Vec<&str> = v.split([',', ':', ' ']).filter(|f| !f.is_empty() && *f != "portals").collect();
            if on {
                flags.push("portals");
            }
            if !flags.is_empty() {
                out.push(format!("GDK_DEBUG={}", flags.join(",")));
            }
            continue;
        }
        out.push(line.to_owned());
    }
    if on {
        out.push("GTK_USE_PORTAL=1".into());
        if !had_debug {
            out.push("GDK_DEBUG=portals".into());
        }
    }
    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

/// Tell GTK apps to use the portal's file dialog, from the next login.
pub fn set_gtk(on: bool) -> Result<(), String> {
    let path = env_file().ok_or("No config folder")?;
    let text = fs::read_to_string(&path).unwrap_or_default();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, with_gtk(&text, on)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_portal_file() {
        assert!(offers_file_chooser("[portal]\nDBusName=x\nInterfaces=org.freedesktop.impl.portal.Access;org.freedesktop.impl.portal.FileChooser;\n"));
        assert!(!offers_file_chooser("[portal]\nInterfaces=org.freedesktop.impl.portal.Screenshot;\n"));
    }

    #[test]
    fn sets_and_removes_chooser() {
        let base = "[preferred]\ndefault=wlr;*\norg.freedesktop.impl.portal.Inhibit=none\n";
        let set = with_chooser(base, Some("kde"));
        assert_eq!(set, "[preferred]\ndefault=wlr;*\norg.freedesktop.impl.portal.Inhibit=none\norg.freedesktop.impl.portal.FileChooser=kde\n");
        assert_eq!(chooser_of(&set).as_deref(), Some("kde"));
        let changed = with_chooser(&set, Some("gtk"));
        assert_eq!(chooser_of(&changed).as_deref(), Some("gtk"));
        assert_eq!(changed.matches("FileChooser").count(), 1);
        assert_eq!(with_chooser(&set, None), base);
    }

    #[test]
    fn install_commands() {
        let strata = OFFERS.iter().find(|o| o.id == "strata").unwrap();
        let gtk = OFFERS.iter().find(|o| o.id == "gtk").unwrap();
        assert_eq!(install_command(strata, Some("paru")).as_deref(), Some("paru -S --needed strata-bin"));
        assert_eq!(install_command(strata, None), None);
        assert_eq!(install_command(gtk, None).as_deref(), Some("sudo pacman -S --needed xdg-desktop-portal-gtk"));
        let d = FileDialogs { pickers: vec![Picker { id: "gtk".into(), name: "GTK".into(), setup: None }], ..Default::default() };
        assert!(d.offers().all(|o| o.id != "gtk"));
        assert!(d.offers().any(|o| o.id == "strata"));
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn keeps_the_last_choice_as_fallback() {
        assert_eq!(chooser_value("strata", Some("lxqt")), "strata;lxqt");
        assert_eq!(chooser_value("gtk", Some("gtk")), "gtk");
        assert_eq!(chooser_value("kde", None), "kde");
        // The current choice is the first name.
        let t = with_chooser("[preferred]\n", Some(&chooser_value("strata", Some("lxqt"))));
        assert_eq!(chooser_of(&t).as_deref(), Some("strata"));
    }

    #[test]
    fn chooser_goes_in_preferred_section() {
        let t = "[preferred]\ndefault=*\n\n[other]\nx=1\n";
        let out = with_chooser(t, Some("gtk"));
        assert_eq!(out, "[preferred]\ndefault=*\norg.freedesktop.impl.portal.FileChooser=gtk\n\n[other]\nx=1\n");
        assert_eq!(with_chooser("", Some("gtk")), "[preferred]\norg.freedesktop.impl.portal.FileChooser=gtk\n");
        // A line in another section is not ours.
        assert_eq!(chooser_of("[other]\norg.freedesktop.impl.portal.FileChooser=kde\n"), None);
    }

    #[test]
    fn seed_takes_first_match() {
        let dir = std::env::temp_dir().join(format!("osmia-portal-test-{}", std::process::id()));
        let (a, b) = (dir.join("a"), dir.join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("labwc-portals.conf"), "[preferred]\ndefault=wlr;*\n").unwrap();
        fs::write(a.join("portals.conf"), "[preferred]\ndefault=gtk\n").unwrap();
        let desktops = vec!["labwc".to_owned(), "wlroots".to_owned()];
        // Folder order comes first, so a's portals.conf beats b's labwc file.
        assert_eq!(seed_text(&desktops, &[a.clone(), b.clone()]), "[preferred]\ndefault=gtk\n");
        assert_eq!(seed_text(&desktops, &[b.clone()]), "[preferred]\ndefault=wlr;*\n");
        assert_eq!(seed_text(&desktops, &[dir.join("none")]), "[preferred]\ndefault=*\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn gtk_lines() {
        let base = "XCURSOR_THEME=elementary\nXCURSOR_SIZE=32\n";
        let on = with_gtk(base, true);
        assert!(gtk_on(&on));
        assert!(on.contains("GDK_DEBUG=portals"));
        assert_eq!(with_gtk(&on, false), base);
        // Other GDK_DEBUG flags stay.
        let mixed = with_gtk("GDK_DEBUG=events\n", true);
        assert!(mixed.contains("GDK_DEBUG=events,portals"));
        assert_eq!(with_gtk(&mixed, false), "GDK_DEBUG=events\n");
        assert!(!gtk_on(base));
    }
}

#[cfg(test)]
mod live {
    /// Opens a real dialog. Run by hand: cargo test try_dialog -- --ignored
    #[test]
    #[ignore]
    fn try_dialog() {
        pollster::block_on(super::open_and_wait()).unwrap();
    }
}
