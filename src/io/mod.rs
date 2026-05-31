pub mod decode;
pub mod exr;
pub mod hdr;
pub mod lut;
pub mod png;
pub mod tiff;
pub mod url;

use std::sync::Arc;

use half::f16;

use crate::color::space::ColorSpace;

/// Canonical in-memory image representation. RGBA f16, interleaved, top-to-bottom.
///
/// We store as `f16` (`half::f16`) to halve memory pressure vs. f32 — important
/// in WASM where the linear-memory ceiling is 2–4 GiB. Conversion to f32
/// happens only inside metric/probe inner loops.
pub struct HdrImage {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub channels: ChannelLayout,
    pub data: Arc<[f16]>,
    pub color_space: ColorSpace,
    /// True if the source format had an alpha channel.
    pub has_alpha: bool,
    /// True if the source values are HDR (linear, > 1.0 possible).
    pub is_hdr: bool,
    /// File size on disk (decoded byte count), for the UI.
    pub source_bytes: usize,
    pub format_label: &'static str,
    /// Present for multichannel sources (e.g. multilayer EXR). Holds every
    /// source channel planar, plus the current R/G/B/A mapping that produced
    /// `data`. `None` for ordinary RGBA-only formats.
    pub multichannel: Option<MultiChannel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    Rgba,
}

/// A single source channel, kept planar (row-major, `width * height` long) so
/// the displayed RGBA buffer can be re-derived for any mapping the user picks.
#[derive(Clone)]
pub struct SourceChannel {
    /// Full channel name as stored in the file (e.g. `diffuse.R`, `Z`, `ch00`).
    pub name: String,
    pub samples: Arc<[f16]>,
}

impl SourceChannel {
    /// The component after the last `.` — `diffuse.R` -> `R`, `Z` -> `Z`.
    pub fn leaf(&self) -> &str {
        self.name.rsplit('.').next().unwrap_or(&self.name)
    }

    /// The layer/AOV prefix before the last `.` — `diffuse.R` -> `diffuse`,
    /// `Z` -> `""` (the root layer).
    pub fn layer(&self) -> &str {
        match self.name.rfind('.') {
            Some(i) => &self.name[..i],
            None => "",
        }
    }
}

/// Which source channels feed the displayed R/G/B and the (always optional)
/// alpha. Indices reference [`MultiChannel::channels`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSelection {
    pub r: usize,
    pub g: usize,
    pub b: usize,
    /// `None` = no alpha (treated as fully opaque). Alpha is always optional:
    /// many sources don't carry it, and inspecting RGB without it is common.
    pub a: Option<usize>,
}

/// A grouping of source channels that share a layer/AOV prefix, for the UI's
/// one-click layer picker.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Display name; the root layer (channels with no prefix) is `"RGBA"`.
    pub name: String,
    /// Indices into [`MultiChannel::channels`], in file order.
    pub indices: Vec<usize>,
}

/// Multichannel source data plus the mapping currently realized into the
/// `HdrImage::data` RGBA buffer.
#[derive(Clone)]
pub struct MultiChannel {
    pub channels: Arc<[SourceChannel]>,
    pub selection: ChannelSelection,
}

impl MultiChannel {
    /// True when the source is just a single ordinary R/G/B(/A) set — nothing
    /// worth a channel picker, so the UI can keep the plain layout.
    pub fn is_simple_rgba(&self) -> bool {
        if self.channels.len() > 4 {
            return false;
        }
        let mut layer: Option<&str> = None;
        for ch in self.channels.iter() {
            match layer {
                None => layer = Some(ch.layer()),
                Some(prev) if prev != ch.layer() => return false,
                _ => {}
            }
            if !matches!(ch.leaf().to_ascii_lowercase().as_str(), "r" | "g" | "b" | "a") {
                return false;
            }
        }
        true
    }

    /// Group channels by their layer prefix, preserving first-seen order.
    pub fn layers(&self) -> Vec<Layer> {
        let mut layers: Vec<Layer> = Vec::new();
        for (i, ch) in self.channels.iter().enumerate() {
            let layer_name = ch.layer();
            match layers.iter_mut().find(|l| l.matches(layer_name)) {
                Some(l) => l.indices.push(i),
                None => layers.push(Layer {
                    name: if layer_name.is_empty() { "RGBA".to_string() } else { layer_name.to_string() },
                    indices: vec![i],
                }),
            }
        }
        layers
    }

    /// The selection that mapping `layer`'s channels onto R/G/B/A produces.
    pub fn selection_for_layer(&self, layer: &Layer) -> ChannelSelection {
        select_within(&self.channels, &layer.indices)
    }
}

impl Layer {
    fn matches(&self, layer_name: &str) -> bool {
        (self.name == "RGBA" && layer_name.is_empty()) || self.name == layer_name
    }
}

/// Build the interleaved RGBA f16 buffer for `sel` over planar `channels`.
/// Indices out of range are skipped (R/G/B left at 0); a missing alpha
/// channel yields fully-opaque pixels.
fn interleave(
    width: usize,
    height: usize,
    channels: &[SourceChannel],
    sel: &ChannelSelection,
) -> Arc<[f16]> {
    let n = width * height;
    let total = n * 4;
    let mut data = vec![f16::ZERO; total];

    let mut plane = |idx: usize, slot: usize| {
        if let Some(ch) = channels.get(idx) {
            let len = n.min(ch.samples.len());
            for i in 0..len {
                data[i * 4 + slot] = ch.samples[i];
            }
        }
    };
    plane(sel.r, 0);
    plane(sel.g, 1);
    plane(sel.b, 2);
    match sel.a {
        Some(ai) => plane(ai, 3),
        None => {
            for i in (3..total).step_by(4) {
                data[i] = f16::ONE;
            }
        }
    }
    Arc::from(data.into_boxed_slice())
}

