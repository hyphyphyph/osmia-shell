//! Default applications: which app opens a web link, a folder, a PDF and so on.
//! Reads the desktop files and the standard mimeapps.list files, and sets a
//! default with `xdg-mime default`, so every other program sees the change.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

/// One row on the page.
pub struct Category {
    pub label: &'static str,
    pub help: &'static str,
    /// Every type the choice is set for. The first one decides the candidates.
    pub types: &'static [&'static str],
}

pub const CATEGORIES: [Category; 8] = [
    Category { label: "Web browser", help: "Opens web links and HTML files.", types: &["x-scheme-handler/http", "x-scheme-handler/https", "text/html"] },
    Category { label: "Email", help: "Opens mailto links.", types: &["x-scheme-handler/mailto"] },
    Category { label: "File manager", help: "Opens folders.", types: &["inode/directory"] },
    Category { label: "Text editor", help: "Opens plain text files.", types: &["text/plain"] },
    Category { label: "Documents", help: "Opens PDF files.", types: &["application/pdf"] },
    Category { label: "Images", help: "Opens PNG, JPEG, GIF and WebP pictures.", types: &["image/png", "image/jpeg", "image/gif", "image/webp"] },
    Category { label: "Video", help: "Opens MP4, MKV and WebM videos.", types: &["video/mp4", "video/x-matroska", "video/webm"] },
    Category { label: "Music", help: "Opens MP3, FLAC, Ogg and WAV audio.", types: &["audio/mpeg", "audio/flac", "audio/ogg", "audio/x-wav"] },
];

#[derive(Clone, Debug, PartialEq)]
pub struct AppChoice {
    /// The desktop file name, such as firefox.desktop.
    pub id: String,
    pub name: String,
}

#[derive(Clone, Default)]
pub struct Row {
    /// Apps that say they open this kind of file, by name.
    pub apps: Vec<AppChoice>,
    pub current: Option<AppChoice>,
}

#[derive(Clone, Default)]
pub struct DefaultApps {
    /// One per entry of CATEGORIES.
    pub rows: Vec<Row>,
}

#[derive(Debug, Default)]
struct Entry {
    name: String,
    mime: Vec<String>,
}

fn parse_entry(text: &str) -> Option<Entry> {
    let mut in_entry = false;
    let (mut name, mut is_app, mut hidden) = (None, false, false);
    let mut mime = Vec::new();
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
            "Type" => is_app = v == "Application",
            "Hidden" => hidden |= v == "true",
            "MimeType" => mime = v.split(';').filter(|s| !s.is_empty()).map(str::to_owned).collect(),
            _ => {}
        }
    }
    if !is_app || hidden {
        return None;
    }
    Some(Entry { name: name?, mime })
}

/// The first id for `mime` in one section of a mimeapps-style file.
fn section_ids<'a>(text: &'a str, section: &str, mime: &str) -> Vec<&'a str> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == format!("[{section}]");
            continue;
        }
        if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == mime {
                    return v.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
                }
            }
        }
    }
    Vec::new()
}

/// The app that opens `mime`: the first installed one from Default Applications in
/// the files in order, then Added Associations, then the cache the system builds.
fn resolve(mime: &str, lists: &[String], caches: &[String], installed: &HashSet<String>) -> Option<String> {
    for section in ["Default Applications", "Added Associations"] {
        for t in lists {
            if let Some(id) = section_ids(t, section, mime).into_iter().find(|id| installed.contains(*id)) {
                return Some(id.to_owned());
            }
        }
    }
    caches.iter().find_map(|t| section_ids(t, "MIME Cache", mime).into_iter().find(|id| installed.contains(*id)).map(str::to_owned))
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn mimeapps_files() -> Vec<PathBuf> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    let mut v = Vec::new();
    let mut config = vec![config_home()];
    let cdirs = std::env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".into());
    config.extend(cdirs.split(':').filter(|d| !d.is_empty()).map(|d| Some(PathBuf::from(d))));
    for c in config.into_iter().flatten() {
        for d in desktop.split(':').filter(|d| !d.is_empty()) {
            v.push(c.join(format!("{d}-mimeapps.list")));
        }
        v.push(c.join("mimeapps.list"));
    }
    for d in crate::launch::data_dirs() {
        v.push(d.join("mimeapps.list"));
    }
    v
}

