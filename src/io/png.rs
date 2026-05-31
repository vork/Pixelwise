//! PNG (8- and 16-bit) decode via the `image` crate. Also a small generic
//! `decode_via_image` helper used by the JPEG path.

use std::io::Cursor;
use std::sync::Arc;

use half::f16;
use image::{DynamicImage, ImageReader};

use super::{ChannelLayout, HdrImage};
use crate::color::space::ColorSpace;
use crate::color::transfer::srgb_to_linear;
use crate::io::decode::DecodeError;

pub fn decode(bytes: &[u8], name: &str) -> Result<HdrImage, DecodeError> {
    decode_via_image(bytes, name, "PNG")
}

pub fn decode_via_image(
    bytes: &[u8],
    name: &str,
    format_label: &'static str,
) -> Result<HdrImage, DecodeError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| DecodeError::Image(format!("{e}")))?;
    let img = reader
        .decode()
        .map_err(|e| DecodeError::Image(format!("{e}")))?;

    let (w, h) = (img.width(), img.height());
    let (data, has_alpha) = match img {
        DynamicImage::ImageLuma8(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                let v = srgb_to_linear((px.0[0] as f32) / 255.0);
                out.extend_from_slice(&[
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::ONE,
                ]);
            }
            (out, false)
        }
        DynamicImage::ImageLumaA8(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                let v = srgb_to_linear((px.0[0] as f32) / 255.0);
                let a = (px.0[1] as f32) / 255.0;
                out.extend_from_slice(&[
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::from_f32(a),
                ]);
            }
            (out, true)
        }
        DynamicImage::ImageRgb8(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                out.extend_from_slice(&[
                    f16::from_f32(srgb_to_linear(px.0[0] as f32 / 255.0)),
                    f16::from_f32(srgb_to_linear(px.0[1] as f32 / 255.0)),
                    f16::from_f32(srgb_to_linear(px.0[2] as f32 / 255.0)),
                    f16::ONE,
                ]);
            }
            (out, false)
        }
        DynamicImage::ImageRgba8(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                out.extend_from_slice(&[
                    f16::from_f32(srgb_to_linear(px.0[0] as f32 / 255.0)),
                    f16::from_f32(srgb_to_linear(px.0[1] as f32 / 255.0)),
                    f16::from_f32(srgb_to_linear(px.0[2] as f32 / 255.0)),
                    f16::from_f32(px.0[3] as f32 / 255.0),
                ]);
            }
            (out, true)
        }
        DynamicImage::ImageLuma16(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                let v = srgb_to_linear((px.0[0] as f32) / 65535.0);
                out.extend_from_slice(&[
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::ONE,
                ]);
            }
            (out, false)
        }
        DynamicImage::ImageLumaA16(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                let v = srgb_to_linear((px.0[0] as f32) / 65535.0);
                let a = (px.0[1] as f32) / 65535.0;
                out.extend_from_slice(&[
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::from_f32(v),
                    f16::from_f32(a),
                ]);
            }
            (out, true)
        }
        DynamicImage::ImageRgb16(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                out.extend_from_slice(&[
                    f16::from_f32(srgb_to_linear(px.0[0] as f32 / 65535.0)),
                    f16::from_f32(srgb_to_linear(px.0[1] as f32 / 65535.0)),
                    f16::from_f32(srgb_to_linear(px.0[2] as f32 / 65535.0)),
                    f16::ONE,
                ]);
            }
            (out, false)
        }
        DynamicImage::ImageRgba16(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                out.extend_from_slice(&[
                    f16::from_f32(srgb_to_linear(px.0[0] as f32 / 65535.0)),
                    f16::from_f32(srgb_to_linear(px.0[1] as f32 / 65535.0)),
                    f16::from_f32(srgb_to_linear(px.0[2] as f32 / 65535.0)),
                    f16::from_f32(px.0[3] as f32 / 65535.0),
                ]);
            }
            (out, true)
        }
        DynamicImage::ImageRgb32F(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                out.extend_from_slice(&[
                    f16::from_f32(px.0[0]),
                    f16::from_f32(px.0[1]),
                    f16::from_f32(px.0[2]),
                    f16::ONE,
                ]);
            }
            (out, false)
        }
        DynamicImage::ImageRgba32F(buf) => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for px in buf.pixels() {
                out.extend_from_slice(&[
                    f16::from_f32(px.0[0]),
                    f16::from_f32(px.0[1]),
                    f16::from_f32(px.0[2]),
                    f16::from_f32(px.0[3]),
                ]);
            }
            (out, true)
        }
        other => {
            return Err(DecodeError::Image(format!(
                "unsupported pixel type for {format_label}: {:?}",
                other.color()
            )))
        }
    };

    let is_hdr = matches!(format_label, "TIFF" | "EXR" | "RGBE")
        || data.iter().take(64).any(|v| v.to_f32() > 1.0);
    Ok(HdrImage {
        name: name.to_string(),
        width: w,
        height: h,
        channels: ChannelLayout::Rgba,
        data: Arc::from(data.into_boxed_slice()),
        color_space: ColorSpace::SRgbEncoded,
        has_alpha,
        is_hdr,
        source_bytes: bytes.len(),
        format_label,
        multichannel: None,
    })
}
