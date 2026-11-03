//! Workspaces, from ext-workspace-v1. The protocol lists them and says which is
//! active. It does not say which windows are on which, so osmia learns that:
//! a window belongs to the workspace that was active when it opened or was focused.

use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1},
    ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1},
    ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
};

use crate::app::App;

pub struct Workspace {
    pub handle: ExtWorkspaceHandleV1,
    pub name: String,
    pub active: bool,
}

impl Workspace {
    /// The label to show: its name, or its place in the list.
    pub fn label(&self, index: usize) -> String {
        if self.name.is_empty() { format!("Workspace {}", index + 1) } else { self.name.clone() }
    }
}

/// Name of the active workspace, if any is known.
pub fn current(list: &[Workspace]) -> Option<String> {
    list.iter().enumerate().find(|(_, w)| w.active).map(|(i, w)| w.label(i))
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for App {
    fn event(
        state: &mut Self,
        _: &ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_workspace_manager_v1::Event::Workspace { workspace } => {
                state.shared.workspaces.push(Workspace { handle: workspace, name: String::new(), active: false });
            }
            ext_workspace_manager_v1::Event::Done => state.on_workspaces_changed(),
            _ => {}
        }
    }

    event_created_child!(App, ExtWorkspaceManagerV1, [
        ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for App {
    fn event(
        _: &mut Self,
        _: &ExtWorkspaceGroupHandleV1,
        _: ext_workspace_group_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for App {
    fn event(
        state: &mut Self,
        handle: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id();
        let Some(i) = state.shared.workspaces.iter().position(|w| w.handle.id() == id) else { return };
        match event {
            ext_workspace_handle_v1::Event::Name { name } => state.shared.workspaces[i].name = name,
            ext_workspace_handle_v1::Event::State { state: s } => {
                let active = match s {
                    wayland_client::WEnum::Value(v) => v.contains(ext_workspace_handle_v1::State::Active),
                    _ => false,
                };
                state.shared.workspaces[i].active = active;
            }
            ext_workspace_handle_v1::Event::Removed => {
                state.shared.workspaces.remove(i);
                handle.destroy();
            }
            _ => {}
        }
    }
}

// ---- keyboard shortcut inhibit: lets the settings window hear keys labwc binds ----

use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1,
    zwp_keyboard_shortcuts_inhibitor_v1::{self, ZwpKeyboardShortcutsInhibitorV1},
};

impl Dispatch<ZwpKeyboardShortcutsInhibitManagerV1, ()> for App {
    fn event(_: &mut Self, _: &ZwpKeyboardShortcutsInhibitManagerV1, _: <ZwpKeyboardShortcutsInhibitManagerV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpKeyboardShortcutsInhibitorV1, ()> for App {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitorV1,
        event: zwp_keyboard_shortcuts_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        log::debug!("shortcut inhibitor: {event:?}");
    }
}
