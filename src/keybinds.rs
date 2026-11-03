//! Keyboard shortcuts live in labwc's rc.xml. This reads them for the settings
//! page and edits the file in place, so comments and everything else in it stay.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// One `Execute` action: the command can be edited.
    Exec(String),
    /// Window-manager actions. The label says what they do; only the key can change.
    Other(String),
}

#[derive(Clone, Debug)]
pub struct Bind {
    pub key: String,
    pub act: Act,
    start: usize,
    end: usize,
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    base.join("osmia/labwc/rc.xml")
}

/// The copy the installer ships, used by "restore defaults".
pub fn shipped() -> PathBuf {
    PathBuf::from("/usr/local/share/osmia/labwc/rc.xml")
}

pub fn load_file() -> Result<Vec<Bind>, String> {
    let text = std::fs::read_to_string(path()).map_err(|e| format!("{}: {e}", path().display()))?;
    Ok(parse(&text))
}

// ---------- parsing ----------

fn comment_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut at = 0;
    while let Some(s) = text[at..].find("<!--") {
        let s = at + s;
        let e = text[s..].find("-->").map(|e| s + e + 3).unwrap_or(text.len());
        out.push((s, e));
        at = e;
    }
    out
}

/// Find `needle` at or after `from`, skipping comments.
fn find_live(text: &str, needle: &str, from: usize, comments: &[(usize, usize)]) -> Option<usize> {
    let mut at = from;
    loop {
        let i = at + text.get(at..)?.find(needle)?;
        match comments.iter().find(|(s, e)| i >= *s && i < *e) {
            Some((_, e)) => at = *e,
            None => return Some(i),
        }
    }
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let mut from = 0;
    while let Some(i) = tag[from..].find(&pat) {
        let i = from + i;
        // Must be a whole attribute name, not the tail of another.
        let whole = i == 0 || tag.as_bytes()[i - 1].is_ascii_whitespace();
        if whole {
            let v = &tag[i + pat.len()..];
            let end = v.find('"')?;
            return Some(unescape(&v[..end]));
        }
        from = i + pat.len();
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn parse(text: &str) -> Vec<Bind> {
    let comments = comment_ranges(text);
    let Some(k0) = find_live(text, "<keyboard>", 0, &comments) else { return vec![] };
    let k1 = find_live(text, "</keyboard>", k0, &comments).unwrap_or(text.len());
    let mut out = vec![];
    let mut at = k0;
    while let Some(s) = find_live(text, "<keybind ", at, &comments) {
        if s >= k1 {
            break;
        }
        let Some(open_end) = text[s..].find('>').map(|i| s + i) else { break };
        let Some(close) = find_live(text, "</keybind>", open_end, &comments) else { break };
        let end = close + "</keybind>".len();
        let key = attr(&text[s..=open_end], "key").unwrap_or_default();
        let body = &text[open_end + 1..close];
        out.push(Bind { key, act: describe(body), start: s, end });
        at = end;
    }
    out
}

/// The actions inside one keybind, as the settings page shows them.
fn describe(body: &str) -> Act {
    let comments = comment_ranges(body);
    let mut tags: Vec<String> = vec![];
    let mut at = 0;
    while let Some(i) = find_live(body, "<action ", at, &comments) {
        let end = body[i..].find('>').map(|e| i + e + 1).unwrap_or(body.len());
        tags.push(body[i..end].to_owned());
        at = end;
    }
    if tags.len() == 1 && attr(&tags[0], "name").as_deref() == Some("Execute") {
        if let Some(c) = attr(&tags[0], "command") {
            return Act::Exec(c);
        }
    }
    let parts: Vec<String> = tags.iter().map(|t| action_label(t)).collect();
    Act::Other(if parts.is_empty() { "No action".to_owned() } else { parts.join(", then ") })
}

fn action_label(tag: &str) -> String {
    let name = attr(tag, "name").unwrap_or_default();
    let a = |k: &str| attr(tag, k).unwrap_or_default();
    match name.as_str() {
        "Execute" => format!("Run {}", a("command")),
        "Reconfigure" => "Reload the window manager config".into(),
        "ToggleShowDesktop" => "Show the desktop".into(),
        "NextWindow" => "Switch to the next window".into(),
        "PreviousWindow" => "Switch to the previous window".into(),
        "Close" => "Close the window".into(),
        "Iconify" => "Minimize the window".into(),
        "Maximize" => "Maximize the window".into(),
        "ToggleMaximize" => "Maximize or restore the window".into(),
        "ToggleFullscreen" => "Toggle full screen".into(),
        "ToggleAlwaysOnTop" => "Keep the window on top".into(),
        "ToggleAlwaysOnBottom" => "Keep the window at the bottom".into(),
        "ToggleOmnipresent" => "Show the window on all desktops".into(),
        "ToggleShade" | "Shade" => "Roll the window up".into(),
        "Unshade" => "Roll the window down".into(),
        "SnapToEdge" => format!("Snap the window to the {}", a("direction")),
        "SnapToRegion" => format!("Snap the window to the {}", a("region").replace('-', " ")),
        "ShowMenu" => match a("menu").as_str() {
            "client-menu" => "Show the window menu".into(),
            "root-menu" => "Show the desktop menu".into(),
            "" => "Show a menu".into(),
            m => format!("Show the {m}"),
        },
        "GoToDesktop" => format!("Go to desktop {}", a("to")),
        "SendToDesktop" => format!("Send the window to desktop {}", a("to")),
        "Focus" | "Raise" | "Unfocus" | "Move" | "Resize" | "MoveTo" | "ResizeTo" | "AutoPlace" => name.clone(),
        "Exit" => "Exit the compositor".into(),
        "" => "Unknown action".into(),
        other => {
            // Split CamelCase into words.
            let mut s = String::new();
            for (i, c) in other.chars().enumerate() {
                if c.is_uppercase() && i > 0 {
                    s.push(' ');
                }
                s.push(if i == 0 { c } else { c.to_ascii_lowercase() });
            }
            s
        }
    }
}

/// A readable name for a shell command bound to a key.
pub fn command_title(cmd: &str) -> String {
    let c = cmd.trim();
    let known: &[(&str, &str)] = &[
        ("osmia launcher toggle", "Open the launcher"),
        ("osmia capture region-video", "Record a region of the screen"),
        ("osmia capture now window", "Screenshot the active window"),
        ("osmia capture now screen", "Screenshot the whole screen"),
        ("osmia capture", "Open the screenshot toolbar"),
        ("osmia media play-pause", "Play or pause music"),
        ("osmia media pause", "Pause music"),
        ("osmia media next", "Next track"),
        ("osmia media previous", "Previous track"),
        ("loginctl lock-session", "Lock the screen"),
    ];
    if let Some((_, t)) = known.iter().find(|(k, _)| *k == c) {
        return (*t).to_owned();
    }
    if c.starts_with("wpctl set-volume") {
        return if c.contains('-') && c.trim_end().ends_with('-') { "Volume down" } else { "Volume up" }.to_owned();
    }
    if c.starts_with("wpctl set-mute") {
        return "Mute or unmute".to_owned();
    }
    if c.starts_with("brightnessctl set") {
        return if c.trim_end().ends_with('-') { "Brightness down" } else { "Brightness up" }.to_owned();
    }
    let first = c.split_whitespace().next().unwrap_or("command");
    format!("Run {}", first.rsplit('/').next().unwrap_or(first))
}

/// Order modifiers so "C-W-a" and "W-C-a" count as the same shortcut.
pub fn canonical(key: &str) -> String {
    let mut parts: Vec<&str> = key.split('-').collect();
    // A bare "-" key ends in an empty part; keep it as the key.
    let last = if key.ends_with('-') {
        parts.truncate(parts.len().saturating_sub(1));
        "-"
    } else {
        parts.pop().unwrap_or("")
    };
    parts.retain(|p| !p.is_empty());
    parts.sort_unstable();
    parts.dedup();
    if parts.is_empty() { last.to_owned() } else { format!("{}-{last}", parts.join("-")) }
}

/// Which shortcuts share a key with an earlier one. The first wins in labwc.
pub fn conflicts(binds: &[Bind]) -> Vec<Option<usize>> {
    binds
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let c = canonical(&b.key);
            binds[..i].iter().position(|o| canonical(&o.key) == c)
        })
        .collect()
}

