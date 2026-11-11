//! One layer-shell surface with its own egui context.

use std::time::{Duration, Instant};

use smithay_client_toolkit::shell::{wlr_layer::LayerSurface, xdg::window::Window, WaylandSurface};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::protocol::wl_output::WlOutput;

use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;

use crate::gfx::Painter;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Bar,
    Popups,
    Overlay,
    /// The settings window: a normal desktop window, not a layer surface.
    Settings,
    /// The screenshot overlay, one per output.
    Capture,
    /// The volume and brightness display. It takes no input.
    Osd,
}

/// What a window is drawn on.
pub enum Surf {
    Layer(LayerSurface),
    Window(Window),
}

impl Surf {
    pub fn wl_surface(&self) -> &WlSurface {
        match self {
            Surf::Layer(l) => l.wl_surface(),
            Surf::Window(w) => w.wl_surface(),
        }
    }
    pub fn commit(&self) {
        match self {
            Surf::Layer(l) => l.commit(),
            Surf::Window(w) => w.commit(),
        }
    }
    pub fn is_layer(&self, other: &LayerSurface) -> bool {
        matches!(self, Surf::Layer(l) if l == other)
    }
    pub fn is_window(&self, other: &Window) -> bool {
        matches!(self, Surf::Window(w) if w == other)
    }
}

pub struct Win {
    // Field order matters. The painter holds a GPU surface on the layer
    // surface, so it must be dropped first.
    pub painter: Option<Painter>,
    pub kind: Kind,
    pub surf: Surf,
    pub output: Option<WlOutput>,
    pub ctx: egui::Context,
    /// Size in logical pixels, as given by the compositor.
    pub size: (u32, u32),
    /// Pixels per logical pixel. Fractional when the compositor supports it.
    pub scale: f32,
    /// Fractional scale and viewport objects. Both or neither.
    pub frac: Option<(WpFractionalScaleV1, WpViewport)>,
    pub configured: bool,
    pub dirty: bool,
    pub frame_pending: bool,
    pub repaint_at: Option<Instant>,
    pub events: Vec<egui::Event>,
    pub pending_tex: egui::TexturesDelta,
    pub start: Instant,
}

impl Win {
    pub fn new(kind: Kind, surf: Surf, output: Option<WlOutput>) -> Self {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        Win {
            kind,
            surf,
            output,
            ctx,
            painter: None,
            size: (0, 0),
            scale: 1.0,
            frac: None,
            configured: false,
            dirty: true,
            frame_pending: false,
            repaint_at: None,
            events: Vec::new(),
            pending_tex: Default::default(),
            start: Instant::now(),
        }
    }

    /// Buffer size in device pixels.
    pub fn px(&self) -> (u32, u32) {
        (
            ((self.size.0 as f32 * self.scale).round() as u32).max(1),
            ((self.size.1 as f32 * self.scale).round() as u32).max(1),
        )
    }

    pub fn wants_draw(&self, now: Instant) -> bool {
        self.configured
            && !self.frame_pending
            && (self.dirty || self.repaint_at.is_some_and(|t| t <= now))
    }

    pub fn deadline(&self) -> Option<Instant> {
        if self.dirty && !self.frame_pending {
            return Some(Instant::now());
        }
        self.repaint_at
    }

    pub fn schedule(&mut self, delay: Duration) {
        if delay >= Duration::from_secs(3600) {
            self.repaint_at = None;
        } else if delay.is_zero() {
            self.dirty = true;
            self.repaint_at = None;
        } else {
            self.repaint_at = Some(Instant::now() + delay);
        }
    }
}

impl Drop for Win {
    fn drop(&mut self) {
        // egui panics if texture updates are dropped unapplied.
        self.pending_tex.clear();
        if let Some((f, v)) = self.frac.take() {
            f.destroy();
            v.destroy();
        }
    }
}
