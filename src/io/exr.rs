//! OpenEXR decode via the `exr` crate (`exrs`).
//!
//! We use the permissive `all_channels` reader and keep *every* channel planar
//! (e.g. `ch00..ch09` or `diffuse.R`/`specular.B` AOVs from a multilayer
//! render), so the user can map any channel onto R/G/B/A in the UI. The
//! initial RGBA mapping is chosen by [`HdrImage::from_source_channels`] using
//! the usual name conventions.

use std::io::Cursor;
use std::sync::Arc;

use ::exr::prelude as exr_pre;
use ::exr::prelude::{read, FlatSamples, Sample};
use exr_pre::{ReadChannels as _, ReadLayers as _};
use half::f16;

use super::{HdrImage, SourceChannel};
use crate::color::space::ColorSpace;
use crate::io::decode::DecodeError;

pub fn decode(bytes: &[u8], name: &str) -> Result<HdrImage, DecodeError> {
    let cursor = Cursor::new(bytes);

    let exr_image = read()
        .no_deep_data()
        .largest_resolution_level()
        .all_channels()
        .first_valid_layer()
        .all_attributes()
        .from_buffered(cursor)
        .map_err(|e| DecodeError::Exr(format!("{e}")))?;

    let layer = exr_image.layer_data;
    let size = layer.size;
    let w = size.x();
    let h = size.y();
    let channels = layer.channel_data.list;

    if channels.is_empty() {
        return Err(DecodeError::Exr("layer has no channels".into()));
    }
    // Channels with non-1 sampling rates (chroma subsampling) would need
    // per-channel pixel mapping; we don't support that yet.
    for ch in &channels {
        if ch.sampling.x() != 1 || ch.sampling.y() != 1 {
            return Err(DecodeError::Exr(format!(
                "channel '{}' uses subsampling {}x{} — unsupported",
                ch.name, ch.sampling.x(), ch.sampling.y()
            )));
        }
    }

    // Lift every channel to a planar f16 plane, preserving file order and the
    // full (layer-qualified) channel name.
    let plane_of = |samples: &FlatSamples| -> Arc<[f16]> {
        let n = w * h;
        let len = n.min(samples.len());
        let mut plane = vec![f16::ZERO; n];
        for i in 0..len {
            let v = match samples.value_by_flat_index(i) {
                Sample::F16(x) => x.to_f32(),
                Sample::F32(x) => x,
                Sample::U32(x) => x as f32,
            };
            plane[i] = f16::from_f32(v);
        }
        Arc::from(plane.into_boxed_slice())
    };

    let source_channels: Vec<SourceChannel> = channels
        .iter()
        .map(|ch| SourceChannel {
            name: ch.name.to_string(),
            samples: plane_of(&ch.sample_data),
        })
        .collect();

    log::info!(
        "exr {name}: {w}x{h}, {} channel(s): {}",
        source_channels.len(),
        source_channels
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    );

    Ok(HdrImage::from_source_channels(
        name.to_string(),
        w as u32,
        h as u32,
        source_channels,
        // EXR convention: scene-linear, sRGB primaries (unless overridden in
        // the chromaticities attribute — we don't read that yet).
        ColorSpace::LinearSRgb,
        true,
        bytes.len(),
        "EXR",
    ))
}