pub fn load() -> DefaultApps {
    let mut entries: HashMap<String, Entry> = HashMap::new();
    let dirs = crate::launch::data_dirs();
    for dir in &dirs {
        let Ok(rd) = fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }
            let id = path.file_name().unwrap().to_string_lossy().into_owned();
            if entries.contains_key(&id) {
                continue;
            }
            if let Some(en) = fs::read_to_string(&path).ok().and_then(|t| parse_entry(&t)) {
                entries.insert(id, en);
            }
        }
    }
    let installed: HashSet<String> = entries.keys().cloned().collect();
    let lists: Vec<String> = mimeapps_files().iter().filter_map(|p| fs::read_to_string(p).ok()).collect();
    let caches: Vec<String> = dirs.iter().filter_map(|d| fs::read_to_string(d.join("mimeinfo.cache")).ok()).collect();
    let rows = CATEGORIES
        .iter()
        .map(|c| {
            let main = c.types[0];
            let mut apps: Vec<AppChoice> = entries
                .iter()
                .filter(|(_, e)| e.mime.iter().any(|m| m == main))
                .map(|(id, e)| AppChoice { id: id.clone(), name: e.name.clone() })
                .collect();
            apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
            let current = resolve(main, &lists, &caches, &installed).and_then(|id| entries.get(&id).map(|e| AppChoice { id, name: e.name.clone() }));
            // A default that does not list the type is still shown and kept in the list.
            if let Some(cur) = &current {
                if !apps.iter().any(|a| a.id == cur.id) {
                    apps.insert(0, cur.clone());
                }
            }
            Row { apps, current }
        })
        .collect();
    DefaultApps { rows }
}

/// Make `id` the default for every type of the category.
pub fn set(cat: usize, id: &str) {
    let Some(c) = CATEGORIES.get(cat) else { return };
    let mut cmd = std::process::Command::new("xdg-mime");
    cmd.arg("default").arg(id).args(c.types).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    match cmd.spawn() {
        // Wait on a thread so the child does not stay a zombie.
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => log::warn!("cannot run xdg-mime: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn desktop_entry_needs_a_name_and_to_be_an_app() {
        let e = parse_entry("[Desktop Entry]\nType=Application\nName=Firefox\nMimeType=text/html;x-scheme-handler/http;\n").unwrap();
        assert_eq!(e.name, "Firefox");
        assert_eq!(e.mime, ["text/html", "x-scheme-handler/http"]);
        assert!(parse_entry("[Desktop Entry]\nType=Application\nHidden=true\nName=X\n").is_none());
        assert!(parse_entry("[Desktop Entry]\nType=Link\nName=X\n").is_none());
        assert!(parse_entry("[Desktop Entry]\nType=Application\n").is_none());
    }

    #[test]
    fn default_beats_added_and_skips_uninstalled() {
        let user = "[Added Associations]\ntext/html=b.desktop;\n[Default Applications]\ntext/html=gone.desktop;a.desktop;\n".to_owned();
        let got = resolve("text/html", &[user], &[], &installed(&["a.desktop", "b.desktop"]));
        assert_eq!(got.as_deref(), Some("a.desktop"));
    }

    #[test]
    fn earlier_files_win() {
        let first = "[Default Applications]\ntext/plain=x.desktop;\n".to_owned();
        let second = "[Default Applications]\ntext/plain=y.desktop;\n".to_owned();
        assert_eq!(resolve("text/plain", &[first, second], &[], &installed(&["x.desktop", "y.desktop"])).as_deref(), Some("x.desktop"));
    }

    #[test]
    fn added_then_cache_are_the_fallbacks() {
        let added = "[Added Associations]\napplication/pdf=p.desktop;\n".to_owned();
        let cache = "[MIME Cache]\ninode/directory=f.desktop;g.desktop;\n".to_owned();
        let ins = installed(&["p.desktop", "g.desktop"]);
        assert_eq!(resolve("application/pdf", &[added.clone()], &[cache.clone()], &ins).as_deref(), Some("p.desktop"));
        assert_eq!(resolve("inode/directory", &[added], &[cache], &ins).as_deref(), Some("g.desktop"));
        assert_eq!(resolve("video/mp4", &[], &[], &ins), None);
    }

    #[test]
    fn every_category_has_types() {
        assert!(CATEGORIES.iter().all(|c| !c.types.is_empty() && !c.label.is_empty()));
    }

    /// Compares with `xdg-mime query default` on this machine: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn matches_xdg_mime() {
        let d = load();
        for (c, r) in CATEGORIES.iter().zip(&d.rows) {
            let out = std::process::Command::new("xdg-mime").args(["query", "default", c.types[0]]).output().unwrap();
            let want = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let got = r.current.as_ref().map(|c| c.id.clone()).unwrap_or_default();
            println!("{:14} {:40} {} ({} candidates)", c.label, got, want, r.apps.len());
            assert_eq!(got, want, "{}", c.label);
        }
    }
}
