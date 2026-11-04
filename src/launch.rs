//! Launcher logic: desktop apps, fuzzy matching, calculator, quick actions,
//! and the usage counts that rank results.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::os::unix::process::CommandExt;

use crate::ui::PowerAction;

#[derive(Clone, Debug)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub comment: String,
    pub terminal: bool,
}

#[derive(Clone, Debug)]
pub struct WinInfo {
    pub index: usize,
    pub title: String,
    pub app_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    App,
    Window,
    Action,
    Command,
    Calc,
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::App => "App",
            Source::Window => "Window",
            Source::Action => "Action",
            Source::Command => "Command",
            Source::Calc => "Calculator",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Kind {
    App(DesktopApp),
    Window(usize),
    Action(PowerAction),
    Command(String),
    Calc(String),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub name: String,
    pub source: Source,
    pub score: f64,
    pub matches: Vec<usize>,
    pub kind: Kind,
}

pub struct Data {
    pub apps: Vec<DesktopApp>,
    usage: HashMap<String, (u32, u64)>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn usage_path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("osmia").join("usage.tsv")
}

impl Data {
    pub fn load() -> Self {
        let mut usage = HashMap::new();
        if let Ok(text) = fs::read_to_string(usage_path()) {
            for line in text.lines() {
                let mut it = line.split('\t');
                if let (Some(id), Some(c), Some(t)) = (it.next(), it.next(), it.next()) {
                    if let (Ok(c), Ok(t)) = (c.parse(), t.parse()) {
                        usage.insert(id.to_owned(), (c, t));
                    }
                }
            }
        }
        Data { apps: scan_apps(), usage }
    }

    fn boost(&self, id: &str) -> f64 {
        match self.usage.get(id) {
            // Entries unused for 90 days fade out.
            Some((c, t)) if now_secs().saturating_sub(*t) < 90 * 86400 => 1.0 + 0.25 * (1.0 + *c as f64).ln(),
            _ => 1.0,
        }
    }

    pub fn record(&mut self, id: &str) {
        let e = self.usage.entry(id.to_owned()).or_insert((0, 0));
        e.0 += 1;
        e.1 = now_secs();
        let p = usage_path();
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let text: String = self
            .usage
            .iter()
            .map(|(k, (c, t))| format!("{k}\t{c}\t{t}\n"))
            .collect();
        let _ = fs::write(p, text);
    }

    /// The most used apps, for an empty search box.
    pub fn top(&self, n: usize) -> Vec<Item> {
        let mut apps: Vec<&DesktopApp> = self.apps.iter().collect();
        apps.sort_by(|a, b| {
            self.boost(&b.id)
                .partial_cmp(&self.boost(&a.id))
                .unwrap()
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        apps.into_iter()
            .take(n)
            .map(|a| Item {
                name: a.name.clone(),
                source: Source::App,
                score: 0.0,
                matches: vec![],
                kind: Kind::App(a.clone()),
            })
            .collect()
    }
}

pub(crate) fn data_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(h) = std::env::var_os("XDG_DATA_HOME") {
        v.push(PathBuf::from(h).join("applications"));
    } else if let Some(h) = std::env::var_os("HOME") {
        v.push(PathBuf::from(h).join(".local/share/applications"));
    }
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in dirs.split(':').filter(|d| !d.is_empty()) {
        v.push(PathBuf::from(d).join("applications"));
    }
    v
}

fn scan_apps() -> Vec<DesktopApp> {
    let desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .map(str::to_owned)
        .collect();
    let mut seen: HashMap<String, DesktopApp> = HashMap::new();
    for dir in data_dirs() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }
            let id = path.file_name().unwrap().to_string_lossy().into_owned();
            if seen.contains_key(&id) {
                continue; // earlier directories win
            }
            if let Some(app) = parse_desktop(&path, &id, &desktops) {
                seen.insert(id, app);
            }
        }
    }
    let mut apps: Vec<DesktopApp> = seen.into_values().collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