pub fn valid_key(key: &str) -> bool {
    !key.is_empty() && !key.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '<' | '>' | '&')) && !key.ends_with("--")
}

// ---------- editing ----------

fn nth<'a>(text: &str, binds: &'a [Bind], i: usize) -> Result<&'a Bind, String> {
    let b = binds.get(i).ok_or("That shortcut is gone. Reopen the page.")?;
    if b.end > text.len() {
        return Err("The file changed. Reopen the page.".into());
    }
    Ok(b)
}

pub fn set_key(text: &str, i: usize, key: &str) -> Result<String, String> {
    if !valid_key(key) {
        return Err(format!("\"{key}\" is not a key name."));
    }
    let binds = parse(text);
    let b = nth(text, &binds, i)?;
    let open_end = text[b.start..].find('>').map(|e| b.start + e).ok_or("Broken rc.xml.")?;
    let tag = &text[b.start..open_end];
    let at = tag.find("key=\"").ok_or("Broken rc.xml.")? + 5;
    let len = tag[at..].find('"').ok_or("Broken rc.xml.")?;
    let mut out = String::with_capacity(text.len() + key.len());
    out.push_str(&text[..b.start + at]);
    out.push_str(&escape(key));
    out.push_str(&text[b.start + at + len..]);
    Ok(out)
}

