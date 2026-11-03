//! Application icons from the system icon theme.
//!
//! A key such as "firefox", "org.mozilla.firefox.desktop", a window app id, a
//! notification's app name or a file path is turned into a picture. The search
//! follows the freedesktop icon theme rules: the chosen theme, the themes it
//! inherits from, hicolor, then a few other common themes. Names are also
//! matched through the .desktop files (file name, Name, StartupWMClass, Exec).
//! PNG and SVG are read. Results, including misses, are cached.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use crate::tray::Pix;

// ---------- settings ----------

static THEME_SETTING: RwLock<String> = RwLock::new(String::new());
static PIX_CACHE: Mutex<Option<HashMap<(String, u32), Option<Arc<Pix>>>>> = Mutex::new(None);
static CHAIN: Mutex<Option<Arc<Vec<Theme>>>> = Mutex::new(None);
static GENERATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Changes whenever the icon theme does, so that cached pictures can be dropped.
pub fn generation() -> u32 {
    GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Use this icon theme. An empty name means: find out what the desktop uses.
pub fn set_theme(name: &str) {
    let changed = {
        let mut cur = THEME_SETTING.write().unwrap();
        let changed = *cur != name;
        *cur = name.to_owned();
        changed
    };
    if changed {
        *CHAIN.lock().unwrap() = None;
        *PIX_CACHE.lock().unwrap() = None;
        GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Start reading directories in the background so the first icons appear quickly.
pub fn warm_up() {
    std::thread::spawn(|| {
        let _ = chain();
        let _ = desktop_map();
        let _ = find_file("application-x-executable", 32, None);
    });
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn data_dirs() -> Vec<PathBuf> {
    let mut v = vec![home().join(".local/share")];
    let xdg = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
    let xdg = if xdg.is_empty() { "/usr/local/share:/usr/share".to_owned() } else { xdg };
    v.extend(xdg.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    for extra in ["/var/lib/flatpak/exports/share", "/var/lib/snapd/desktop"] {
        v.push(PathBuf::from(extra));
    }
    v.push(home().join(".local/share/flatpak/exports/share"));
    v
}

fn icon_roots() -> Vec<PathBuf> {
    let mut v = vec![home().join(".icons")];
    v.extend(data_dirs().into_iter().map(|d| d.join("icons")));
    v.retain(|p| p.is_dir());
    v
}

/// What the desktop says the icon theme is.
fn detect_theme() -> Option<String> {
    let cfg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    let read_key = |file: PathBuf, key: &str| -> Option<String> {
        let text = std::fs::read_to_string(file).ok()?;
        text.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim().trim_matches('"').to_owned()).filter(|v| !v.is_empty())
        })
    };
    read_key(cfg.join("gtk-3.0/settings.ini"), "gtk-icon-theme-name")
        .or_else(|| read_key(cfg.join("gtk-4.0/settings.ini"), "gtk-icon-theme-name"))
        .or_else(|| read_key(cfg.join("lxqt/lxqt.conf"), "icon_theme"))
        .or_else(|| read_key(cfg.join("kdeglobals"), "Theme"))
        .or_else(|| {
            let out = std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", "icon-theme"]).output().ok()?;
            let s = String::from_utf8_lossy(&out.stdout).trim().trim_matches('\'').to_owned();
            (!s.is_empty()).then_some(s)
        })
}

// ---------- directory listings ----------

type Listing = Arc<HashMap<String, u8>>;
const PNG: u8 = 1;
const SVG: u8 = 2;

static LISTINGS: Mutex<Option<HashMap<PathBuf, Listing>>> = Mutex::new(None);

/// The icon files in one directory: name without extension to the kinds present.
fn listing(dir: &Path) -> Listing {
    {
        let g = LISTINGS.lock().unwrap();
        if let Some(l) = g.as_ref().and_then(|m| m.get(dir)) {
            return l.clone();
        }
    }
    let mut map: HashMap<String, u8> = HashMap::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some((stem, ext)) = name.rsplit_once('.') {
                let bit = match ext {
                    "png" => PNG,
                    "svg" => SVG,
                    _ => continue,
                };
                *map.entry(stem.to_owned()).or_insert(0) |= bit;
            }
        }
    }
    let l = Arc::new(map);
    LISTINGS.lock().unwrap().get_or_insert_with(HashMap::new).insert(dir.to_owned(), l.clone());
    l
}

