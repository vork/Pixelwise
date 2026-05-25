pub mod decode;
pub mod exr;
pub mod hdr;
pub mod png;
pub mod tiff;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    Rgba,
}

impl HdrImage {
    /// Stride in `f16` units (channel count) — always 4.
    pub const STRIDE: usize = 4;

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