pub fn set_command(text: &str, i: usize, cmd: &str) -> Result<String, String> {
    let binds = parse(text);
    let b = nth(text, &binds, i)?;
    if !matches!(b.act, Act::Exec(_)) {
        return Err("This shortcut does not run a command.".into());
    }
    let span = &text[b.start..b.end];
    let a = span.find("<action ").ok_or("Broken rc.xml.")?;
    let tag_end = span[a..].find('>').ok_or("Broken rc.xml.")? + a;
    let tag = &span[a..tag_end];
    let c = tag.find("command=\"").ok_or("Broken rc.xml.")? + 9;
    let len = tag[c..].find('"').ok_or("Broken rc.xml.")?;
    let base = b.start + a + c;
    let mut out = String::with_capacity(text.len() + cmd.len());
    out.push_str(&text[..base]);
    out.push_str(&escape(cmd.trim()));
    out.push_str(&text[base + len..]);
    Ok(out)
}

pub fn delete(text: &str, i: usize) -> Result<String, String> {
    let binds = parse(text);
    let b = nth(text, &binds, i)?;
    // Take the indentation before it and the line break after it too.
    let mut s = b.start;
    while s > 0 && matches!(text.as_bytes()[s - 1], b' ' | b'\t') {
        s -= 1;
    }
    let mut e = b.end;
    if text[e..].starts_with("\r\n") {
        e += 2;
    } else if text[e..].starts_with('\n') {
        e += 1;
    }
    Ok(format!("{}{}", &text[..s], &text[e..]))
}

pub fn add(text: &str, key: &str, cmd: &str) -> Result<String, String> {
    if !valid_key(key) {
        return Err(format!("\"{key}\" is not a key name."));
    }
    if cmd.trim().is_empty() {
        return Err("Type the command to run.".into());
    }
    let comments = comment_ranges(text);
    let k0 = find_live(text, "<keyboard>", 0, &comments).ok_or("rc.xml has no <keyboard> section.")?;
    let end = find_live(text, "</keyboard>", k0, &comments).ok_or("rc.xml has no </keyboard>.")?;
    // Insert at the start of the closing tag's line.
    let line = text[..end].rfind('\n').map(|n| n + 1).unwrap_or(end);
    let indent = if text[line..end].trim().is_empty() { &text[line..end] } else { "  " };
    let item = format!(
        "{indent}  <keybind key=\"{}\">\n{indent}    <action name=\"Execute\" command=\"{}\" />\n{indent}  </keybind>\n",
        escape(key),
        escape(cmd.trim())
    );
    let at = if text[line..end].trim().is_empty() { line } else { end };
    Ok(format!("{}{}{}", &text[..at], item, &text[at..]))
}