// ---------- themes ----------

#[derive(Clone, Debug)]
struct DirSpec {
    rel: String,
    size: i32,
    min: i32,
    max: i32,
    threshold: i32,
    kind: u8, // 0 fixed, 1 scalable, 2 threshold
}

impl DirSpec {
    fn distance(&self, want: i32) -> i32 {
        match self.kind {
            0 => (self.size - want).abs(),
            1 => {
                if want < self.min {
                    self.min - want
                } else if want > self.max {
                    want - self.max
                } else {
                    0
                }
            }
            _ => {
                if want < self.size - self.threshold {
                    self.size - self.threshold - want
                } else if want > self.size + self.threshold {
                    want - self.size - self.threshold
                } else {
                    0
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Theme {
    name: String,
    dirs: Vec<DirSpec>,
}

fn parse_index(text: &str) -> (Vec<String>, Vec<DirSpec>) {
    let mut sections: Vec<(String, HashMap<String, String>)> = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') && l.ends_with(']') {
            sections.push((l[1..l.len() - 1].to_owned(), HashMap::new()));
        } else if let (Some((k, v)), Some(s)) = (l.split_once('='), sections.last_mut()) {
            s.1.insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    let head = sections.iter().find(|s| s.0 == "Icon Theme").map(|s| s.1.clone()).unwrap_or_default();
    let list = |k: &str| head.get(k).map(|v| v.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect::<Vec<_>>()).unwrap_or_default();
    let inherits = list("Inherits");
    let mut dirs = Vec::new();
    for rel in list("Directories") {
        let Some((_, s)) = sections.iter().find(|s| s.0 == rel) else { continue };
        let num = |k: &str| s.get(k).and_then(|v| v.parse::<i32>().ok());
        if num("Scale").unwrap_or(1) != 1 {
            continue;
        }
        let size = num("Size").unwrap_or(0);
        let kind = match s.get("Type").map(String::as_str) {
            Some("Fixed") => 0,
            Some("Scalable") => 1,
            _ => 2,
        };
        dirs.push(DirSpec {
            rel,
            size,
            min: num("MinSize").unwrap_or(size),
            max: num("MaxSize").unwrap_or(size),
            threshold: num("Threshold").unwrap_or(2),
            kind,
        });
    }
    (inherits, dirs)
}

fn load_theme(name: &str, roots: &[PathBuf]) -> Option<(Theme, Vec<String>)> {
    let mut dirs: Vec<DirSpec> = Vec::new();
    let mut inherits: Vec<String> = Vec::new();
    let mut found = false;
    for r in roots {
        if let Ok(text) = std::fs::read_to_string(r.join(name).join("index.theme")) {
            found = true;
            let (inh, d) = parse_index(&text);
            for i in inh {
                if !inherits.contains(&i) {
                    inherits.push(i);
                }
            }
            for spec in d {
                if !dirs.iter().any(|x| x.rel == spec.rel) {
                    dirs.push(spec);
                }
            }
        }
    }
    found.then(|| (Theme { name: name.to_owned(), dirs }, inherits))
}

/// The chosen theme, what it inherits, hicolor, then other common themes.
fn chain() -> Arc<Vec<Theme>> {
    if let Some(c) = CHAIN.lock().unwrap().as_ref() {
        return c.clone();
    }
    let roots = icon_roots();
    let setting = THEME_SETTING.read().unwrap().clone();
    let first = if setting.is_empty() { detect_theme() } else { Some(setting) };
    let mut queue: Vec<String> = first.into_iter().collect();
    let mut out: Vec<Theme> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut i = 0;
    let mut fallbacks_added = false;
    loop {
        if i >= queue.len() {
            if fallbacks_added {
                break;
            }
            fallbacks_added = true;
            for f in ["hicolor", "Adwaita", "breeze", "Papirus", "elementary", "Cosmic", "oxygen", "AdwaitaLegacy"] {
                queue.push(f.to_owned());
            }
            continue;
        }
        let name = queue[i].clone();
        i += 1;
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some((t, inh)) = load_theme(&name, &roots) {
            // Inherited themes come right after their child.
            let at = (i).min(queue.len());
            for (k, n) in inh.into_iter().enumerate() {
                queue.insert(at + k, n);
            }
            out.push(t);
        }
    }
    let c = Arc::new(out);
    *CHAIN.lock().unwrap() = Some(c.clone());
    c
}

// ---------- finding a file ----------

#[derive(Clone, Debug)]
enum Found {
    Png(PathBuf),
    Svg(PathBuf),
}

fn kind_of(path: &Path) -> Option<Found> {
    match path.extension()?.to_str()? {
        "png" => Some(Found::Png(path.to_owned())),
        "svg" => Some(Found::Svg(path.to_owned())),
        _ => None,
    }
}

fn pick_in(dir: &Path, stem: &str) -> Option<Found> {
    let bits = *listing(dir).get(stem)?;
    if bits & SVG != 0 {
        Some(Found::Svg(dir.join(format!("{stem}.svg"))))
    } else if bits & PNG != 0 {
        Some(Found::Png(dir.join(format!("{stem}.png"))))
    } else {
        None
    }
}

/// The best file for `name` inside one theme: the directory whose size is closest.
fn find_in_theme(theme: &Theme, roots: &[PathBuf], name: &str, want: i32) -> Option<Found> {
    let mut best: Option<(i32, bool, Found)> = None;
    for root in roots {
        for d in &theme.dirs {
            let dir = root.join(&theme.name).join(&d.rel);
            if let Some(f) = pick_in(&dir, name) {
                let dist = d.distance(want);
                let svg = matches!(f, Found::Svg(_));
                let better = match &best {
                    None => true,
                    Some((bd, bsvg, _)) => dist < *bd || (dist == *bd && svg && !*bsvg),
                };
                if better {
                    best = Some((dist, svg, f));
                }
            }
        }
    }
    best.map(|(_, _, f)| f)
}

/// Look for any of the names, trying the themes in order. The theme comes first:
/// a name that exists only in a lower theme loses to any name in the chosen one.
fn find_names(names: &[String], want: i32, extra_root: Option<&Path>) -> Option<Found> {
    let mut roots = icon_roots();
    if let Some(e) = extra_root {
        roots.insert(0, e.to_owned());
    }
    let names: Vec<&String> = names.iter().filter(|n| !n.is_empty()).collect();
    // Absolute paths need no theme.
    for n in &names {
        if n.starts_with('/') {
            if let Some(f) = kind_of(Path::new(n.as_str())).filter(|f| matches!(f, Found::Png(p) | Found::Svg(p) if p.is_file())) {
                return Some(f);
            }
        }
    }
    let names: Vec<&String> = names.into_iter().filter(|n| !n.starts_with('/') && !n.contains('/')).collect();
    // Loose files in the extra root, such as a tray app's own icon folder.
    if let Some(e) = extra_root {
        for n in &names {
            if let Some(f) = pick_in(e, n) {
                return Some(f);
            }
        }
    }
    for theme in chain().iter() {
        for n in &names {
            if let Some(f) = find_in_theme(theme, &roots, n, want) {
                return Some(f);
            }
        }
    }
    // Icons dropped straight into an icon root or the pixmaps directory.
    let mut loose: Vec<PathBuf> = roots.clone();
    loose.push(PathBuf::from("/usr/share/pixmaps"));
    loose.push(home().join(".local/share/pixmaps"));
    for dir in loose {
        for n in &names {
            if let Some(f) = pick_in(&dir, n) {
                return Some(f);
            }
        }
    }
    None
}

fn find_file(name: &str, want: i32, extra_root: Option<&Path>) -> Option<Found> {
    find_names(&[name.to_owned()], want, extra_root)
}

// ---------- desktop files ----------

/// Lowercase names that point at a desktop entry, mapped to its Icon value.
fn desktop_map() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: HashMap<String, String> = HashMap::new();
        // Earlier directories win, so the user's own entries come first.
        for d in data_dirs() {
            let apps = d.join("applications");
            let mut stack = vec![apps.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(rd) = std::fs::read_dir(&dir) else { continue };
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                        continue;
                    }
                    if p.extension().and_then(|x| x.to_str()) != Some("desktop") {
                        continue;
                    }
                    let Ok(text) = std::fs::read_to_string(&p) else { continue };
                    let (mut icon, mut name, mut wm, mut exec) = (None, None, None, None);
                    let mut in_main = false;
                    for l in text.lines() {
                        let l = l.trim();
                        if l.starts_with('[') {
                            in_main = l == "[Desktop Entry]";
                            continue;
                        }
                        if !in_main {
                            continue;
                        }
                        if let Some(v) = l.strip_prefix("Icon=") {
                            icon.get_or_insert(v.trim().to_owned());
                        } else if let Some(v) = l.strip_prefix("Name=") {
                            name.get_or_insert(v.trim().to_owned());
                        } else if let Some(v) = l.strip_prefix("StartupWMClass=") {
                            wm.get_or_insert(v.trim().to_owned());
                        } else if let Some(v) = l.strip_prefix("Exec=") {
                            exec.get_or_insert(v.trim().to_owned());
                        }
                    }
                    let Some(icon) = icon.filter(|i| !i.is_empty()) else { continue };
                    let rel = p.strip_prefix(&apps).unwrap_or(&p).to_string_lossy().replace('/', "-");
                    let stem = rel.trim_end_matches(".desktop").to_lowercase();
                    let mut keys = vec![stem.clone(), format!("{stem}.desktop")];
                    keys.extend(name.map(|n| n.to_lowercase()));
                    keys.extend(wm.map(|n| n.to_lowercase()));
                    if let Some(x) = exec {
                        let cmd = x.split_whitespace().find(|t| !t.starts_with('%') && !t.contains('=') && *t != "env").unwrap_or("");
                        let base = cmd.rsplit('/').next().unwrap_or("").to_lowercase();
                        if !base.is_empty() {
                            keys.push(base);
                        }
                    }
                    for k in keys {
                        map.entry(k).or_insert_with(|| icon.clone());
                    }
                }
            }
        }
        map
    })
}

