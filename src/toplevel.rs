//! Open windows, from the wlr foreign-toplevel protocol. They feed the bar's
//! attention list and the launcher's window search.

use std::time::Instant;

use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

use crate::app::App;

pub struct Toplevel {
    pub handle: ZwlrForeignToplevelHandleV1,
    pub title: String,
    pub app_id: String,
    pub minimized: bool,
    pub activated: bool,
    /// Ids of notifications that put this window in alert mode.
    pub alert_ids: Vec<u32>,
    /// When the window entered the attention list.
    pub attn_since: Option<Instant>,
    pub ready: bool,
    /// The workspace it was last seen on, learned from focus and opening.
    pub ws: Option<String>,
    pub ws_at: Option<Instant>,
}

impl Toplevel {
    pub fn new(handle: ZwlrForeignToplevelHandleV1) -> Self {
        Toplevel {
            handle,
            title: String::new(),
            app_id: String::new(),
            minimized: false,
            activated: false,
            alert_ids: Vec::new(),
            attn_since: None,
            ws: None,
            ws_at: None,
            ready: false,
        }
    }

    pub fn in_alert(&self) -> bool {
        !self.alert_ids.is_empty()
    }

    /// Update the attention clock after a state change.
    pub fn refresh_attention(&mut self) {
        if self.activated {
            self.alert_ids.clear();
        }
        let wants = self.minimized || self.in_alert();
        match (wants, self.attn_since) {
            (true, None) => self.attn_since = Some(Instant::now()),
            (false, Some(_)) => self.attn_since = None,
            _ => {}
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for App {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            state.shared.toplevels.push(Toplevel::new(toplevel));
        }
    }

    event_created_child!(App, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for App {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event as E;
        let id = handle.id();
        let Some(i) = state.shared.toplevels.iter().position(|t| t.handle.id() == id) else {
            return;
        };
        match event {
            E::Title { title } => state.shared.toplevels[i].title = title,
            E::AppId { app_id } => state.shared.toplevels[i].app_id = app_id,
            E::State { state: bytes } => {
                let t = &mut state.shared.toplevels[i];
                let flags: Vec<u32> = bytes
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                t.minimized = flags.contains(&1);
                let was = t.activated;
                t.activated = flags.contains(&2);
                if t.activated && !was {
                    t.ws = crate::workspace::current(&state.shared.workspaces).or(t.ws.take());
                    t.ws_at = Some(Instant::now());
                }
            }
            E::Done => {
                let late = state.started.elapsed() > std::time::Duration::from_secs(3);
                let cur = crate::workspace::current(&state.shared.workspaces);
                let t = &mut state.shared.toplevels[i];
                if !t.ready && late && t.ws.is_none() {
                    // A window that opens now opens on the workspace you are on.
                    t.ws = cur;
                    t.ws_at = Some(Instant::now());
                }
                t.ready = true;
                t.refresh_attention();
                state.on_toplevels_changed();
            }
            E::Closed => {
                state.shared.toplevels.remove(i);
                handle.destroy();
                state.on_toplevels_changed();
            }
            _ => {}
        }
    }
}