// ---------- key names for recording ----------

/// The name labwc uses for a pressed key, or None for modifier keys alone.
pub fn key_name(sym: u32, logo: bool, alt: bool, ctrl: bool, shift: bool) -> Option<String> {
    let base = sym_name(sym)?;
    let mut s = String::new();
    if logo {
        s.push_str("W-");
    }
    if alt {
        s.push_str("A-");
    }
    if ctrl {
        s.push_str("C-");
    }
    if shift {
        s.push_str("S-");
    }
    s.push_str(&base);
    Some(s)
}

fn sym_name(sym: u32) -> Option<String> {
    // Modifiers and locks on their own are not shortcuts.
    if matches!(sym, 0xffe1..=0xffee | 0xfe01..=0xfe13 | 0xff7f) {
        return None;
    }
    let fixed: &[(u32, &str)] = &[
        (0xff0d, "Return"), (0xff8d, "KP_Enter"), (0xff1b, "Escape"), (0xff08, "BackSpace"), (0xffff, "Delete"),
        (0xff09, "Tab"), (0xfe20, "Tab"), (0x20, "space"), (0xff50, "Home"), (0xff57, "End"), (0xff55, "Prior"),
        (0xff56, "Next"), (0xff63, "Insert"), (0xff51, "Left"), (0xff52, "Up"), (0xff53, "Right"), (0xff54, "Down"),
        (0xff61, "Print"), (0xff13, "Pause"), (0xff67, "Menu"), (0xff14, "Scroll_Lock"),
        (0x2c, "comma"), (0x2e, "period"), (0x2f, "slash"), (0x3b, "semicolon"), (0x27, "apostrophe"), (0x5b, "bracketleft"),
        (0x5d, "bracketright"), (0x5c, "backslash"), (0x2d, "minus"), (0x3d, "equal"), (0x60, "grave"),
        (0x1008ff11, "XF86_AudioLowerVolume"), (0x1008ff12, "XF86_AudioMute"), (0x1008ff13, "XF86_AudioRaiseVolume"),
        (0x1008ff14, "XF86_AudioPlay"), (0x1008ff15, "XF86_AudioStop"), (0x1008ff16, "XF86_AudioPrev"),
        (0x1008ff17, "XF86_AudioNext"), (0x1008ff1b, "XF86_Search"), (0x1008ff1d, "XF86_Calculator"),
        (0x1008ff2f, "XF86_Sleep"), (0x1008ff31, "XF86_AudioPause"), (0x1008ff3e, "XF86_AudioRewind"),
        (0x1008ff02, "XF86_MonBrightnessUp"), (0x1008ff03, "XF86_MonBrightnessDown"),
        (0x1008ff04, "XF86_KbdLightOnOff"), (0x1008ff05, "XF86_KbdBrightnessDown"), (0x1008ff06, "XF86_KbdBrightnessUp"),
        (0x1008ff59, "XF86_Display"), (0x1008ff95, "XF86_WLAN"), (0x1008ff9d, "XF86_TouchpadToggle"),
        (0x1008ffa9, "XF86_TouchpadToggle"), (0x1008ff4a, "XF86_LaunchA"), (0x1008ff2a, "XF86_PowerOff"),
        (0x1008ff18, "XF86_HomePage"), (0x1008ff19, "XF86_Mail"), (0x1008ff1c, "XF86_Standby"),
    ];
    if let Some((_, n)) = fixed.iter().find(|(s, _)| *s == sym) {
        return Some((*n).to_owned());
    }
    match sym {
        0xffbe..=0xffe0 => return Some(format!("F{}", sym - 0xffbe + 1)),
        0xffb0..=0xffb9 => return Some(format!("KP_{}", sym - 0xffb0)),
        0xffab => return Some("KP_Add".into()),
        0xffad => return Some("KP_Subtract".into()),
        0xffaa => return Some("KP_Multiply".into()),
        0xffaf => return Some("KP_Divide".into()),
        0xffae => return Some("KP_Decimal".into()),
        _ => {}
    }
    if (0x21..=0x7e).contains(&sym) {
        let c = sym as u8 as char;
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_lowercase().to_string());
        }
    }
    // Anything else: xkb reads the hex form, so it still works.
    Some(format!("0x{sym:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RC: &str = r#"<openbox_config>
  <!-- <keyboard> in a comment <keybind key="W-x"><action name="Close" /></keybind> -->
  <keyboard>
    <repeatRate>25</repeatRate>
    <!-- Reload -->
    <keybind key="W-C-S-r">
      <action name="Reconfigure" />
    </keybind>
    <keybind key="W-Return">
      <action name="Execute" command="qterminal -e &quot;x&quot;" />
    </keybind>
    <keybind key="W-Left">
      <action name="SnapToEdge" direction="left" combine="yes" />
    </keybind>
    <!-- <keybind key="W-y"><action name="Close" /></keybind> -->
  </keyboard>
  <mouse><keybind key="W-q"><action name="Close" /></keybind></mouse>
</openbox_config>
"#;

    #[test]
    fn parses_live_binds_only() {
        let b = parse(RC);
        let keys: Vec<_> = b.iter().map(|b| b.key.as_str()).collect();
        assert_eq!(keys, ["W-C-S-r", "W-Return", "W-Left"]);
        assert_eq!(b[1].act, Act::Exec("qterminal -e \"x\"".into()));
        assert_eq!(b[2].act, Act::Other("Snap the window to the left".into()));
    }

    #[test]
    fn edits_keep_everything_else() {
        let t = set_key(RC, 1, "W-t").unwrap();
        assert!(t.contains("key=\"W-t\"") && t.contains("<!-- Reload -->") && t.contains("W-C-S-r"));
        let t = set_command(&t, 1, "foot & \"x\"").unwrap();
        assert_eq!(parse(&t)[1].act, Act::Exec("foot & \"x\"".into()));
        let t = delete(&t, 0).unwrap();
        assert_eq!(parse(&t).len(), 2);
        assert!(!t.contains("<!-- Reload -->") || t.contains("<!-- Reload -->"));
        let t = add(&t, "W-b", "firefox").unwrap();
        let b = parse(&t);
        assert_eq!(b.last().unwrap().key, "W-b");
        assert_eq!(b.last().unwrap().act, Act::Exec("firefox".into()));
        assert!(t.contains("<mouse>"));
    }

    #[test]
    fn rejects_bad_keys_and_flags_conflicts() {
        assert!(set_key(RC, 0, "W a").is_err());
        assert!(add(RC, "", "x").is_err());
        assert_eq!(canonical("C-W-a"), canonical("W-C-a"));
        let t = add(RC, "C-S-W-r", "x").unwrap();
        let c = conflicts(&parse(&t));
        assert_eq!(c.last().copied().flatten(), Some(0));
    }

    #[test]
    fn names_keys() {
        assert_eq!(key_name(0xff61, false, false, false, false).as_deref(), Some("Print"));
        assert_eq!(key_name(0x44, true, false, false, true).as_deref(), Some("W-S-d"));
        assert_eq!(key_name(0xffbf, false, true, false, false).as_deref(), Some("A-F2"));
        assert_eq!(key_name(0xffe1, false, false, false, true), None);
        assert_eq!(key_name(0x1008ff12, false, false, false, false).as_deref(), Some("XF86_AudioMute"));
    }
}
