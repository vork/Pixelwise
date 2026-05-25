//! OpenEXR decode via the `exr` crate (`exrs`).
//!
//! Returns RGBA f16, interleaved. Missing alpha → 1.0. Missing color channels
//! default to 0.0. We coerce all values into f16 at parse time to halve the
//! WASM memory footprint vs. the f32 intermediate.

use std::io::Cursor;
use std::sync::Arc;

// Use a glob from a non-prelude path so we get all the traits (ReadChannels,
// ReadLargestLevel, ReadAttributes, etc.) without shadowing `std::Result`.
use ::exr::prelude as exr_pre;
use ::exr::prelude::{read, RgbaChannels};
use ::exr::math::Vec2;
use exr_pre::{ReadChannels as _, ReadLayers as _};
use half::f16;

use super::{ChannelLayout, HdrImage};
use crate::color::space::ColorSpace;
use crate::io::decode::DecodeError;

pub fn decode(bytes: &[u8], name: &str) -> Result<HdrImage, DecodeError> {
    let cursor = Cursor::new(bytes);

    // High-level rgba_channels reader:
    //   create_image(resolution: Vec2<usize>, _channels: &RgbaChannels) -> UserImage
    //   set_pixel(image: &mut UserImage, position: Vec2<usize>, (r,g,b,a): (f32,f32,f32,f32))
    let exr_image = read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |resolution: Vec2<usize>, _channels: &RgbaChannels| {
                FlatRgba::new(resolution.x(), resolution.y())
            },
            |img: &mut FlatRgba, position: Vec2<usize>, (r, g, b, a): (f32, f32, f32, f32)| {
                img.set(position.x(), position.y(), r, g, b, a);
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_buffered(cursor)
        .map_err(|e| DecodeError::Exr(format!("{e}")))?;

    let layer = exr_image.layer_data;
    let img = layer.channel_data.pixels;

    Ok(HdrImage {
        name: name.to_string(),
        width: img.width as u32,
        height: img.height as u32,
        channels: ChannelLayout::Rgba,
        data: Arc::from(img.data.into_boxed_slice()),
        // EXR convention: scene-linear, sRGB primaries (unless overridden in
        // chromaticities attribute — we don't read that yet).
        color_space: ColorSpace::LinearSRgb,
        has_alpha: true,
        is_hdr: true,
        source_bytes: bytes.len(),
        format_label: "EXR",
    })
}

struct FlatRgba {
    width: usize,
    height: usize,
    data: Vec<f16>,
}

impl FlatRgba {
    fn new(width: usize, height: usize) -> Self {
        let total = width * height * 4;
        let mut data = vec![f16::ZERO; total];
        for i in (3..total).step_by(4) {
            data[i] = f16::ONE;
        }
        Self { width, height, data }
    }
    #[inline]
    fn set(&mut self, x: usize, y: usize, r: f32, g: f32, b: f32, a: f32) {
        let i = (y * self.width + x) * 4;
        self.data[i] = f16::from_f32(r);
        self.data[i + 1] = f16::from_f32(g);
        self.data[i + 2] = f16::from_f32(b);
        self.data[i + 3] = f16::from_f32(a);
    }
}
