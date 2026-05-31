//! Per-frame orchestration: compute uniforms from state, bind, draw.

use std::sync::Arc;

use leptos::prelude::*;

use crate::state::store::Store;
use crate::state::view::{DisplayUniform, ViewMode};

use super::context::RenderContext;
use super::pipeline::{lut_bgl, ComparePipeline, CompareUniform, DisplayPipeline};
use super::texture::{upload_lut, GpuImage, Samplers};

pub struct FrameResources {
    pub display: DisplayPipeline,
    pub compare: ComparePipeline,
    pub samplers: Samplers,
    pub ramp: wgpu::TextureView,
    pub mipmap_shader: wgpu::ShaderModule,
    pub primary: Option<Arc<GpuImage>>,
    pub secondary: Option<Arc<GpuImage>>,
    pub lut_bgl: wgpu::BindGroupLayout,
    pub lut_sampler: wgpu::Sampler,
    /// Currently-uploaded LUT (None until the user loads one). When None we
    /// bind the identity placeholder so the bind group is always valid.
    pub lut_loaded: Option<Arc<crate::io::lut::Lut>>,
    pub lut_view: wgpu::TextureView,
    // Keep the texture alive alongside the view.
    pub lut_texture: wgpu::Texture,
}

impl FrameResources {
    pub fn new(ctx: &RenderContext) -> Self {
        let lut_bgl = lut_bgl(&ctx.device);
        let display = DisplayPipeline::new(&ctx.device, ctx.surface_format, &lut_bgl);
        let compare = ComparePipeline::new(&ctx.device, ctx.surface_format, &lut_bgl);
        let samplers = Samplers::new(&ctx.device);
        let ramp = super::texture::create_ramp(&ctx.device, &ctx.queue);
        let mipmap_shader = super::pipeline::mipmap_module(&ctx.device);
        let identity = crate::io::lut::Lut::identity();
        let (lut_texture, lut_view) = upload_lut(&ctx.device, &ctx.queue, &identity);
        let lut_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lut sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Self {
            display,
            compare,
            samplers,
            ramp,
            mipmap_shader,
            primary: None,
            secondary: None,
            lut_bgl,
            lut_sampler,
            lut_loaded: None,
            lut_view,
            lut_texture,
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
        // Re-upload the LUT only when the Arc identity changes.
        let current_lut = store.lut.get_untracked();
        let needs_lut = match (&self.lut_loaded, &current_lut) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (None, Some(_)) | (Some(_), None) => true,
            (None, None) => false,
        };
        if needs_lut {
            let to_upload = current_lut.clone().unwrap_or_else(crate::io::lut::Lut::identity);
            let (tex, view) = upload_lut(&ctx.device, &ctx.queue, &to_upload);
            self.lut_texture = tex;
            self.lut_view = view;
            self.lut_loaded = current_lut;
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
    // Flicker shows A or B alternately using the single-image pipeline; only
    // Split/Onion/Difference actually need the two-image compare pipeline.
    let two_image = matches!(
        mode,
        ViewMode::Split | ViewMode::OnionSkin | ViewMode::Difference
    ) && fr.primary.is_some()
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
            let du = compute_display_uniform(store, ctx, a);
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
                diff_mode: store.diff_mode.get_untracked() as u32,
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
            let lut_bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lut bg"),
                layout: &fr.lut_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&fr.lut_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&fr.lut_sampler) },
                ],
            });
            rp.set_pipeline(&fr.compare.pipeline);
            rp.set_bind_group(0, &bg, &[]);
            rp.set_bind_group(1, &lut_bg, &[]);
            rp.draw(0..3, 0..1);
        } else {
            // For Flicker, swap A↔B based on the toggle so the display
            // pipeline draws the chosen image; everything else just shows A.
            let pick = if matches!(mode, ViewMode::Flicker) {
                let show_a = store.flicker_a.get_untracked();
                if show_a { fr.primary.as_ref() }
                else { fr.secondary.as_ref().or(fr.primary.as_ref()) }
            } else {
                fr.primary.as_ref()
            };
            if let Some(a) = pick {
                let du = compute_display_uniform(store, ctx, a);
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
                let lut_bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("lut bg"),
                    layout: &fr.lut_bgl,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&fr.lut_view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&fr.lut_sampler) },
                    ],
                });
                rp.set_pipeline(&fr.display.pipeline);
                rp.set_bind_group(0, &bg, &[]);
                rp.set_bind_group(1, &lut_bg, &[]);
                rp.draw(0..3, 0..1);
            }
        }
    }

    ctx.queue.submit(Some(enc.finish()));
    frame.present();
}

fn compute_display_uniform(
    store: &Store,
    ctx: &RenderContext,
    img: &GpuImage,
) -> DisplayUniform {
    let cam = store.camera.get_untracked();
    let (sw, sh) = ctx.size;
    let (iw, ih) = (img.width as f32, img.height as f32);
    let zoom = cam.zoom.max(0.001);

    // Map clip-space [-1,1]^2 to image-space [0,1]^2.
    // The vertex shader computes:
    //   uv = xy * uv_xform.xy * 0.5 + 0.5 + uv_xform.zw
    // We want uv == cam.center/(iw,ih) when xy == (0,0) (center of screen),
    // and we want uv to span `visible_image_uv` when xy spans [-1,1].
    let uv_scale_x = (sw as f32) / (zoom * iw);
    let uv_scale_y = (sh as f32) / (zoom * ih);
    let off_x = cam.center[0] / iw - 0.5;
    let off_y = cam.center[1] / ih - 0.5;

    let (pw_segments, pw_white) = crate::color::tonemap::piecewise_segments_from_user(
        store.piecewise_toe_strength.get_untracked(),
        store.piecewise_toe_length.get_untracked(),
        store.piecewise_shoulder_strength.get_untracked(),
        store.piecewise_shoulder_length.get_untracked(),
    );
    DisplayUniform {
        uv_xform: [uv_scale_x, uv_scale_y, off_x, off_y],
        piecewise: pw_segments,
        tm_extras: [
            pw_white,
            store.reinhard_white.get_untracked(),
            store.tonemap_gamma.get_untracked(),
            0.0,
        ],
        exposure: store.exposure.get_untracked(),
        tonemap: store.tonemap.get_untracked() as u32,
        channel: store.channel.get_untracked() as u32,
        clip_flags: store.clip.get_untracked().bits(),
        // HDR output only when the canvas supports it AND the user hasn't
        // switched to the SDR preview.
        output_is_hdr: (ctx.hdr_active && store.hdr_enabled.get_untracked()) as u32,
        width: img.width,
        height: img.height,
        false_color_min: 0.0,
        false_color_max: 1.0,
        lut_active: store.lut.get_untracked().is_some() as u32,
        _pad1: 0.0,
        _pad2: 0.0,
    }
}
