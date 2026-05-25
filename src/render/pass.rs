//! Per-frame orchestration: compute uniforms from state, bind, draw.

use std::sync::Arc;

use leptos::prelude::*;

use crate::state::store::Store;
use crate::state::view::{DisplayUniform, ViewMode};

use super::context::RenderContext;
use super::pipeline::{ComparePipeline, CompareUniform, DisplayPipeline};
use super::texture::{GpuImage, Samplers};

pub struct FrameResources {
    pub display: DisplayPipeline,
    pub compare: ComparePipeline,
    pub samplers: Samplers,
    pub ramp: wgpu::TextureView,
    pub mipmap_shader: wgpu::ShaderModule,
    pub primary: Option<Arc<GpuImage>>,
    pub secondary: Option<Arc<GpuImage>>,
}

impl FrameResources {
    pub fn new(ctx: &RenderContext) -> Self {
        let display = DisplayPipeline::new(&ctx.device, ctx.surface_format);
        let compare = ComparePipeline::new(&ctx.device, ctx.surface_format);
        let samplers = Samplers::new(&ctx.device);
        let ramp = super::texture::create_ramp(&ctx.device, &ctx.queue);
        let mipmap_shader = super::pipeline::mipmap_module(&ctx.device);
        Self {
            display,
            compare,
            samplers,
            ramp,
            mipmap_shader,
            primary: None,
            secondary: None,
        }
    }

    /// Upload (or skip if cached by source ptr) the primary/secondary images.
    pub fn sync_images(&mut self, ctx: &RenderContext, store: &Store) {
        if let Some(img) = store.primary_image() {
            let needs = self.primary.as_ref().map(|g| !Arc::ptr_eq(&g.source, &img)).unwrap_or(true);
            if needs {
                let gpu = GpuImage::upload(&ctx.device, &ctx.queue, img);
                super::texture::generate_mips(
                    &ctx.device,
                    &ctx.queue,
                    &gpu,
                    &self.samplers,
                    &self.mipmap_shader,
                );
                self.primary = Some(Arc::new(gpu));
            }
        } else {
            self.primary = None;
        }
        if let Some(img) = store.secondary_image() {
            let needs = self.secondary.as_ref().map(|g| !Arc::ptr_eq(&g.source, &img)).unwrap_or(true);
            if needs {
                let gpu = GpuImage::upload(&ctx.device, &ctx.queue, img);
                super::texture::generate_mips(
                    &ctx.device,
                    &ctx.queue,
                    &gpu,
                    &self.samplers,
                    &self.mipmap_shader,
                );
                self.secondary = Some(Arc::new(gpu));
            }
        } else {
            self.secondary = None;
        }
    }
}

pub fn render(ctx: &mut RenderContext, fr: &mut FrameResources, store: &Store) {
    let frame = match ctx.surface.get_current_texture() {
        Ok(f) => f,
        Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
            ctx.resize(ctx.size.0, ctx.size.1);
            return;
        }
        Err(e) => {
            log::error!("surface error: {e:?}");
            return;
        }
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mode = store.mode.get_untracked();
    let two_image = mode.needs_two_images()
        && fr.primary.is_some()
        && fr.secondary.is_some();

    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

    {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.043, g: 0.043, b: 0.059, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if two_image {
            let (Some(a), Some(b)) = (fr.primary.as_ref(), fr.secondary.as_ref()) else {
                return;
            };
            let du = compute_display_uniform(store, ctx, a, true);
            ctx.queue.write_buffer(&fr.compare.display_uniform, 0, bytemuck::bytes_of(&du));
            let cu = CompareUniform {
                mode: match mode {
                    ViewMode::Split => 0,
                    ViewMode::OnionSkin => 1,
                    ViewMode::Difference => 3,
                    _ => 0,
                },
                blend: if matches!(mode, ViewMode::OnionSkin) {
                    store.onion_alpha.get_untracked()
                } else {
                    store.split_pos.get_untracked()
                },
                diff_scale: 4.0,
                _pad: 0.0,
            };
            ctx.queue.write_buffer(&fr.compare.compare_uniform, 0, bytemuck::bytes_of(&cu));
            let bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compare bg"),
                layout: &fr.compare.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: fr.compare.display_uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&a.view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&b.view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&fr.samplers.linear) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&fr.samplers.nearest) },
                    wgpu::BindGroupEntry { binding: 5, resource: fr.compare.compare_uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&fr.ramp) },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&fr.samplers.linear) },
                ],
            });
            rp.set_pipeline(&fr.compare.pipeline);
            rp.set_bind_group(0, &bg, &[]);
            rp.draw(0..3, 0..1);
        } else if let Some(a) = fr.primary.as_ref() {
            let du = compute_display_uniform(store, ctx, a, false);
            ctx.queue.write_buffer(&fr.display.uniform, 0, bytemuck::bytes_of(&du));
            let bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("display bg"),
                layout: &fr.display.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: fr.display.uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&a.view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&fr.samplers.linear) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&fr.samplers.nearest) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&fr.ramp) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&fr.samplers.linear) },
                ],
            });
            rp.set_pipeline(&fr.display.pipeline);
            rp.set_bind_group(0, &bg, &[]);
            rp.draw(0..3, 0..1);
        }
    }

    ctx.queue.submit(Some(enc.finish()));
    frame.present();
}

fn compute_display_uniform(
    store: &Store,
    ctx: &RenderContext,
    img: &GpuImage,
    is_compare: bool,
) -> DisplayUniform {
    let cam = store.camera.get_untracked();
    let (sw, sh) = ctx.size;
    let (iw, ih) = (img.width as f32, img.height as f32);
    let zoom = cam.zoom.max(0.001);

    // Map clip-space [-1,1]^2 to image-space [0,1]^2.
    //   image_pixels_visible_x = sw / zoom
    //   uv_scale_x = image_pixels_visible / iw
    let uv_scale_x = (sw as f32) / (zoom * iw);
    let uv_scale_y = (sh as f32) / (zoom * ih);
    // Center offset in UV.
    let off_x = cam.center[0] / iw - uv_scale_x * 0.5;
    let off_y = cam.center[1] / ih - uv_scale_y * 0.5;

    DisplayUniform {
        uv_xform: [uv_scale_x, uv_scale_y, off_x, off_y],
        exposure: store.exposure.get_untracked(),
        tonemap: store.tonemap.get_untracked() as u32,
        channel: store.channel.get_untracked() as u32,
        clip_flags: store.clip.get_untracked().bits(),
        output_is_hdr: ctx.hdr_active as u32,
        width: img.width,
        height: img.height,
        false_color_min: 0.0,
        false_color_max: 1.0,
        _pad0: is_compare as u32 as f32,
        _pad1: 0.0,
        _pad2: 0.0,
    }
}
