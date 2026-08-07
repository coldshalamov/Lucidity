//! Per-window egui host sharing WezTerm's WebGPU device/queue/surface.

use egui::{ClippedPrimitive, Context, FullOutput, PlatformOutput, RawInput, TexturesDelta};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Factory installed on product hooks; stock WezTerm never calls this.
pub type ProductUiFactory =
    Arc<dyn Fn() -> Box<dyn ProductUiController> + Send + Sync + 'static>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TerminalVisibility {
    Visible,
    CoveredByOpaqueUi,
    HiddenPreserveSize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProductLayoutSpec {
    pub sidebar_width_points: f32,
    pub header_height_points: f32,
    pub bottom_bar_height_points: f32,
    pub terminal_visibility: TerminalVisibility,
}

impl Default for ProductLayoutSpec {
    fn default() -> Self {
        Self {
            sidebar_width_points: 288.0,
            header_height_points: 52.0,
            bottom_bar_height_points: 0.0,
            terminal_visibility: TerminalVisibility::Visible,
        }
    }
}

pub struct ProductUiFrame {
    pub window_width_points: f32,
    pub window_height_points: f32,
    pub pixels_per_point: f32,
    pub focused: bool,
    pub now: Instant,
}

#[derive(Clone, Debug, Default)]
pub struct ProductUiResponse {
    pub layout_changed: bool,
    pub terminal_focus_requested: bool,
    pub repaint_after: Option<Duration>,
    pub wants_pointer_input: bool,
    pub wants_keyboard_input: bool,
}

/// Lucidity (or any product) implements this; WezTerm only hosts it.
pub trait ProductUiController {
    fn layout_spec(&self) -> ProductLayoutSpec;
    fn show(&mut self, ctx: &Context, frame: &ProductUiFrame) -> ProductUiResponse;
    /// Optional: publish a new snapshot from the host thread.
    fn on_host_tick(&mut self) {}
}

/// Per-window UI host. Absent in stock WezTerm mode.
pub struct ProductUiHost {
    pub context: Context,
    pub controller: Box<dyn ProductUiController>,
    pub raw_input: RawInput,
    pub last_layout: ProductLayoutSpec,
    pub last_platform_output: PlatformOutput,
    pub last_textures_delta: TexturesDelta,
    pub last_primitives: Vec<ClippedPrimitive>,
    pub last_response: ProductUiResponse,
    pub start: Instant,
    renderer: Option<egui_wgpu::Renderer>,
    renderer_format: Option<wgpu::TextureFormat>,
}

impl ProductUiHost {
    pub fn new(controller: Box<dyn ProductUiController>) -> Self {
        let context = Context::default();
        context.set_visuals(egui::Visuals::dark());
        Self {
            context,
            controller,
            raw_input: RawInput::default(),
            last_layout: ProductLayoutSpec::default(),
            last_platform_output: PlatformOutput::default(),
            last_textures_delta: TexturesDelta::default(),
            last_primitives: Vec::new(),
            last_response: ProductUiResponse::default(),
            start: Instant::now(),
            renderer: None,
            renderer_format: None,
        }
    }

    pub fn layout_spec(&self) -> ProductLayoutSpec {
        self.controller.layout_spec()
    }

    pub fn begin_frame(
        &mut self,
        window_width_px: u32,
        window_height_px: u32,
        pixels_per_point: f32,
        focused: bool,
    ) {
        let ppp = pixels_per_point.max(0.5);
        let width_pts = window_width_px as f32 / ppp;
        let height_pts = window_height_px as f32 / ppp;
        self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width_pts, height_pts),
        ));
        self.raw_input.max_texture_side = Some(8192);
        self.raw_input.time = Some(self.start.elapsed().as_secs_f64());
        // viewport info
        self.raw_input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);

        self.controller.on_host_tick();
        self.context.begin_pass(self.raw_input.take());

        let frame = ProductUiFrame {
            window_width_points: width_pts,
            window_height_points: height_pts,
            pixels_per_point: ppp,
            focused,
            now: Instant::now(),
        };
        let mut response = self.controller.show(&self.context, &frame);
        response.wants_pointer_input = self.context.wants_pointer_input();
        response.wants_keyboard_input = self.context.wants_keyboard_input();

        let full: FullOutput = self.context.end_pass();
        self.last_platform_output = full.platform_output;
        let clipped = self
            .context
            .tessellate(full.shapes, full.pixels_per_point);
        self.last_primitives = clipped;
        self.last_textures_delta = full.textures_delta;
        self.last_layout = self.controller.layout_spec();
        self.last_response = response;
        self.raw_input = RawInput::default();
    }

    pub fn wants_pointer_input(&self) -> bool {
        self.last_response.wants_pointer_input
    }

    pub fn wants_keyboard_input(&self) -> bool {
        self.last_response.wants_keyboard_input
    }

    /// Render egui into an existing WebGPU pass (LoadOp::Load) using the shared device.
    pub fn render_webgpu(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        width_px: u32,
        height_px: u32,
        pixels_per_point: f32,
    ) -> anyhow::Result<()> {
        if self.renderer.is_none() || self.renderer_format != Some(surface_format) {
            self.renderer = Some(egui_wgpu::Renderer::new(
                device,
                surface_format,
                None,
                1,
                false,
            ));
            self.renderer_format = Some(surface_format);
        }
        let renderer = self.renderer.as_mut().unwrap();

        for (id, image_delta) in &self.last_textures_delta.set {
            renderer.update_texture(device, queue, *id, image_delta);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width_px.max(1), height_px.max(1)],
            pixels_per_point: pixels_per_point.max(0.5),
        };

        let _callbacks = renderer.update_buffers(
            device,
            queue,
            encoder,
            &self.last_primitives,
            &screen_descriptor,
        );

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lucidity-egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            renderer.render(
                &mut render_pass.forget_lifetime(),
                &self.last_primitives,
                &screen_descriptor,
            );
        }

        for id in &self.last_textures_delta.free {
            renderer.free_texture(id);
        }
        self.last_textures_delta = TexturesDelta::default();
        Ok(())
    }

    pub fn push_event(&mut self, event: egui::Event) {
        self.raw_input.events.push(event);
    }

    pub fn set_modifiers(&mut self, modifiers: egui::Modifiers) {
        self.raw_input.modifiers = modifiers;
    }
}
