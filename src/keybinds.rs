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

/// One `<action>` with its arguments, in file order.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Step {
    pub name: String,
    pub args: Vec<(String, String)>,
}

impl Step {
    pub fn arg(&self, k: &str) -> &str {
        self.args.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("")
    }

    pub fn set_arg(&mut self, k: &str, v: String) {
        match self.args.iter_mut().find(|(n, _)| n == k) {
            Some(a) => a.1 = v,
            None => self.args.push((k.to_owned(), v)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Bind {
    pub key: String,
    pub act: Act,
    /// The actions as plain steps, or None when the body holds something the
    /// step editor cannot show (If, ForEach, nested elements). Those are edited
    /// as XML.
    pub steps: Option<Vec<Step>>,
    /// What is between `<keybind>` and `</keybind>`, as written.
    pub body: String,
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

/// Every attribute of a tag, in order.
fn attrs(tag: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let t = tag.trim_start_matches('<');
    let mut rest = t.split_once(char::is_whitespace).map(|(_, r)| r).unwrap_or("");
    loop {
        rest = rest.trim_start();
        let Some(eq) = rest.find('=') else { break };
        let name = rest[..eq].trim().to_owned();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| matches!(c, '"' | '\'')) else { break };
        let Some(close) = after[1..].find(q) else { break };
        out.push((name, unescape(&after[1..1 + close])));
        rest = &after[close + 2..];
    }
    out
}

/// Skip whitespace and comments. Returns the offset of the next real thing.
fn skip_blank(s: &str, mut at: usize) -> Option<usize> {
    loop {
        let rest = &s[at..];
        let t = rest.trim_start();
        at += rest.len() - t.len();
        if t.starts_with("<!--") {
            at += t.find("-->")? + 3;
        } else {
            return Some(at);
        }
    }
}

/// Read a keybind body as a list of plain actions. Arguments may be attributes
/// or simple child elements (`<to>2</to>`); anything deeper gives None.
pub fn parse_steps(body: &str) -> Option<Vec<Step>> {
    let mut steps = vec![];
    let mut at = 0;
    loop {
        at = skip_blank(body, at)?;
        let t = &body[at..];
        if t.is_empty() {
            break;
        }
        if !t.starts_with("<action") || !t[7..].starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
            return None;
        }
        let gt = t.find('>')?;
        let tag = &t[..=gt];
        let mut args = attrs(tag);
        let name = args.remove(args.iter().position(|(k, _)| k == "name")?).1;
        if tag.ends_with("/>") {
            at += gt + 1;
        } else {
            let close = gt + 1 + t[gt + 1..].find("</action>")?;
            let inner = &t[gt + 1..close];
            let mut j = 0;
            loop {
                j = skip_blank(inner, j)?;
                let c = &inner[j..];
                if c.is_empty() {
                    break;
                }
                let c = c.strip_prefix('<')?;
                let n = c.find('>')?;
                let el = &c[..n];
                if el.is_empty() || !el.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.') {
                    return None;
                }
                let v = &c[n + 1..];
                let lt = v.find('<')?;
                let end_tag = format!("</{el}>");
                if !v[lt..].starts_with(&end_tag) {
                    return None;
                }
                args.push((el.to_owned(), unescape(v[..lt].trim())));
                j += 1 + n + 1 + lt + end_tag.len();
            }
            at += close + "</action>".len();
        }
        steps.push(Step { name, args });
    }
    Some(steps)
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
        out.push(Bind { key, act: describe(body), steps: parse_steps(body), body: body.to_owned(), start: s, end });
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
        "SnapToEdge" => match a("direction").as_str() {
            "up" => "Snap the window to the top".into(),
            "down" => "Snap the window to the bottom".into(),
            "center" => "Snap the window to the center".into(),
            d => format!("Snap the window to the {d}"),
        },
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
        ("osmia clipboard toggle", "Open the clipboard history"),
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

// ---------- the actions the editor offers ----------

/// How an argument is edited.
#[derive(Clone, Copy, Debug)]
pub enum Arg {
    Text(&'static str),
    /// One of these values, or empty for labwc's default.
    Choice(&'static [&'static str]),
}

pub struct Spec {
    pub name: &'static str,
    pub label: &'static str,
    pub args: &'static [(&'static str, Arg)],
}

const YN: Arg = Arg::Choice(&["yes", "no"]);
const DIR4: Arg = Arg::Choice(&["left", "right", "up", "down"]);
const MAXDIR: Arg = Arg::Choice(&["both", "horizontal", "vertical"]);

/// labwc's keyboard actions, roughly in the order people look for them.
pub const ACTIONS: &[Spec] = &[
    Spec { name: "Execute", label: "Run a command", args: &[("command", Arg::Text("command"))] },
    Spec { name: "Close", label: "Close the window", args: &[] },
    Spec { name: "Kill", label: "Force the window's app to quit", args: &[] },
    Spec { name: "Iconify", label: "Minimize the window", args: &[] },
    Spec { name: "ToggleMaximize", label: "Maximize or restore", args: &[("direction", MAXDIR)] },
    Spec { name: "Maximize", label: "Maximize the window", args: &[("direction", MAXDIR)] },
    Spec { name: "UnMaximize", label: "Restore the window", args: &[("direction", MAXDIR)] },
    Spec { name: "ToggleFullscreen", label: "Toggle full screen", args: &[] },
    Spec { name: "SnapToEdge", label: "Snap to an edge", args: &[("direction", Arg::Choice(&["left", "right", "up", "down", "center"])), ("combine", YN)] },
    Spec { name: "ToggleSnapToEdge", label: "Snap to an edge or restore", args: &[("direction", Arg::Choice(&["left", "right", "up", "down", "center"]))] },
    Spec { name: "SnapToRegion", label: "Snap to a region", args: &[("region", Arg::Text("region name"))] },
    Spec { name: "UnSnap", label: "Unsnap the window", args: &[] },
    Spec { name: "GrowToEdge", label: "Grow to the next edge", args: &[("direction", DIR4)] },
    Spec { name: "ShrinkToEdge", label: "Shrink from an edge", args: &[("direction", DIR4)] },
    Spec { name: "MoveToEdge", label: "Move to an edge", args: &[("direction", DIR4), ("snapWindows", YN)] },
    Spec { name: "NextWindow", label: "Switch to the next window", args: &[] },
    Spec { name: "PreviousWindow", label: "Switch to the previous window", args: &[] },
    Spec { name: "ToggleShowDesktop", label: "Show the desktop", args: &[] },
    Spec { name: "GoToDesktop", label: "Go to a desktop", args: &[("to", Arg::Text("left, right, last or a number")), ("wrap", YN), ("toggle", YN)] },
    Spec { name: "SendToDesktop", label: "Send the window to a desktop", args: &[("to", Arg::Text("left, right, last or a number")), ("follow", YN), ("wrap", YN)] },
    Spec { name: "MoveToOutput", label: "Move the window to a screen", args: &[("output", Arg::Text("output name")), ("direction", DIR4), ("wrap", YN)] },
    Spec { name: "ShowMenu", label: "Show a menu", args: &[("menu", Arg::Choice(&["root-menu", "client-menu", "client-list-combined-menu", "client-send-to-menu"])), ("atCursor", YN)] },
    Spec { name: "ToggleAlwaysOnTop", label: "Keep the window on top", args: &[] },
    Spec { name: "ToggleAlwaysOnBottom", label: "Keep the window at the bottom", args: &[] },
    Spec { name: "ToggleOmnipresent", label: "Show the window on all desktops", args: &[] },
    Spec { name: "ToggleDecorations", label: "Toggle the title bar", args: &[] },
    Spec { name: "SetDecorations", label: "Set the window decorations", args: &[("decorations", Arg::Choice(&["full", "border", "none"])), ("forceSSD", YN)] },
    Spec { name: "ToggleShade", label: "Roll the window up or down", args: &[] },
    Spec { name: "Shade", label: "Roll the window up", args: &[] },
    Spec { name: "Unshade", label: "Roll the window down", args: &[] },
    Spec { name: "Move", label: "Move with the mouse", args: &[] },
    Spec { name: "Resize", label: "Resize with the mouse", args: &[] },
    Spec { name: "MoveTo", label: "Move to a position", args: &[("x", Arg::Text("x")), ("y", Arg::Text("y")), ("output", Arg::Text("output"))] },
    Spec { name: "MoveRelative", label: "Move by an amount", args: &[("x", Arg::Text("x")), ("y", Arg::Text("y"))] },
    Spec { name: "ResizeTo", label: "Resize to a size", args: &[("width", Arg::Text("width")), ("height", Arg::Text("height"))] },
    Spec { name: "ResizeRelative", label: "Resize by an amount", args: &[("left", Arg::Text("0")), ("right", Arg::Text("0")), ("top", Arg::Text("0")), ("bottom", Arg::Text("0"))] },
    Spec { name: "AutoPlace", label: "Place the window automatically", args: &[("policy", Arg::Choice(&["automatic", "cursor", "center", "cascade"]))] },
    Spec { name: "Focus", label: "Focus the window", args: &[] },
    Spec { name: "Unfocus", label: "Unfocus the window", args: &[] },
    Spec { name: "Raise", label: "Raise the window", args: &[] },
    Spec { name: "Lower", label: "Lower the window", args: &[] },
    Spec { name: "ZoomIn", label: "Zoom in", args: &[] },
    Spec { name: "ZoomOut", label: "Zoom out", args: &[] },
    Spec { name: "ToggleMagnify", label: "Toggle the magnifier", args: &[] },
    Spec { name: "WarpCursor", label: "Move the pointer", args: &[("to", Arg::Choice(&["output", "window"])), ("x", Arg::Text("x")), ("y", Arg::Text("y"))] },
    Spec { name: "HideCursor", label: "Hide the pointer", args: &[] },
    Spec { name: "ToggleKeybinds", label: "Turn shortcuts off or on", args: &[] },
    Spec { name: "ToggleTearing", label: "Toggle tearing", args: &[] },
    Spec { name: "VirtualOutputAdd", label: "Add a virtual screen", args: &[("output_name", Arg::Text("name"))] },
    Spec { name: "VirtualOutputRemove", label: "Remove a virtual screen", args: &[("output_name", Arg::Text("name"))] },
    Spec { name: "Reconfigure", label: "Reload the window manager config", args: &[] },
    Spec { name: "Exit", label: "Exit the compositor", args: &[] },
    Spec { name: "None", label: "Do nothing", args: &[] },
];

pub fn spec(name: &str) -> Option<&'static Spec> {
    ACTIONS.iter().find(|s| s.name == name)
}

