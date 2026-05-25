//! Histogram + percentile markers in log-luminance space.

use crate::color::space::LUM_WEIGHTS_REC709;
use crate::io::HdrImage;

#[derive(Debug, Clone)]
pub struct Histogram {
    /// One bin per stop, covering `min_ev..=max_ev`.
    pub bins: Vec<u32>,
    pub min_ev: f32,
    pub max_ev: f32,
    /// (p50, p90, p99, p999) in EV.
    pub percentiles: (f32, f32, f32, f32),
    /// Per-channel linear bins (256 bins, 0..max_value).
    pub rgb_bins: [Vec<u32>; 3],
    pub rgb_max: f32,
}

const N_BINS: usize = 128;
const N_RGB: usize = 256;

pub fn compute(img: &HdrImage, min_ev: f32, max_ev: f32) -> Histogram {
    let n = (img.width as usize) * (img.height as usize);
    let mut bins = vec![0u32; N_BINS];
    let mut rgb_bins = [vec![0u32; N_RGB], vec![0u32; N_RGB], vec![0u32; N_RGB]];
    let mut rgb_max = 0.0_f32;
    let mut all_lum = Vec::with_capacity(n.min(1 << 22));

    for i in 0..n {
        let ai = i * 4;
        let r = img.data[ai].to_f32();
        let g = img.data[ai + 1].to_f32();
        let b = img.data[ai + 2].to_f32();
        for (c, v) in [r, g, b].iter().enumerate() {
            if v.is_finite() && *v > rgb_max {
                rgb_max = *v;
            }
            let _ = c;
        }
        // Defer per-channel binning until we know the max.
        let lum =
            LUM_WEIGHTS_REC709[0] * r + LUM_WEIGHTS_REC709[1] * g + LUM_WEIGHTS_REC709[2] * b;
        if lum.is_finite() && lum > 0.0 {
            let ev = lum.log2();
            let t = ((ev - min_ev) / (max_ev - min_ev)).clamp(0.0, 1.0);
            let b = ((t * (N_BINS as f32 - 1.0)).round() as usize).min(N_BINS - 1);
            bins[b] += 1;
            if all_lum.len() < (1 << 22) {
                all_lum.push(ev);
            }
        }
    }

    let denom = rgb_max.max(1e-4);
    for i in 0..n {
        let ai = i * 4;
        let r = img.data[ai].to_f32();
        let g = img.data[ai + 1].to_f32();
        let b = img.data[ai + 2].to_f32();
        for (c, v) in [r, g, b].iter().enumerate() {
            if v.is_finite() {
                let t = (v / denom).clamp(0.0, 1.0);
                let bi = ((t * (N_RGB as f32 - 1.0)).round() as usize).min(N_RGB - 1);
                rgb_bins[c][bi] += 1;
            }
        }
    }

    let percentiles = if all_lum.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        all_lum.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pct = |p: f32| {
            let i = ((p * (all_lum.len() - 1) as f32).round() as usize).min(all_lum.len() - 1);
            all_lum[i]
        };
        (pct(0.50), pct(0.90), pct(0.99), pct(0.999))
    };

    Histogram { bins, min_ev, max_ev, percentiles, rgb_bins, rgb_max }
}