// ---------- resolving a key ----------

/// Possible icon names for one candidate string.
fn names_for(c: &str) -> Vec<String> {
    let c = c.trim().trim_start_matches("file://");
    let mut v: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !s.is_empty() && !v.contains(&s) {
            v.push(s);
        }
    };
    push(c.to_owned());
    let stripped = c.strip_suffix(".desktop").unwrap_or(c);
    push(stripped.to_owned());
    push(stripped.to_lowercase());
    push(stripped.to_lowercase().replace([' ', '_'], "-"));
    if let Some(last) = stripped.rsplit('.').next() {
        push(last.to_lowercase());
    }
    v
}

fn find_for_key(key: &str, want: i32, extra_root: Option<&Path>) -> Option<Found> {
    let cands: Vec<&str> = key.split('\n').map(str::trim).filter(|s| !s.is_empty()).collect();
    // Names in order of how much we trust them: what the app called its icon,
    // then what its desktop file says, then guesses from the app's name.
    let mut names: Vec<String> = Vec::new();
    let mut add = |n: String| {
        if !n.is_empty() && !names.contains(&n) {
            names.push(n);
        }
    };
    for c in &cands {
        add(c.trim_start_matches("file://").to_owned());
    }
    let map = desktop_map();
    for c in &cands {
        for n in names_for(c) {
            if let Some(icon) = map.get(&n.to_lowercase()) {
                add(icon.clone());
            }
        }
    }
    for c in &cands {
        for n in names_for(c) {
            add(n);
        }
    }
    find_names(&names, want, extra_root)
}