fn parse_desktop(path: &std::path::Path, id: &str, desktops: &[String]) -> Option<DesktopApp> {
    let text = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let (mut name, mut exec, mut comment) = (None, None, String::new());
    let (mut hidden, mut terminal, mut is_app) = (false, false, false);
    let (mut only, mut not): (Vec<String>, Vec<String>) = (vec![], vec![]);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        match k.trim() {
            "Name" => name = Some(v.to_owned()),
            "Exec" => exec = Some(v.to_owned()),
            "Comment" => comment = v.to_owned(),
            "Type" => is_app = v == "Application",
            "NoDisplay" | "Hidden" => hidden |= v == "true",
            "Terminal" => terminal = v == "true",
            "OnlyShowIn" => only = v.split(';').filter(|s| !s.is_empty()).map(str::to_owned).collect(),
            "NotShowIn" => not = v.split(';').filter(|s| !s.is_empty()).map(str::to_owned).collect(),
            _ => {}
        }
    }
    if hidden || !is_app {
        return None;
    }
    if !only.is_empty() && !only.iter().any(|o| desktops.contains(o)) {
        return None;
    }
    if not.iter().any(|n| desktops.contains(n)) {
        return None;
    }
    Some(DesktopApp {
        id: id.to_owned(),
        name: name?,
        exec: strip_field_codes(&exec?),
        comment,
        terminal,
    })
}

