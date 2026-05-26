//! Per-image sanity statistics: NaN/Inf/negative counts, min/max, mean.

use crate::color::space::LUM_WEIGHTS_REC709;
use crate::io::HdrImage;

#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub nan_pixels: u32,
    pub inf_pixels: u32,
    pub negative_pixels: u32,
    pub min_rgb: [f32; 3],
    pub max_rgb: [f32; 3],
    pub mean_rgb: [f32; 3],
    pub mean_log_lum: f32,
    pub pixels: u32,
}

pub fn compute(img: &HdrImage) -> Stats {
    let n = (img.width as usize) * (img.height as usize);
    let mut nan = 0_u32;
    let mut inf = 0_u32;
    let mut neg = 0_u32;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut sum = [0.0_f64; 3];
    let mut sum_log = 0.0_f64;
    let mut log_n = 0_u32;
    for i in 0..n {
        let ai = i * 4;
        let mut has_nan = false;
        let mut has_inf = false;
        let mut has_neg = false;
        for c in 0..3 {
            let v = img.data[ai + c].to_f32();
            if v.is_nan() {
                has_nan = true;
                continue;
            }
            if v.is_infinite() {
                has_inf = true;
                continue;
            }
            if v < 0.0 {
                has_neg = true;
            }
            if v < min[c] {
                min[c] = v;
            }
            if v > max[c] {
                max[c] = v;
            }
            sum[c] += v as f64;
        }
        if has_nan {
            nan += 1;
        }
        if has_inf {
            inf += 1;
        }
        if has_neg {
            neg += 1;
        }
        if !has_nan && !has_inf {
            let r = img.data[ai].to_f32();
            let g = img.data[ai + 1].to_f32();
            let b = img.data[ai + 2].to_f32();
            let l =
                LUM_WEIGHTS_REC709[0] * r + LUM_WEIGHTS_REC709[1] * g + LUM_WEIGHTS_REC709[2] * b;
            if l > 0.0 {
                sum_log += (l.ln()) as f64;
                log_n += 1;
            }
        }
    }
    let mean = [
        (sum[0] / n as f64) as f32,
        (sum[1] / n as f64) as f32,
        (sum[2] / n as f64) as f32,
    ];
    Stats {
        nan_pixels: nan,
        inf_pixels: inf,
        negative_pixels: neg,
        min_rgb: min,
        max_rgb: max,
        mean_rgb: mean,
        mean_log_lum: if log_n > 0 {
            (sum_log / log_n as f64) as f32
        } else {
            0.0
        },
        pixels: n as u32,
    }
}
