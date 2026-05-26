//! wgpu device + surface init from a `<canvas>`.

use std::sync::Arc;

use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use wgpu::SurfaceTarget;

use super::canvas_hdr::{self, HdrCapability};

/// All long-lived GPU resources. Lives in a StoredValue in the UI layer.
pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub surface: wgpu::Surface<'static>,
    pub surface_format: wgpu::TextureFormat,
    pub hdr_active: bool,
    pub hdr_capability: HdrCapability,
    pub canvas: HtmlCanvasElement,
    pub size: (u32, u32),
}

impl RenderContext {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let target = SurfaceTarget::Canvas(canvas.clone());
        let surface = instance
            .create_surface(target)
            .map_err(|e| format!("create_surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "no compatible WebGPU adapter".to_string())?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("pixelwise device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits {
                        max_texture_dimension_2d: 16384,
                        ..wgpu::Limits::downlevel_webgl2_defaults()
                    }
                    .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| format!("request_device: {e}"))?;

        device.on_uncaptured_error(Box::new(|err| {
            log::error!("wgpu uncaptured error: {err}");
        }));

        let capability = HdrCapability::detect();
        let (w, h) = canvas_size(&canvas);

        // Use the adapter-provided defaults so format/present_mode/alpha_mode
        // match exactly what WebGPU expects for this canvas; otherwise wgpu's
        // WebGPU backend silently picks an internal swap-chain that the
        // canvas never displays.
        let mut config = surface
            .get_default_config(&adapter, w.max(1), h.max(1))
            .ok_or_else(|| "surface has no default config".to_string())?;
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let surface_format = config.format;
        surface.configure(&device, &config);

        let hdr_active = match canvas_hdr::configure(
            &canvas,
            &canvas_device(&canvas).unwrap_or(wasm_bindgen::JsValue::UNDEFINED),
            true,
        ) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("canvas hdr configure failed: {e:?}");
                false
            }
        };

        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface,
            surface_format: if hdr_active {
                wgpu::TextureFormat::Rgba16Float
            } else {
                surface_format
            },
            hdr_active,
            hdr_capability: capability,
            canvas,
            size: (w, h),
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.size = (w, h);
        let mut config = self
            .surface
            .get_default_config(&self.adapter, w, h)
            .expect("surface has no default config");
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        config.format = self.surface_format;
        self.surface.configure(&self.device, &config);
    }
}

pub fn canvas_size(canvas: &HtmlCanvasElement) -> (u32, u32) {
    let el: &web_sys::Element = canvas.unchecked_ref();
    let rect = el.get_bounding_client_rect();
    let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0);
    let w = (rect.width() * dpr).round().max(1.0) as u32;
    let h = (rect.height() * dpr).round().max(1.0) as u32;
    canvas.set_width(w);
    canvas.set_height(h);
    (w, h)
}

fn canvas_device(canvas: &HtmlCanvasElement) -> Option<wasm_bindgen::JsValue> {
    let ctx = canvas.get_context("webgpu").ok().flatten()?;
    js_sys::Reflect::get(&ctx, &"device".into()).ok()
}