fn strip_field_codes(exec: &str) -> String {
    let mut out = String::new();
    let mut it = exec.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' {
            match it.next() {
                Some('%') => out.push('%'),
                Some(_) | None => {}
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

/// Greedy subsequence match. Returns a score and the matched char positions.
pub fn fuzzy(query: &str, text: &str) -> Option<(f64, Vec<usize>)> {
    let q: Vec<char> = query.to_lowercase().chars().collect();
    if q.is_empty() {
        return Some((0.0, vec![]));
    }
    let t: Vec<char> = text.chars().collect();
    let tl: Vec<char> = text.to_lowercase().chars().collect();
    if tl.len() != t.len() {
        return None;
    }
    let mut pos = Vec::new();
    let mut qi = 0;
    let mut score = 0.0;
    let mut prev: Option<usize> = None;
    for (i, c) in tl.iter().enumerate() {
        if qi < q.len() && *c == q[qi] {
            score += 1.0;
            if prev == Some(i.wrapping_sub(1)) && i > 0 {
                score += 2.0;
            }
            if i == 0 {
                score += 5.0;
            } else if !t[i - 1].is_alphanumeric() {
                score += 3.0;
            }
            pos.push(i);
            prev = Some(i);
            qi += 1;
        }
    }
    if qi < q.len() {
        return None;
    }
    // Shorter names with the same letters match better.
    score -= (t.len() as f64 - q.len() as f64) * 0.05;
    Some((score, pos))
}

pub fn search(data: &Data, windows: &[WinInfo], raw: &str, max: usize) -> Vec<Item> {
    let raw = raw.trim_start();
    let (prefix, query) = match raw.chars().next() {
        Some(c @ ('>' | '@' | ':')) => (Some(c), raw[1..].trim_start()),
        _ => (None, raw),
    };
    let mut items: Vec<Item> = Vec::new();

    if prefix == Some('>') {
        if !query.is_empty() {
            items.push(Item {
                name: query.to_owned(),
                source: Source::Command,
                score: 100.0,
                matches: vec![],
                kind: Kind::Command(query.to_owned()),
            });
        }
        return items;
    }

    if prefix.is_none() {
        if let Some(ans) = crate::calc::answer(query) {
            items.push(Item {
                name: ans.clone(),
                source: Source::Calc,
                score: 1000.0,
                matches: vec![],
                kind: Kind::Calc(ans),
            });
        }
        if query.is_empty() {
            items.extend(data.top(max));
            return items;
        }
    }

    if prefix.is_none() || prefix == Some('@') {
        for w in windows {
            let label = if w.title.is_empty() { w.app_id.clone() } else { w.title.clone() };
            if let Some((s, m)) = if query.is_empty() { Some((0.0, vec![])) } else { fuzzy(query, &label) } {
                items.push(Item { name: label, source: Source::Window, score: s * 1.2, matches: m, kind: Kind::Window(w.index) });
            }
        }
    }
    if prefix.is_none() {
        for a in &data.apps {
            let mut best = fuzzy(query, &a.name);
            if best.is_none() && !a.comment.is_empty() {
                // A match in the comment ranks lower and shows no highlight.
                best = fuzzy(query, &a.comment).map(|(s, _)| (s * 0.4, vec![]));
            }
            if let Some((s, m)) = best {
                items.push(Item {
                    name: a.name.clone(),
                    source: Source::App,
                    score: s * data.boost(&a.id),
                    matches: m,
                    kind: Kind::App(a.clone()),
                });
            }
        }
    }
    if prefix.is_none() || prefix == Some(':') {
        let mut actions = vec![PowerAction::Lock, PowerAction::Logout, PowerAction::Suspend, PowerAction::Reboot, PowerAction::PowerOff];
        if crate::sys::can_hibernate() {
            actions.insert(3, PowerAction::Hibernate);
        }
        for a in actions {
            if let Some((s, m)) = if query.is_empty() { Some((0.0, vec![])) } else { fuzzy(query, a.label()) } {
                items.push(Item { name: a.label().to_owned(), source: Source::Action, score: s, matches: m, kind: Kind::Action(a) });
            }
        }
    }
    if prefix.is_none() && !query.is_empty() && items.iter().all(|i| i.source != Source::App && i.source != Source::Calc) && items.len() < max {
        items.push(Item {
            name: format!("Run {query}"),
            source: Source::Command,
            score: -1.0,
            matches: vec![],
            kind: Kind::Command(query.to_owned()),
        });
    }
    items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    items.truncate(max.max(8));
    items
}

/// Start a program detached from the shell: its own session, no terminal.
pub fn spawn(cmd: &str) {
    let mut c = Command::new("sh");
    c.arg("-c").arg(cmd).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // SAFETY: setsid is async-signal-safe and runs before exec.
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    if let Err(e) = c.spawn() {
        log::warn!("cannot run {cmd:?}: {e}");
    }
}

pub fn run_app(app: &DesktopApp, terminal: &str) {
    if app.terminal {
        spawn(&format!("{terminal} -e sh -c {}", shell_quote(&app.exec)));
    } else {
        spawn(&app.exec);
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn run_command(cmd: &str) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
    let mut c = Command::new(shell);
    c.arg("-c").arg(cmd).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    if let Err(e) = c.spawn() {
        log::warn!("cannot run {cmd:?}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_prefers_prefix_and_rejects_non_matches() {
        let (a, _) = fuzzy("fire", "Firefox").unwrap();
        let (b, _) = fuzzy("fire", "Wildfire Games").unwrap();
        assert!(a > b);
        assert!(fuzzy("xyz", "Firefox").is_none());
        let (_, pos) = fuzzy("ff", "Firefox").unwrap();
        assert_eq!(pos, vec![0, 4]);
    }

    #[test]
    fn exec_field_codes_are_removed() {
        assert_eq!(strip_field_codes("firefox %u"), "firefox");
        assert_eq!(strip_field_codes("app --name 100%% %F"), "app --name 100%");
    }

    fn data(names: &[&str]) -> Data {
        Data {
            apps: names
                .iter()
                .map(|n| DesktopApp { id: format!("{n}.desktop"), name: n.to_string(), exec: n.to_lowercase(), comment: String::new(), terminal: false })
                .collect(),
            usage: HashMap::new(),
        }
    }

    #[test]
    fn search_modes() {
        let d = data(&["Firefox", "Files", "Terminal"]);
        let r = search(&d, &[], "fire", 8);
        assert_eq!(r[0].name, "Firefox");
        let r = search(&d, &[], "2+2", 8);
        assert_eq!(r[0].source, Source::Calc);
        assert_eq!(r[0].name, "4");
        let r = search(&d, &[], "> ls -la", 8);
        assert!(matches!(&r[0].kind, Kind::Command(c) if c == "ls -la"));
        let r = search(&d, &[], ":lock", 8);
        assert!(r.iter().all(|i| i.source == Source::Action));
        let w = [WinInfo { index: 3, title: "Inbox".into(), app_id: "mail".into() }];
        let r = search(&d, &w, "@inb", 8);
        assert!(matches!(r[0].kind, Kind::Window(3)));
    }
}