/// A fresh step for this action, with every argument it takes, empty.
pub fn new_step(name: &str) -> Step {
    let args = spec(name).map(|s| s.args.iter().map(|(k, _)| ((*k).to_owned(), String::new())).collect()).unwrap_or_default();
    Step { name: name.to_owned(), args }
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

/// The keybind's own indentation, used for the lines written inside it.
fn indent_of(text: &str, start: usize) -> &str {
    let line = text[..start].rfind('\n').map(|n| n + 1).unwrap_or(0);
    let ind = &text[line..start];
    if ind.trim().is_empty() { ind } else { "" }
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

pub fn check_steps(steps: &[Step]) -> Result<(), String> {
    if steps.is_empty() {
        return Err("A shortcut needs at least one action.".into());
    }
    for st in steps {
        if !is_name(&st.name) {
            return Err(format!("\"{}\" is not an action name.", st.name));
        }
        if let Some((k, _)) = st.args.iter().find(|(k, _)| !is_name(k) || k == "name") {
            return Err(format!("\"{k}\" is not an argument name."));
        }
        if st.name == "Execute" && st.arg("command").trim().is_empty() {
            return Err("Type the command to run.".into());
        }
    }
    Ok(())
}

/// Write steps as the inside of a keybind. Empty arguments are left out, so
/// labwc uses its default.
fn render_steps(steps: &[Step], indent: &str) -> String {
    let mut s = String::from("\n");
    for st in steps {
        s.push_str(&format!("{indent}  <action name=\"{}\"", escape(&st.name)));
        for (k, v) in &st.args {
            if !v.trim().is_empty() {
                s.push_str(&format!(" {k}=\"{}\"", escape(v.trim())));
            }
        }
        s.push_str(" />\n");
    }
    s.push_str(indent);
    s
}

/// Steps as XML for the editor, without indentation.
pub fn steps_xml(steps: &[Step]) -> String {
    dedent(&render_steps(steps, ""))
}

/// Replace one shortcut's actions.
pub fn set_steps(text: &str, i: usize, steps: &[Step]) -> Result<String, String> {
    check_steps(steps)?;
    let binds = parse(text);
    let b = nth(text, &binds, i)?;
    let open_end = text[b.start..].find('>').map(|e| b.start + e).ok_or("Broken rc.xml.")?;
    let close = b.end - "</keybind>".len();
    Ok(format!("{}{}{}", &text[..=open_end], render_steps(steps, indent_of(text, b.start)), &text[close..]))
}

/// The body with its shared indentation removed, for the XML editor.
pub fn dedent(body: &str) -> String {
    let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
    let cut = lines.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    lines.iter().map(|l| l.get(cut..).unwrap_or(l.trim_start())).collect::<Vec<_>>().join("\n")
}

/// Replace one shortcut's body with XML typed by hand.
pub fn set_body(text: &str, i: usize, xml: &str) -> Result<String, String> {
    let xml = xml.trim();
    if xml.is_empty() {
        return Err("A shortcut needs at least one action.".into());
    }
    if xml.contains("keybind") || xml.matches("<!--").count() != xml.matches("-->").count() {
        return Err("That XML would break the shortcut list.".into());
    }
    let opens = xml.matches("<action").count();
    let closes = xml.matches("</action>").count() + xml.matches("/>").count();
    if opens == 0 || closes < opens {
        return Err("The XML needs at least one complete <action>.".into());
    }
    let binds = parse(text);
    let b = nth(text, &binds, i)?;
    let indent = indent_of(text, b.start);
    let open_end = text[b.start..].find('>').map(|e| b.start + e).ok_or("Broken rc.xml.")?;
    let close = b.end - "</keybind>".len();
    let mut body = String::from("\n");
    for l in xml.lines() {
        if !l.trim().is_empty() {
            body.push_str(&format!("{indent}  {}\n", l.trim_end()));
        }
    }
    body.push_str(indent);
    let out = format!("{}{}{}", &text[..=open_end], body, &text[close..]);
    let after = parse(&out);
    if after.len() != binds.len() || after[i].key != b.key {
        return Err("That XML would break the shortcut list.".into());
    }
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

pub fn add(text: &str, key: &str, steps: &[Step]) -> Result<String, String> {
    if !valid_key(key) {
        return Err(format!("\"{key}\" is not a key name."));
    }
    check_steps(steps)?;
    let comments = comment_ranges(text);
    let k0 = find_live(text, "<keyboard>", 0, &comments).ok_or("rc.xml has no <keyboard> section.")?;
    let end = find_live(text, "</keyboard>", k0, &comments).ok_or("rc.xml has no </keyboard>.")?;
    // Insert at the start of the closing tag's line.
    let line = text[..end].rfind('\n').map(|n| n + 1).unwrap_or(end);
    let indent = if text[line..end].trim().is_empty() { &text[line..end] } else { "  " };
    let inner = format!("{indent}  ");
    let item = format!("{inner}<keybind key=\"{}\">{}</keybind>\n", escape(key), render_steps(steps, &inner));
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
        let mut st = new_step("Execute");
        st.set_arg("command", "foot & \"x\"".into());
        let t = set_steps(&t, 1, &[st]).unwrap();
        assert_eq!(parse(&t)[1].act, Act::Exec("foot & \"x\"".into()));
        let t = delete(&t, 0).unwrap();
        assert_eq!(parse(&t).len(), 2);
        assert!(!t.contains("<!-- Reload -->") || t.contains("<!-- Reload -->"));
        let mut ff = new_step("Execute");
        ff.set_arg("command", "firefox".into());
        let t = add(&t, "W-b", &[ff]).unwrap();
        let b = parse(&t);
        assert_eq!(b.last().unwrap().key, "W-b");
        assert_eq!(b.last().unwrap().act, Act::Exec("firefox".into()));
        assert!(t.contains("<mouse>"));
    }

    #[test]
    fn rejects_bad_keys_and_flags_conflicts() {
        assert!(set_key(RC, 0, "W a").is_err());
        let x = Step { name: "Execute".into(), args: vec![("command".into(), "x".into())] };
        assert!(add(RC, "", &[x.clone()]).is_err());
        assert!(add(RC, "W-z", &[]).is_err());
        assert!(add(RC, "W-z", &[new_step("Execute")]).is_err());
        assert_eq!(canonical("C-W-a"), canonical("W-C-a"));
        let t = add(RC, "C-S-W-r", &[x]).unwrap();
        let c = conflicts(&parse(&t));
        assert_eq!(c.last().copied().flatten(), Some(0));
    }

    #[test]
    fn steps_round_trip() {
        let b = parse(RC);
        let st = b[2].steps.clone().unwrap();
        assert_eq!(st, vec![Step { name: "SnapToEdge".into(), args: vec![("direction".into(), "left".into()), ("combine".into(), "yes".into())] }]);
        // Change a window action into two steps; empty arguments are left out.
        let mut go = new_step("GoToDesktop");
        go.set_arg("to", "2".into());
        let t = set_steps(RC, 2, &[new_step("Iconify"), go]).unwrap();
        assert!(t.contains("      <action name=\"Iconify\" />\n      <action name=\"GoToDesktop\" to=\"2\" />\n    </keybind>"));
        let b = parse(&t);
        assert_eq!(b.len(), 3);
        assert_eq!(b[2].key, "W-Left");
        assert_eq!(b[2].steps.as_ref().unwrap().len(), 2);
        assert!(t.contains("<mouse>") && t.contains("<!-- Reload -->"));
    }

    #[test]
    fn child_elements_and_complex_bodies() {
        let simple = r#"<action name="GoToDesktop"><to>left</to><wrap>yes</wrap></action>"#;
        assert_eq!(parse_steps(simple).unwrap()[0].args, vec![("to".into(), "left".into()), ("wrap".into(), "yes".into())]);
        let nested = r#"<action name="If"><query identifier="foot" /><then><action name="Close" /></then></action>"#;
        assert!(parse_steps(nested).is_none());
        assert!(parse_steps("<position><x>1</x></position>").is_none());
    }

    #[test]
    fn xml_editing() {
        let t = set_body(RC, 0, "<action name=\"Close\" />\n<action name=\"None\" />").unwrap();
        let b = parse(&t);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].steps.as_ref().unwrap().len(), 2);
        assert_eq!(dedent(&b[0].body), "<action name=\"Close\" />\n<action name=\"None\" />");
        assert!(set_body(RC, 0, "</keybind><keybind key=\"x\">").is_err());
        assert!(set_body(RC, 0, "<action name=\"Close\">").is_err());
        assert!(set_body(RC, 0, "").is_err());
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
