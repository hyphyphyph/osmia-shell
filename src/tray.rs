//! System tray: a StatusNotifierItem host (and watcher, when no one else is one).
//! Menus use com.canonical.dbusmenu and are drawn by the shell.

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use egui::{vec2, Sense, Ui};
use futures_lite::StreamExt;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use zbus::{OwnedVariant as OwnedValue, Variant as Value};
use zbus::{message::Type, Connection, MatchRule, MessageStream};

use crate::ui::Action;

#[derive(Clone, Debug)]
pub struct Pix {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub bus: String,
    pub path: String,
    pub title: String,
    pub icon: Option<Pix>,
    pub menu: Option<String>,
    pub version: u32,
    /// Passive, Active or NeedsAttention.
    pub status: String,
    /// True when the app says its click only means "show the menu".
    pub item_is_menu: bool,
    pub tip_title: String,
    pub tip_text: String,
    /// Id and title, joined, for matching the app to its window and desktop entry.
    pub app_key: String,
}

#[derive(Clone, Debug, Default)]
pub struct MenuNode {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub checked: Option<bool>,
    pub children: Vec<MenuNode>,
}

pub enum Msg {
    Upsert(Item),
    Remove(String),
    Menu(String, Vec<MenuNode>),
    /// The app refused the Activate call.
    ActivateFailed(String),
}

pub enum Cmd {
    Activate(String, i32, i32),
    OpenMenu(String),
    /// Ask the app to show its own menu.
    NativeMenu(String),
    /// The icon theme changed: read every item again.
    Refresh,
    ClickMenu(String, i32),
}

type Items = Arc<Mutex<HashMap<String, Item>>>;

struct Watcher {
    items: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &self,
        service: String,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let (bus, path) = split_service(&service, &sender);
        let id = format!("{bus}{path}");
        {
            let mut v = self.items.lock().unwrap();
            if !v.contains(&id) {
                v.push(id.clone());
            }
        }
        let _ = Self::status_notifier_item_registered(&emitter, &id).await;
    }

    fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(emitter: &zbus::object_server::SignalEmitter<'_>, service: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(emitter: &zbus::object_server::SignalEmitter<'_>, service: &str) -> zbus::Result<()>;
}

fn split_service(service: &str, sender: &str) -> (String, String) {
    if service.starts_with('/') {
        (sender.to_owned(), service.to_owned())
    } else if let Some(i) = service.find('/') {
        (service[..i].to_owned(), service[i..].to_owned())
    } else {
        (service.to_owned(), "/StatusNotifierItem".to_owned())
    }
}

/// Start the tray. Returns a sender for commands.
pub fn start(tx: Sender<Msg>) -> mpsc::Sender<Cmd> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
    let items: Items = Arc::new(Mutex::new(HashMap::new()));
    let conn_cell: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
    {
        let (items, tx, conn_cell) = (items.clone(), tx.clone(), conn_cell.clone());
        std::thread::spawn(move || {
            if let Err(e) = pollster::block_on(run(tx, items, conn_cell)) {
                log::warn!("tray is off: {e}");
            }
        });
    }
    std::thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            let conn = conn_cell.lock().unwrap().clone();
            let Some(conn) = conn else { continue };
            pollster::block_on(handle_cmd(&conn, &items, &tx, cmd));
        }
    });
    cmd_tx
}