// ---------- decoding ----------

fn decode(f: &Found, px: u32) -> Option<Pix> {
    match f {
        Found::Png(p) => crate::tray::load_png(p),
        Found::Svg(p) => {
            let data = std::fs::read(p).ok()?;
            let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
            let size = tree.size();
            let (sw, sh) = (size.width(), size.height());
            if sw <= 0.0 || sh <= 0.0 {
                return None;
            }
            let scale = px as f32 / sw.max(sh);
            let (w, h) = (((sw * scale).round() as u32).max(1), ((sh * scale).round() as u32).max(1));
            let mut pm = resvg::tiny_skia::Pixmap::new(w, h)?;
            resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for c in pm.pixels() {
                let u = c.demultiply();
                rgba.extend_from_slice(&[u.red(), u.green(), u.blue(), u.alpha()]);
            }
            Some(Pix { w: w as usize, h: h as usize, rgba })
        }
    }
}

/// A picture for the key at about `px` pixels. None when nothing matches.
pub fn load(key: &str, px: u32, extra_root: Option<&Path>) -> Option<Arc<Pix>> {
    let cache_key = (match extra_root {
        Some(e) => format!("{}\u{1}{key}", e.display()),
        None => key.to_owned(),
    }, px);
    if let Some(hit) = PIX_CACHE.lock().unwrap().as_ref().and_then(|m| m.get(&cache_key)) {
        return hit.clone();
    }
    let pix = find_for_key(key, px as i32, extra_root).and_then(|f| decode(&f, px)).map(Arc::new);
    PIX_CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(cache_key, pix.clone());
    pix
}

