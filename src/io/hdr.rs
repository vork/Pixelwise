//! Radiance HDR (.hdr) decode via the `image` crate's `hdr` feature.

use std::io::Cursor;
use std::mem::size_of;
use std::sync::Arc;

use half::f16;
use image::codecs::hdr::HdrDecoder;
use image::ImageDecoder;

use super::{ChannelLayout, HdrImage};
use crate::color::space::ColorSpace;
use crate::io::decode::DecodeError;

pub fn decode(bytes: &[u8], name: &str) -> Result<HdrImage, DecodeError> {
    let cursor = Cursor::new(bytes);
    let dec = HdrDecoder::new_nonstrict(cursor).map_err(|e| DecodeError::Hdr(format!("{e}")))?;
    let (width, height) = dec.dimensions();

    // ImageDecoder::read_image writes Rgb<f32> bytes; we drop into a typed view.
    let total = (width as usize) * (height as usize) * 3;
    let mut bytes_buf = vec![0u8; total * size_of::<f32>()];
    dec.read_image(&mut bytes_buf)
        .map_err(|e| DecodeError::Hdr(format!("{e}")))?;
    let floats: &[f32] = bytemuck::cast_slice(&bytes_buf);

    let mut data = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for px in floats.chunks_exact(3) {
        data.push(f16::from_f32(px[0]));
        data.push(f16::from_f32(px[1]));
        data.push(f16::from_f32(px[2]));
        data.push(f16::ONE);
    }
    Ok(HdrImage {
        name: name.to_string(),
        width,
        height,
        channels: ChannelLayout::Rgba,
        data: Arc::from(data.into_boxed_slice()),
        color_space: ColorSpace::LinearSRgb,
        has_alpha: false,
        is_hdr: true,
        source_bytes: bytes.len(),
        format_label: "RGBE",
        multichannel: None,
    })
}