async fn run(tx: Sender<Msg>, items: Items, conn_cell: Arc<Mutex<Option<Connection>>>) -> zbus::Result<()> {
    let conn = Connection::session().await?;
    let list: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let _ = conn.object_server().at("/StatusNotifierWatcher", Watcher { items: list }).await;
    let owns = conn.request_name("org.kde.StatusNotifierWatcher").await.is_ok();
    log::info!("tray watcher: {}", if owns { "this shell" } else { "another program" });
    let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
    let _ = conn.request_name(host.as_str()).await;
    *conn_cell.lock().unwrap() = Some(conn.clone());

    let watcher = zbus::Proxy::new(&conn, "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher").await?;
    let _ = watcher.call_method("RegisterStatusNotifierHost", &(host.as_str(),)).await;

    // Listen first, then read the list, so no item is missed.
    let rule_w = MatchRule::builder().msg_type(Type::Signal).interface("org.kde.StatusNotifierWatcher").build()?;
    let rule_i = MatchRule::builder().msg_type(Type::Signal).interface("org.kde.StatusNotifierItem").build()?;
    let rule_n = MatchRule::builder().msg_type(Type::Signal).interface("org.freedesktop.DBus").member("NameOwnerChanged").build()?;
    let s1 = MessageStream::for_match_rule(rule_w, &conn, None).await?;
    let s2 = MessageStream::for_match_rule(rule_i, &conn, None).await?;
    let s3 = MessageStream::for_match_rule(rule_n, &conn, None).await?;
    let mut merged = s1.or(s2).or(s3);

    let existing: Vec<String> = watcher.get_property("RegisteredStatusNotifierItems").await.unwrap_or_default();
    for id in existing {
        add_item(&conn, &items, &tx, &id).await;
    }

    while let Some(Ok(msg)) = merged.next().await {
        let hdr = msg.header();
        let member = hdr.member().map(|m| m.as_str().to_owned()).unwrap_or_default();
        match member.as_str() {
            "StatusNotifierItemRegistered" => {
                let body = msg.body();
                if let Ok(id) = body.deserialize::<&str>() {
                    let id = id.to_owned();
                    add_item(&conn, &items, &tx, &id).await;
                }
            }
            "StatusNotifierItemUnregistered" => {
                let body = msg.body();
                if let Ok(id) = body.deserialize::<&str>() {
                    remove_item(&items, &tx, id);
                }
            }
            "NameOwnerChanged" => {
                let body = msg.body();
                if let Ok((name, _old, new)) = body.deserialize::<(&str, &str, &str)>() {
                    let name = name.to_owned();
                    if new.is_empty() {
                        let gone: Vec<String> = items.lock().unwrap().values().filter(|i| i.bus == name).map(|i| i.id.clone()).collect();
                        for id in gone {
                            remove_item(&items, &tx, &id);
                        }
                    }
                }
            }
            "NewIcon" | "NewTitle" | "NewStatus" | "NewAttentionIcon" | "NewOverlayIcon" | "NewToolTip" => {
                let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
                let found = {
                    let map = items.lock().unwrap();
                    map.values().find(|i| i.bus == sender || owner_matches(i, &sender)).map(|i| i.id.clone())
                };
                if let Some(id) = found {
                    add_item(&conn, &items, &tx, &id).await;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn owner_matches(_item: &Item, _sender: &str) -> bool {
    false
}

fn remove_item(items: &Items, tx: &Sender<Msg>, id: &str) {
    if items.lock().unwrap().remove(id).is_some() {
        let _ = tx.send(Msg::Remove(id.to_owned()));
    }
}

async fn add_item(conn: &Connection, items: &Items, tx: &Sender<Msg>, id: &str) {
    let (bus, path) = split_service(id, "");
    if bus.is_empty() {
        return;
    }
    // Items usually register by well-known name. Signals come from the unique name.
    let unique = match (zbus::fdo::DBusProxy::new(conn).await, zbus::names::BusName::try_from(bus.as_str())) {
        (Ok(d), Ok(name)) => d.get_name_owner(name).await.map(|o| o.to_string()).unwrap_or_else(|_| bus.clone()),
        _ => bus.clone(),
    };
    let Ok(p) = zbus::Proxy::new(conn, bus.clone(), path.clone(), "org.kde.StatusNotifierItem").await else {
        return;
    };
    let title: String = p.get_property("Title").await.unwrap_or_default();
    let item_id: String = p.get_property("Id").await.unwrap_or_default();
    let icon_name: String = p.get_property("IconName").await.unwrap_or_default();
    let theme_path: String = p.get_property("IconThemePath").await.unwrap_or_default();
    let pixmaps: Vec<(i32, i32, Vec<u8>)> = p.get_property("IconPixmap").await.unwrap_or_default();
    let menu: Option<String> = p
        .get_property::<zbus::OwnedObjectPath>("Menu")
        .await
        .ok()
        .map(|m| m.to_string())
        .filter(|m| m != "/");
    let item_is_menu: bool = p.get_property("ItemIsMenu").await.unwrap_or(false);
    let tip: (String, Vec<(i32, i32, Vec<u8>)>, String, String) = p.get_property("ToolTip").await.unwrap_or_default();
    let attention_name: String = p.get_property("AttentionIconName").await.unwrap_or_default();
    let status: String = p.get_property("Status").await.unwrap_or_default();
    let name = if status == "NeedsAttention" && !attention_name.is_empty() { attention_name } else { icon_name };
    let key = format!("{item_id}\n{title}");
    // A themed icon matches the rest of the desktop; the app's pixmap is the fallback.
    let icon = find_icon(&name, &theme_path, "").or_else(|| pick_pixmap(&pixmaps)).or_else(|| find_icon("", &theme_path, &key));
    let version = items.lock().unwrap().get(id).map(|i| i.version + 1).unwrap_or(0);
    let title = if title.is_empty() { item_id } else { title };
    let item = Item {
        id: id.to_owned(),
        bus: unique,
        path,
        title,
        icon,
        menu,
        version,
        status,
        item_is_menu,
        tip_title: tip.2,
        tip_text: strip_tags(&tip.3),
        app_key: key.clone(),
    };
    // The command side needs the name to call, not the unique owner.
    let mut stored = item.clone();
    stored.bus = bus;
    items.lock().unwrap().insert(id.to_owned(), stored);
    let _ = tx.send(Msg::Upsert(item));
}

/// Tooltip text may carry simple markup such as <b>. Keep the words.
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").trim().to_owned()
}

fn pick_pixmap(list: &[(i32, i32, Vec<u8>)]) -> Option<Pix> {
    let best = list
        .iter()
        .filter(|(w, h, d)| *w > 0 && *h > 0 && d.len() >= (*w * *h * 4) as usize)
        .min_by_key(|(w, _, _)| (*w - 32).abs())?;
    let (w, h, data) = best;
    let mut rgba = Vec::with_capacity(data.len());
    for px in data.chunks_exact(4) {
        // Network byte order ARGB to RGBA.
        rgba.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
    }
    Some(Pix { w: *w as usize, h: *h as usize, rgba })
}

fn find_icon(name: &str, theme_path: &str, fallback_key: &str) -> Option<Pix> {
    let extra = (!theme_path.is_empty()).then(|| std::path::Path::new(theme_path));
    // The icon name first, then the app's own id and title through its desktop file.
    [name.to_owned(), fallback_key.to_owned()]
        .iter()
        .filter(|k| !k.is_empty())
        .find_map(|k| crate::appicon::load(k, 32, extra))
        .map(|p| (*p).clone())
}

pub fn load_png(path: &std::path::Path) -> Option<Pix> {
    let file = std::fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data.chunks_exact(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        _ => return None,
    };
    Some(Pix { w, h, rgba })
}

async fn handle_cmd(conn: &Connection, items: &Items, tx: &Sender<Msg>, cmd: Cmd) {
    let get = |id: &str| items.lock().unwrap().get(id).cloned();
    match cmd {
        Cmd::Activate(id, x, y) if get(&id).is_some() => {
            let it = get(&id).unwrap();
            let r = conn
                .call_method(Some(it.bus.as_str()), it.path.as_str(), Some("org.kde.StatusNotifierItem"), "Activate", &(x, y))
                .await;
            if r.is_err() {
                let _ = tx.send(Msg::ActivateFailed(id));
            }
        }
        Cmd::Refresh => {
            let ids: Vec<String> = items.lock().unwrap().keys().cloned().collect();
            for id in ids {
                add_item(conn, items, tx, &id).await;
            }
        }
        Cmd::NativeMenu(id) => {
            let Some(it) = get(&id) else { return };
            let _ = conn
                .call_method(Some(it.bus.as_str()), it.path.as_str(), Some("org.kde.StatusNotifierItem"), "ContextMenu", &(0i32, 0i32))
                .await;
        }
        Cmd::OpenMenu(id) => {
            let Some(it) = get(&id) else { return };
            let Some(menu) = it.menu.clone() else {
                // No menu to draw. Ask the app to show its own.
                let _ = conn
                    .call_method(Some(it.bus.as_str()), it.path.as_str(), Some("org.kde.StatusNotifierItem"), "ContextMenu", &(0i32, 0i32))
                    .await;
                return;
            };
            let _ = conn
                .call_method(Some(it.bus.as_str()), menu.as_str(), Some("com.canonical.dbusmenu"), "AboutToShow", &(0i32,))
                .await;
            let reply = conn
                .call_method(
                    Some(it.bus.as_str()),
                    menu.as_str(),
                    Some("com.canonical.dbusmenu"),
                    "GetLayout",
                    &(0i32, -1i32, Vec::<&str>::new()),
                )
                .await;
            if let Ok(reply) = reply {
                let body = reply.body();
                if let Ok((_rev, root)) = body.deserialize::<(u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>))>() {
                    let nodes = root.2.iter().filter_map(|c| parse_node(&c)).collect();
                    let _ = tx.send(Msg::Menu(id, nodes));
                }
            }
        }
        Cmd::ClickMenu(id, menu_id) => {
            let Some(it) = get(&id) else { return };
            let Some(menu) = it.menu.clone() else { return };
            let _ = conn
                .call_method(
                    Some(it.bus.as_str()),
                    menu.as_str(),
                    Some("com.canonical.dbusmenu"),
                    "Event",
                    &(menu_id, "clicked", Value::from(0i32), 0u32),
                )
                .await;
        }
        _ => {}
    }
}

fn parse_node(v: &Value<'_>) -> Option<MenuNode> {
    let mut v = v;
    while let Value::Variant(inner) = v {
        v = inner;
    }
    let Value::Structure(s) = v else { return None };
    let f = s.fields();
    let id = i32::try_from(f.first()?).ok()?;
    let mut props: HashMap<String, Value<'_>> = HashMap::new();
    if let Some(Value::Dict(d)) = f.get(1) {
        for (k, val) in d.iter() {
            if let Ok(k) = <&str>::try_from(k) {
                let mut val = val;
                while let Value::Variant(inner) = val {
                    val = inner;
                }
                props.insert(k.to_owned(), val.try_clone().ok()?);
            }
        }
    }
    let str_prop = |k: &str| props.get(k).and_then(|v| <&str>::try_from(v).ok()).map(str::to_owned);
    let bool_prop = |k: &str, d: bool| props.get(k).and_then(|v| bool::try_from(v).ok()).unwrap_or(d);
    if !bool_prop("visible", true) {
        return None;
    }
    let separator = str_prop("type").as_deref() == Some("separator");
    let label = str_prop("label").unwrap_or_default().replace("__", "\u{1}").replace('_', "").replace('\u{1}', "_");
    let checked = match str_prop("toggle-type").as_deref() {
        Some("checkmark") | Some("radio") => Some(props.get("toggle-state").and_then(|v| i32::try_from(v).ok()).unwrap_or(0) == 1),
        _ => None,
    };
    let children = match f.get(2) {
        Some(Value::Array(a)) => a.iter().filter_map(parse_node).collect(),
        _ => vec![],
    };
    Some(MenuNode { id, label, enabled: bool_prop("enabled", true), separator, checked, children })
}

// ---------- bar drawing ----------

pub fn bar_items(ui: &mut Ui, items: &[Item], actions: &mut Vec<Action>) {
    for it in items {
        let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 4, crate::theme::PLATE);
        }
        let icon_rect = egui::Rect::from_center_size(rect.center(), vec2(16.0, 16.0));
        match texture_for(ui.ctx(), it) {
            Some(tex) => {
                ui.painter().image(
                    tex.id(),
                    icon_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            None => crate::ui::widgets::tile(ui.painter(), rect.center(), 14.0, &it.title),
        }
        // One rule for every item: apps with nothing to show open directly; the rest open a panel.
        let simple = it.menu.is_none() && it.tip_text.is_empty() && !it.item_is_menu;
        let hint = if simple { "click to open" } else { "click for options" };
        let resp = resp.on_hover_text(format!("{} ({hint})", it.title));
        if resp.clicked() || resp.secondary_clicked() {
            if simple {
                actions.push(Action::TrayActivate(it.id.clone()));
            } else {
                actions.push(Action::TrayMenuOpen(it.id.clone(), rect.center().x));
            }
        }
    }
}

pub fn icon_texture(ctx: &egui::Context, it: &Item) -> Option<egui::TextureHandle> {
    texture_for(ctx, it)
}

fn texture_for(ctx: &egui::Context, it: &Item) -> Option<egui::TextureHandle> {
    let pix = it.icon.as_ref()?;
    let key = crate::ui::widgets::id(("trayicon", &it.id, it.version));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return Some(t);
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([pix.w, pix.h], &pix.rgba);
    let tex = ctx.load_texture(format!("tray-{}-{}", it.id, it.version), img, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(key, tex.clone()));
    Some(tex)
}
