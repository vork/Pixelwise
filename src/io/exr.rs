//! OpenEXR decode via the `exr` crate (`exrs`).
//!
//! Returns RGBA f16, interleaved. We use the permissive `all_channels` reader
//! so files with non-standard channel names (e.g. `ch00..ch09` from renderer
//! AOVs) still load — we look up R/G/B/A by common name conventions and fall
//! back to the first three channels by index when the layer doesn't use them.

use std::io::Cursor;
use std::sync::Arc;

use ::exr::prelude as exr_pre;
use ::exr::prelude::{read, FlatSamples, Sample};
use exr_pre::{ReadChannels as _, ReadLayers as _};
use half::f16;

use super::{ChannelLayout, HdrImage};
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

    // Channel-name lookup: match the leaf component (`foo.R` -> `R`) case-
    // insensitively against any of the supplied aliases.
    let pick = |aliases: &[&str]| -> Option<usize> {
        channels.iter().position(|ch| {
            let name = ch.name.to_string().to_ascii_lowercase();
            let leaf = name.rsplit('.').next().unwrap_or(name.as_str());
            aliases.iter().any(|a| *a == leaf)
        })
    };

    let r_idx = pick(&["r", "red"]).unwrap_or(0);
    let g_idx = pick(&["g", "green"]).unwrap_or(1.min(channels.len() - 1));
    let b_idx = pick(&["b", "blue"]).unwrap_or(2.min(channels.len() - 1));
    let a_idx = pick(&["a", "alpha"]);

    let total = w * h * 4;
    let mut data = vec![f16::ZERO; total];
    if a_idx.is_none() {
        // No alpha in source — default to fully opaque.
        for i in (3..total).step_by(4) {
            data[i] = f16::ONE;
        }
    }

    let copy = |dst: &mut [f16], samples: &FlatSamples, slot: usize| {
        let len = (w * h).min(samples.len());
        for i in 0..len {
            let v = match samples.value_by_flat_index(i) {
                Sample::F16(x) => x.to_f32(),
                Sample::F32(x) => x,
                Sample::U32(x) => x as f32,
            };
            dst[i * 4 + slot] = f16::from_f32(v);
        }
    };

    copy(&mut data, &channels[r_idx].sample_data, 0);
    copy(&mut data, &channels[g_idx].sample_data, 1);
    copy(&mut data, &channels[b_idx].sample_data, 2);
    if let Some(ai) = a_idx {
        copy(&mut data, &channels[ai].sample_data, 3);
    }

    log::info!(
        "exr {name}: {w}x{h}, {} channel(s), mapped R={} G={} B={} A={}",
        channels.len(),
        channels[r_idx].name,
        channels[g_idx].name,
        channels[b_idx].name,
        a_idx.map(|i| channels[i].name.to_string()).unwrap_or_else(|| "<none>".into()),
    );

    Ok(HdrImage {
        name: name.to_string(),
        width: w as u32,
        height: h as u32,
        channels: ChannelLayout::Rgba,
        data: Arc::from(data.into_boxed_slice()),
        // EXR convention: scene-linear, sRGB primaries (unless overridden in
        // the chromaticities attribute — we don't read that yet).
        color_space: ColorSpace::LinearSRgb,
        has_alpha: a_idx.is_some(),
        is_hdr: true,
        source_bytes: bytes.len(),
        format_label: "EXR",
    })
}
