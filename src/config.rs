//! The one config file: ~/.config/osmia/config.toml.
//! A missing file means built-in defaults. A bad edit keeps the last good config.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub bar: BarCfg,
    pub theme: ThemeCfg,
    pub notifications: NotifCfg,
    pub launcher: LauncherCfg,
    pub session: SessionCfg,
    pub capture: CaptureCfg,
    /// Optional map from a notification app name to a window app id.
    pub alerts: HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct BarCfg {
    pub edge: String,
    pub height: u32,
    pub layout: Layout,
    pub clock_format: String,
    /// Shown before the time. Empty hides the date.
    pub date_format: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Layout {
    pub left: Vec<String>,
    pub center: Vec<String>,
    pub right: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ThemeCfg {
    pub accent: String,
    pub font_size: f32,
    pub corner_radius: u8,
    /// Icon theme name. Empty means the one the desktop uses.
    pub icon_theme: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct NotifCfg {
    pub corner: String,
    pub default_timeout_ms: u64,
    pub history_size: usize,
    /// Start with do-not-disturb on.
    pub dnd_on_start: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct LauncherCfg {
    pub terminal: String,
    pub width: u32,
    /// How many result rows show before the list scrolls.
    pub max_rows: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct SessionCfg {
    pub lock: String,
    pub logout: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct CaptureCfg {
    pub pictures_dir: String,
    pub videos_dir: String,
    /// Also put screenshots on the clipboard.
    pub copy: bool,
    pub cursor: bool,
    pub fps: u32,
    /// auto, hardware or software.
    pub encoder: String,
    /// small, balanced or high.
    pub quality: String,
    /// none, system, mic or both.
    pub audio: String,
    /// After a recording, also make a "none", "gif" or "webm" copy.
    pub export: String,
    /// Seconds to wait before capturing.
    pub delay: u32,
}

impl Default for CaptureCfg {
    fn default() -> Self {
        CaptureCfg {
            pictures_dir: "~/Pictures/Screenshots".into(),
            videos_dir: "~/Videos".into(),
            copy: true,
            cursor: false,
            fps: 30,
            encoder: "auto".into(),
            quality: "balanced".into(),
            audio: "none".into(),
            export: "none".into(),
            delay: 0,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bar: BarCfg::default(),
            theme: ThemeCfg::default(),
            notifications: NotifCfg::default(),
            launcher: LauncherCfg::default(),
            session: SessionCfg::default(),
            capture: CaptureCfg::default(),
            alerts: HashMap::new(),
        }
    }
}
impl Default for BarCfg {
    fn default() -> Self {
        BarCfg {
            edge: "top".into(),
            height: 30,
            layout: Layout::default(),
            clock_format: "%H:%M".into(),
            date_format: "%a %-d %b".into(),
        }
    }
}
impl Default for Layout {
    fn default() -> Self {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect();
        Layout {
            left: v(&["windows", "attention"]),
            center: v(&[]),
            right: v(&[
                "media", "tray", "|", "cpu", "ram", "|", "battery", "network", "volume", "brightness",
                "notifications", "|", "clock", "capture", "settings", "power",
            ]),
        }
    }
}
impl Default for ThemeCfg {
    fn default() -> Self {
        ThemeCfg {
            accent: "#f0a35e".into(),
            font_size: 12.0,
            corner_radius: 4,
            icon_theme: String::new(),
        }
    }
}
impl Default for NotifCfg {
    fn default() -> Self {
        NotifCfg {
            corner: "bottom-right".into(),
            default_timeout_ms: 5000,
            history_size: 100,
            dnd_on_start: false,
        }
    }
}
impl Default for LauncherCfg {
    fn default() -> Self {
        LauncherCfg {
            terminal: std::env::var("TERMINAL").unwrap_or_else(|_| "xterm".into()),
            width: 600,
            max_rows: 8,
        }
    }
}
impl Default for SessionCfg {
    fn default() -> Self {
        SessionCfg {
            lock: "loginctl lock-session".into(),
            logout: "loginctl terminate-session self".into(),
        }
    }
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("osmia").join("config.toml")
}

const KNOWN: &[(&str, &[&str])] = &[
    ("", &["bar", "theme", "notifications", "launcher", "session", "capture", "alerts"]),
    ("capture", &["pictures_dir", "videos_dir", "copy", "cursor", "fps", "encoder", "quality", "audio", "export", "delay"]),
    ("bar", &["edge", "height", "layout", "clock_format", "date_format"]),
    ("theme", &["accent", "font_size", "corner_radius", "icon_theme"]),
    ("notifications", &["corner", "default_timeout_ms", "history_size", "dnd_on_start"]),
    ("launcher", &["terminal", "width", "max_rows"]),
    ("session", &["lock", "logout"]),
];

fn unknown_keys(v: &toml::Value) -> Vec<String> {
    let mut out = Vec::new();
    for (table, keys) in KNOWN {
        let t = if table.is_empty() { Some(v) } else { v.get(*table) };
        if let Some(toml::Value::Table(t)) = t {
            for k in t.keys() {
                if !keys.contains(&k.as_str()) {
                    out.push(if table.is_empty() { k.clone() } else { format!("{table}.{k}") });
                }
            }
        }
    }
    out
}

pub struct Loaded {
    pub config: Config,
    /// Problems to tell the user about.
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

pub fn parse(text: &str) -> Result<(Config, Vec<String>), String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let value = toml::Value::Table(table);
    let warnings = unknown_keys(&value)
        .into_iter()
        .map(|k| format!("Unknown config key ignored: {k}"))
        .collect();
    let config: Config = value.try_into().map_err(|e: toml::de::Error| e.to_string())?;
    Ok((config, warnings))
}

pub fn load() -> Loaded {
    match std::fs::read_to_string(path()) {
        Ok(text) => match parse(&text) {
            Ok((config, warnings)) => Loaded { config, warnings, error: None },
            Err(e) => Loaded {
                config: Config::default(),
                warnings: vec![],
                error: Some(e),
            },
        },
        Err(_) => Loaded { config: Config::default(), warnings: vec![], error: None },
    }
}

pub fn mtime() -> Option<SystemTime> {
    std::fs::metadata(path()).and_then(|m| m.modified()).ok()
}

pub fn parse_color(s: &str) -> Option<egui::Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(egui::Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty() {
        let (c, w) = parse("").unwrap();
        assert_eq!(c.bar.height, 30);
        assert_eq!(c.notifications.history_size, 100);
        assert!(w.is_empty());
    }

    #[test]
    fn overrides_and_unknown_keys() {
        let (c, w) = parse("[bar]\nedge = \"bottom\"\nbogus = 1\n[theme]\naccent = \"#112233\"\n[nope]\nx = 1\n").unwrap();
        assert_eq!(c.bar.edge, "bottom");
        assert_eq!(c.theme.accent, "#112233");
        assert!(w.iter().any(|m| m.contains("bar.bogus")));
        assert!(w.iter().any(|m| m.contains("nope")));
    }

    #[test]
    fn bad_toml_is_an_error() {
        assert!(parse("[bar\nedge=").is_err());
        assert!(parse("[bar]\nheight = \"tall\"").is_err());
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#f0a35e"), Some(egui::Color32::from_rgb(0xf0, 0xa3, 0x5e)));
        assert_eq!(parse_color("nope"), None);
    }
}

/// Set one key, such as "bar.height", in config.toml. Keys and comments we do not
/// touch stay, except that comments are not kept when the file is rewritten.
pub fn set(key: &str, value: crate::ui::ConfValue) -> Result<(), String> {
    use crate::ui::ConfValue;
    let p = path();
    let mut table: toml::Table = match std::fs::read_to_string(&p) {
        Ok(text) => text.parse().map_err(|e: toml::de::Error| format!("config.toml has an error, not changed: {}", e.to_string().lines().next().unwrap_or("")))?,
        Err(_) => toml::Table::new(),
    };
    let parts: Vec<&str> = key.split('.').collect();
    let (last, parents) = parts.split_last().ok_or("empty key")?;
    let mut cur = &mut table;
    for part in parents {
        let entry = cur.entry((*part).to_owned()).or_insert_with(|| toml::Value::Table(toml::Table::new()));
        cur = entry.as_table_mut().ok_or_else(|| format!("{part} is not a table"))?;
    }
    cur.insert(
        (*last).to_owned(),
        match value {
            ConfValue::Str(s) => toml::Value::String(s),
            ConfValue::Int(n) => toml::Value::Integer(n),
            ConfValue::Float(f) => toml::Value::Float(f),
            ConfValue::Bool(b) => toml::Value::Boolean(b),
            ConfValue::List(l) => toml::Value::Array(l.into_iter().map(toml::Value::String).collect()),
        },
    );
    let text = toml::to_string_pretty(&table).map_err(|e| e.to_string())?;
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = p.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// Make sure the file exists so an editor can open it.
pub fn ensure_file() {
    let p = path();
    if !p.exists() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&p, "# osmia settings. See the README for every key.\n");
    }
}

#[cfg(test)]
mod set_tests {
    #[test]
    fn set_writes_and_keeps_other_keys() {
        let dir = std::env::temp_dir().join(format!("osmia-cfg-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("osmia")).unwrap();
        std::fs::write(dir.join("osmia/config.toml"), "[launcher]\nterminal = \"foot\"\n").unwrap();
        // SAFETY: single test in this process touches this variable.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
        super::set("bar.height", crate::ui::ConfValue::Int(36)).unwrap();
        super::set("theme.accent", crate::ui::ConfValue::Str("#6cb6ff".into())).unwrap();
        let text = std::fs::read_to_string(dir.join("osmia/config.toml")).unwrap();
        let (cfg, warnings) = super::parse(&text).unwrap();
        assert_eq!(cfg.bar.height, 36);
        assert_eq!(cfg.theme.accent, "#6cb6ff");
        assert_eq!(cfg.launcher.terminal, "foot");
        assert!(warnings.is_empty());
        super::set("bar.layout.right", crate::ui::ConfValue::List(vec!["clock".into(), "|".into(), "power".into()])).unwrap();
        super::set("notifications.dnd_on_start", crate::ui::ConfValue::Bool(true)).unwrap();
        super::set("bar.date_format", crate::ui::ConfValue::Str(String::new())).unwrap();
        let (cfg, warnings) = super::parse(&std::fs::read_to_string(dir.join("osmia/config.toml")).unwrap()).unwrap();
        assert_eq!(cfg.bar.layout.right, vec!["clock", "|", "power"]);
        assert!(cfg.notifications.dnd_on_start);
        assert_eq!(cfg.bar.date_format, "");
        assert!(warnings.is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