/// An egui texture for the key, sized for `size` logical points on this screen.
pub fn texture(ctx: &egui::Context, key: &str, size: f32) -> Option<egui::TextureHandle> {
    if key.trim().is_empty() {
        return None;
    }
    let want = (size * ctx.pixels_per_point()).ceil() as u32;
    let px = [16u32, 24, 32, 48, 64, 96, 128].into_iter().find(|b| *b >= want).unwrap_or(128);
    let id = crate::ui::widgets::id(("appicon", key, px, generation()));
    if let Some(t) = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return t;
    }
    let tex = load(key, px, None).map(|pix| {
        let img = egui::ColorImage::from_rgba_unmultiplied([pix.w, pix.h], &pix.rgba);
        ctx.load_texture(format!("appicon-{key}-{px}"), img, egui::TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_common_icons() {
        let t = std::time::Instant::now();
        let mut miss = vec![];
        for k in ["firefox", "spotify-client", "spotify", "nordvpn", "ghostty", "com.mitchellh.ghostty", "Ghostty", "org.kde.dolphin", "code", "discord", "steam", "nm-applet", "blueman", "vlc", "no-such-app-xyz", "pavucontrol", "chromium", "Brave-browser"] {
            let f = super::find_for_key(k, 32, None);
            eprintln!("{k:28} {f:?}");
            if f.is_none() { miss.push(k); }
        }
        eprintln!("chain: {:?}", super::chain().iter().map(|t| t.name.clone()).collect::<Vec<_>>());
        eprintln!("took {:?}; missed {miss:?}", t.elapsed());
        let p = super::load("firefox", 32, None);
        eprintln!("decoded: {:?}", p.map(|p| (p.w, p.h)));
    }
}

/// Names of the installed icon themes, for a picker. Cursor-only themes are left out.
pub fn installed_themes() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for root in icon_roots() {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(text) = std::fs::read_to_string(e.path().join("index.theme")) else { continue };
            if text.contains("Directories=") && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out.sort_by_key(|n| n.to_lowercase());
    out
}

#[cfg(test)]
mod theme_choice_tests {
    #[test]
    fn chosen_theme_wins_over_other_themes() {
        // Papirus has both icons; the choice decides which file is used.
        super::set_theme("Papirus");
        let f = super::find_for_key("spotify", 32, None);
        eprintln!("{f:?}");
        assert!(matches!(&f, Some(super::Found::Svg(p) | super::Found::Png(p)) if p.to_string_lossy().contains("/Papirus/")));
        super::set_theme("breeze");
        let g = super::find_for_key("folder", 32, None);
        eprintln!("{g:?}");
        assert!(matches!(&g, Some(super::Found::Svg(p) | super::Found::Png(p)) if p.to_string_lossy().contains("/breeze")));
    }
}
