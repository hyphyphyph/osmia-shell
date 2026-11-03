//! GPU drawing for layer-shell surfaces. One wgpu device is shared by every
//! surface. Each surface owns its own wgpu surface and egui renderer.

use std::ffi::c_void;
use std::ptr::NonNull;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};

struct Core {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// Lazily creates the shared GPU device when the first surface appears.
pub struct Gfx {
    instance: wgpu::Instance,
    display: NonNull<c_void>,
    core: Option<Core>,
}

pub struct Painter {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: egui_wgpu::Renderer,
}

impl Gfx {
    pub fn new(display: NonNull<c_void>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Gfx {
            instance,
            display,
            core: None,
        }
    }

    pub fn make_painter(
        &mut self,
        wl_surface: NonNull<c_void>,
        width: u32,
        height: u32,
    ) -> Result<Painter, String> {
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                self.display,
            ))),
            raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(wl_surface)),
        };
        // SAFETY: the display and surface outlive the painter. The shell
        // drops every painter before it closes the connection.
        let surface = unsafe { self.instance.create_surface_unsafe(target) }
            .map_err(|e| format!("create surface: {e}"))?;

        if self.core.is_none() {
            let adapter = pollster::block_on(self.instance.request_adapter(
                &wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    force_fallback_adapter: false,
                    compatible_surface: Some(&surface),
                    apply_limit_buckets: false,
                },
            ))
            .or_else(|_| {
                // Software fallback.
                pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    force_fallback_adapter: true,
                    compatible_surface: Some(&surface),
                    apply_limit_buckets: false,
                }))
            })
            .map_err(|e| format!("no adapter: {e}"))?;
            log::info!("gpu adapter: {:?}", adapter.get_info());
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                    .map_err(|e| format!("request device: {e}"))?;
            self.core = Some(Core {
                adapter,
                device,
                queue,
            });
        }
        let core = self.core.as_ref().unwrap();

        let caps = surface.get_capabilities(&core.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or("surface has no formats")?;
        let alpha_mode = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ]
        .into_iter()
        .find(|m| caps.alpha_modes.contains(m))
        .unwrap_or(caps.alpha_modes[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            // Mailbox never makes the main loop wait for the compositor. Frame
            // callbacks already pace the drawing.
            present_mode: if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else {
                wgpu::PresentMode::Fifo
            },
            desired_maximum_frame_latency: 2,
            alpha_mode,
            color_space: Default::default(),
            view_formats: vec![],
        };
        surface.configure(&core.device, &config);
        let renderer = egui_wgpu::Renderer::new(
            &core.device,
            format,
            egui_wgpu::RendererOptions {
                dithering: false,
                ..Default::default()
            },
        );
        Ok(Painter {
            surface,
            config,
            renderer,
        })
    }

    pub fn resize(&self, p: &mut Painter, width: u32, height: u32) {
        let core = self.core.as_ref().unwrap();
        p.config.width = width.max(1);
        p.config.height = height.max(1);
        p.surface.configure(&core.device, &p.config);
    }

    /// Draw one egui frame. Returns false if the frame could not be drawn.
    pub fn paint(
        &self,
        p: &mut Painter,
        textures: &egui::TexturesDelta,
        jobs: &[egui::ClippedPrimitive],
        pixels_per_point: f32,
    ) -> bool {
        let core = self.core.as_ref().unwrap();
        let frame = match p.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                p.surface.configure(&core.device, &p.config);
                return false;
            }
            other => {
                log::debug!("surface not ready: {other:?}");
                return false;
            }
        };
        for (id, deltas) in &textures.set {
            for delta in deltas {
                p.renderer
                    .update_texture(&core.device, &core.queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [p.config.width, p.config.height],
            pixels_per_point,
        };
        let mut encoder = core
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("osmia") });
        let extra = p
            .renderer
            .update_buffers(&core.device, &core.queue, &mut encoder, jobs, &screen);
        {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("osmia-ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            p.renderer
                .render(&mut pass.forget_lifetime(), jobs, &screen);
        }
        core.queue
            .submit(extra.into_iter().chain(std::iter::once(encoder.finish())));
        core.queue.present(frame);
        for id in &textures.free {
            p.renderer.free_texture(id);
        }
        true
    }
}
