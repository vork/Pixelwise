//! GPU texture upload + mip generation for an `HdrImage`.

use std::num::NonZeroU32;
use std::sync::Arc;

use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use crate::io::HdrImage;

pub struct GpuImage {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub source: Arc<HdrImage>,
}

impl GpuImage {
    pub fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: Arc<HdrImage>,
    ) -> Self {
        let w = img.width;
        let h = img.height;
        let mip_levels = ((w.max(h)) as f32).log2().floor() as u32 + 1;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hdr image"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        // Upload mip 0 directly. f16 RGBA → bytes.
        let bytes: &[u8] = cast_slice(&img.data);
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 8), // 4 channels × 2 bytes (f16)
                rows_per_image: Some(h),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view, width: w, height: h, mip_levels, source: img }
    }
}

/// Two samplers we use everywhere: linear-mipmap-linear for normal zoom,
/// nearest for deep zoom (one source pixel covers many screen pixels).
pub struct Samplers {
    pub linear: wgpu::Sampler,
    pub nearest: wgpu::Sampler,
}

impl Samplers {
    pub fn new(device: &wgpu::Device) -> Self {
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Self { linear, nearest }
    }
}

/// Upload a 3D LUT to an `Rgba16Float` 3D texture. The source data is the
/// `.cube` ordering (R fastest, G next, B slowest), interleaved RGB.
pub fn upload_lut(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    lut: &crate::io::lut::Lut,
) -> (wgpu::Texture, wgpu::TextureView) {
    let size = lut.size;
    // Convert RGB → RGBA f16. Padding alpha = 1.0 keeps things simple and
    // keeps the texture format aligned for the GPU.
    use half::f16;
    let texel_count = (size as usize).pow(3);
    let mut data: Vec<f16> = Vec::with_capacity(texel_count * 4);
    for i in 0..texel_count {
        let base = i * 3;
        data.push(f16::from_f32(lut.data[base]));
        data.push(f16::from_f32(lut.data[base + 1]));
        data.push(f16::from_f32(lut.data[base + 2]));
        data.push(f16::ONE);
    }
    let bytes: &[u8] = bytemuck::cast_slice(&data);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lut3d"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: size },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(size * 8), // 4 channels * 2 bytes
            rows_per_image: Some(size),
        },
        wgpu::Extent3d { width: size, height: size, depth_or_array_layers: size },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// A 1D viridis-style ramp for the false-color channel mode.
pub fn create_ramp(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let bytes = crate::color::lut::viridis_ramp(256);
    let tex = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("false-color ramp"),
            size: wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D1,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::default(),
        &bytes,
    );
    let _ = NonZeroU32::new(256);
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Generate mip levels via a tiny compute shader. Idempotent.
pub fn generate_mips(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &GpuImage,
    samplers: &Samplers,
    shader: &wgpu::ShaderModule,
) {
    if img.mip_levels <= 1 {
        return;
    }
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mipmap bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mipmap layout"),
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("mipmap pipeline"),
        layout: Some(&layout),
        module: shader,
        entry_point: "cs",
        compilation_options: Default::default(),
        cache: None,
    });

    for level in 1..img.mip_levels {
        let src_view = img.texture.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: level - 1,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let dst_view = img.texture.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: level,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mipmap bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&samplers.linear) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&dst_view) },
            ],
        });
        let dst_w = (img.width >> level).max(1);
        let dst_h = (img.height >> level).max(1);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mipmap pass"),
        });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mipmap cpass"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&pipe);
            cp.set_bind_group(0, &bg, &[]);
            cp.dispatch_workgroups((dst_w + 7) / 8, (dst_h + 7) / 8, 1);
        }
        queue.submit(Some(enc.finish()));
    }
}