/// Pick R/G/B/A out of a subset of channel indices: match leaf names first,
/// then fall back to the channels' positional order within the subset.
fn select_within(channels: &[SourceChannel], ids: &[usize]) -> ChannelSelection {
    let find = |aliases: &[&str]| -> Option<usize> {
        ids.iter().copied().find(|&i| {
            let leaf = channels[i].leaf().to_ascii_lowercase();
            aliases.iter().any(|a| *a == leaf)
        })
    };
    let nth = |n: usize| ids.get(n).copied();
    let r = find(&["r", "red"]).or_else(|| nth(0)).unwrap_or(0);
    let g = find(&["g", "green"]).or_else(|| nth(1)).unwrap_or(r);
    let b = find(&["b", "blue"]).or_else(|| nth(2)).unwrap_or(g);
    let a = find(&["a", "alpha"]);
    ChannelSelection { r, g, b, a }
}

/// Choose a sensible default mapping for a freshly-decoded multichannel image:
/// favor a canonically-named beauty layer, else the first layer that carries
/// an explicit red channel, else the first layer.
fn default_selection(channels: &[SourceChannel]) -> ChannelSelection {
    if channels.is_empty() {
        return ChannelSelection { r: 0, g: 0, b: 0, a: None };
    }
    let mc = MultiChannel {
        channels: Arc::from(channels.to_vec().into_boxed_slice()),
        selection: ChannelSelection { r: 0, g: 0, b: 0, a: None },
    };
    let layers = mc.layers();
    const BEAUTY: &[&str] = &["rgba", "rgb", "beauty", "color", "combined", "composite"];
    let has_red = |l: &Layer| {
        l.indices
            .iter()
            .any(|&i| matches!(channels[i].leaf().to_ascii_lowercase().as_str(), "r" | "red"))
    };
    let chosen = layers
        .iter()
        .find(|l| BEAUTY.contains(&l.name.to_ascii_lowercase().as_str()))
        .or_else(|| layers.iter().find(|l| has_red(l)))
        .or_else(|| layers.first());
    match chosen {
        Some(l) => select_within(channels, &l.indices),
        None => ChannelSelection { r: 0, g: 0, b: 0, a: None },
    }
}

impl HdrImage {
    /// Stride in `f16` units (channel count) — always 4.
    pub const STRIDE: usize = 4;

    /// Build an image from planar source channels, choosing a default R/G/B/A
    /// mapping. Use this for multichannel formats (EXR) so the user can later
    /// remap channels without re-decoding.
    #[allow(clippy::too_many_arguments)]
    pub fn from_source_channels(
        name: String,
        width: u32,
        height: u32,
        channels: Vec<SourceChannel>,
        color_space: ColorSpace,
        is_hdr: bool,
        source_bytes: usize,
        format_label: &'static str,
    ) -> Self {
        let channels: Arc<[SourceChannel]> = Arc::from(channels.into_boxed_slice());
        let selection = default_selection(&channels);
        let data = interleave(width as usize, height as usize, &channels, &selection);
        Self {
            name,
            width,
            height,
            channels: ChannelLayout::Rgba,
            data,
            color_space,
            has_alpha: selection.a.is_some(),
            is_hdr,
            source_bytes,
            format_label,
            multichannel: Some(MultiChannel { channels, selection }),
        }
    }

    /// Re-derive the displayed RGBA buffer for a new channel mapping. Shares
    /// the (immutable) planar channel store, so only the interleaved buffer is
    /// rebuilt. Returns a clone unchanged for non-multichannel images.
    pub fn with_selection(&self, sel: ChannelSelection) -> Self {
        let Some(mc) = self.multichannel.as_ref() else {
            return self.shallow_clone();
        };
        let data = interleave(self.width as usize, self.height as usize, &mc.channels, &sel);
        Self {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            channels: ChannelLayout::Rgba,
            data,
            color_space: self.color_space,
            has_alpha: sel.a.is_some(),
            is_hdr: self.is_hdr,
            source_bytes: self.source_bytes,
            format_label: self.format_label,
            multichannel: Some(MultiChannel { channels: mc.channels.clone(), selection: sel }),
        }
    }

    fn shallow_clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            channels: self.channels,
            data: self.data.clone(),
            color_space: self.color_space,
            has_alpha: self.has_alpha,
            is_hdr: self.is_hdr,
            source_bytes: self.source_bytes,
            format_label: self.format_label,
            multichannel: self.multichannel.clone(),
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[f32; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y as usize * self.width as usize + x as usize) * Self::STRIDE) as usize;
        let s = &self.data[i..i + 4];
        Some([s[0].to_f32(), s[1].to_f32(), s[2].to_f32(), s[3].to_f32()])
    }

    pub fn bytes_in_memory(&self) -> usize {
        self.data.len() * std::mem::size_of::<f16>()
    }

    pub fn dynamic_range_ev(&self) -> Option<f32> {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for chunk in self.data.chunks_exact(4) {
            for c in &chunk[..3] {
                let v = c.to_f32();
                if v.is_finite() && v > 0.0 {
                    if v < min {
                        min = v;
                    }
                    if v > max {
                        max = v;
                    }
                }
            }
        }
        if min.is_finite() && max.is_finite() && min > 0.0 {
            Some((max / min).log2())
        } else {
            None
        }
    }
}
