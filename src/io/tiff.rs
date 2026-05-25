//! TIFF decode via the `tiff` crate directly (bypassing `image`'s
//! `DynamicImage` wrapper) so we preserve `SampleFormat::Float` precision.

use std::io::Cursor;
use std::sync::Arc;

use half::f16;
use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;
use tiff::ColorType;

use super::{ChannelLayout, HdrImage};
use crate::color::space::ColorSpace;
use crate::color::transfer::srgb_to_linear;
use crate::io::decode::DecodeError;

pub fn decode(bytes: &[u8], name: &str) -> Result<HdrImage, DecodeError> {
    let cursor = Cursor::new(bytes);
    let mut dec = Decoder::new(cursor).map_err(|e| DecodeError::Tiff(format!("{e}")))?;
    let (w, h) = dec
        .dimensions()
        .map_err(|e| DecodeError::Tiff(format!("{e}")))?;
    let color = dec
        .colortype()
        .map_err(|e| DecodeError::Tiff(format!("{e}")))?;

    let result = dec
        .read_image()
        .map_err(|e| DecodeError::Tiff(format!("{e}")))?;

    // Was the source pre-linear (float / >=16-bit usually) or sRGB-encoded?
    let assume_srgb = matches!(color, ColorType::RGB(8) | ColorType::RGBA(8) | ColorType::Gray(8));

    let (data, has_alpha, is_hdr) = match (color, result) {
        (ColorType::RGB(8), DecodingResult::U8(buf)) => {
            (unpack_rgb_u8(&buf, assume_srgb), false, false)
        }
        (ColorType::RGBA(8), DecodingResult::U8(buf)) => {
            (unpack_rgba_u8(&buf, assume_srgb), true, false)
        }
        (ColorType::Gray(8), DecodingResult::U8(buf)) => {
            (unpack_gray_u8(&buf, assume_srgb), false, false)
        }
        (ColorType::RGB(16), DecodingResult::U16(buf)) => (unpack_rgb_u16(&buf), false, false),
        (ColorType::RGBA(16), DecodingResult::U16(buf)) => (unpack_rgba_u16(&buf), true, false),
        (ColorType::Gray(16), DecodingResult::U16(buf)) => (unpack_gray_u16(&buf), false, false),
        (ColorType::RGB(32), DecodingResult::F32(buf)) => (unpack_rgb_f32(&buf), false, true),
        (ColorType::RGBA(32), DecodingResult::F32(buf)) => (unpack_rgba_f32(&buf), true, true),
        (ColorType::Gray(32), DecodingResult::F32(buf)) => (unpack_gray_f32(&buf), false, true),
        (c, _) => {
            return Err(DecodeError::Tiff(format!(
                "unsupported TIFF pixel layout: {:?}",
                c
            )))
        }
    };

    // Optional ICC sniff via the Photometric / Resolution tags goes here.
    let _ = dec.get_tag(Tag::PhotometricInterpretation);

    Ok(HdrImage {
        name: name.to_string(),
        width: w,
        height: h,
        channels: ChannelLayout::Rgba,
        data: Arc::from(data.into_boxed_slice()),
        color_space: if assume_srgb {
            ColorSpace::SRgbEncoded
        } else {
            ColorSpace::LinearSRgb
        },
        has_alpha,
        is_hdr,
        source_bytes: bytes.len(),
        format_label: "TIFF",
    })
}

fn unpack_rgb_u8(buf: &[u8], srgb: bool) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len() / 3 * 4);
    for px in buf.chunks_exact(3) {
        out.push(f16::from_f32(cvt_u8(px[0], srgb)));
        out.push(f16::from_f32(cvt_u8(px[1], srgb)));
        out.push(f16::from_f32(cvt_u8(px[2], srgb)));
        out.push(f16::ONE);
    }
    out
}
fn unpack_rgba_u8(buf: &[u8], srgb: bool) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len() / 4 * 4);
    for px in buf.chunks_exact(4) {
        out.push(f16::from_f32(cvt_u8(px[0], srgb)));
        out.push(f16::from_f32(cvt_u8(px[1], srgb)));
        out.push(f16::from_f32(cvt_u8(px[2], srgb)));
        out.push(f16::from_f32(px[3] as f32 / 255.0));
    }
    out
}
fn unpack_gray_u8(buf: &[u8], srgb: bool) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len() * 4);
    for &v in buf {
        let g = f16::from_f32(cvt_u8(v, srgb));
        out.extend_from_slice(&[g, g, g, f16::ONE]);
    }
    out
}
fn unpack_rgb_u16(buf: &[u16]) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len() / 3 * 4);
    for px in buf.chunks_exact(3) {
        out.push(f16::from_f32(px[0] as f32 / 65535.0));
        out.push(f16::from_f32(px[1] as f32 / 65535.0));
        out.push(f16::from_f32(px[2] as f32 / 65535.0));
        out.push(f16::ONE);
    }
    out
}
fn unpack_rgba_u16(buf: &[u16]) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len());
    for px in buf.chunks_exact(4) {
        out.push(f16::from_f32(px[0] as f32 / 65535.0));
        out.push(f16::from_f32(px[1] as f32 / 65535.0));
        out.push(f16::from_f32(px[2] as f32 / 65535.0));
        out.push(f16::from_f32(px[3] as f32 / 65535.0));
    }
    out
}
fn unpack_gray_u16(buf: &[u16]) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len() * 4);
    for &v in buf {
        let g = f16::from_f32(v as f32 / 65535.0);
        out.extend_from_slice(&[g, g, g, f16::ONE]);
    }
    out
}
fn unpack_rgb_f32(buf: &[f32]) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len() / 3 * 4);
    for px in buf.chunks_exact(3) {
        out.push(f16::from_f32(px[0]));
        out.push(f16::from_f32(px[1]));
        out.push(f16::from_f32(px[2]));
        out.push(f16::ONE);
    }
    out
}
fn unpack_rgba_f32(buf: &[f32]) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len());
    for px in buf.chunks_exact(4) {
        out.push(f16::from_f32(px[0]));
        out.push(f16::from_f32(px[1]));
        out.push(f16::from_f32(px[2]));
        out.push(f16::from_f32(px[3]));
    }
    out
}
fn unpack_gray_f32(buf: &[f32]) -> Vec<f16> {
    let mut out = Vec::with_capacity(buf.len() * 4);
    for &v in buf {
        let g = f16::from_f32(v);
        out.extend_from_slice(&[g, g, g, f16::ONE]);
    }
    out
}

#[inline]
fn cvt_u8(v: u8, srgb: bool) -> f32 {
    let n = v as f32 / 255.0;
    if srgb {
        srgb_to_linear(n)
    } else {
        n
    }
}
