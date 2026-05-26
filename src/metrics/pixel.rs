//! Pixel-pair metrics: MAE, MSE, RMSE, PSNR, max-abs, relative error,
//! log-luminance error, plus naive SSIM. All single-threaded CPU; phase 7
//! parallelizes via wasm-bindgen-rayon once COOP/COEP is set up.

use std::cmp::Ordering;

use crate::color::space::LUM_WEIGHTS_REC709;
use crate::io::HdrImage;

#[derive(Debug, Clone, Copy, Default)]
pub struct PixelMetrics {
    pub mae: f32,
    pub mse: f32,
    pub rmse: f32,
    pub psnr: f32,
    pub max_abs: f32,
    pub relative_error: f32,
    pub log_lum_rmse: f32,
    pub ssim: f32,
    pub pixels_compared: usize,
}

/// Compute all scalar metrics between two equally-sized images. Returns
/// `None` if dimensions don't match. Skips NaN/Inf pixels (counted into
/// `pixels_compared`).
pub fn compute(a: &HdrImage, b: &HdrImage) -> Option<PixelMetrics> {
    if a.width != b.width || a.height != b.height {
        return None;
    }
    let n = (a.width as usize) * (a.height as usize);
    let mut sum_abs = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut max_abs = 0.0_f32;
    let mut sum_rel = 0.0_f64;
    let mut sum_log_sq = 0.0_f64;
    let mut counted = 0_usize;
    let mut peak = 0.0_f32;

    for i in 0..n {
        let ai = i * 4;
        let bi = i * 4;
        let mut bad = false;
        for c in 0..3 {
            let av = a.data[ai + c].to_f32();
            let bv = b.data[bi + c].to_f32();
            if !av.is_finite() || !bv.is_finite() {
                bad = true;
                break;
            }
            let d = av - bv;
            sum_abs += d.abs() as f64;
            sum_sq += (d as f64) * (d as f64);
            if d.abs() > max_abs {
                max_abs = d.abs();
            }
            sum_rel += (d.abs() / (av.abs().max(bv.abs()).max(1e-4))) as f64;
            peak = peak.max(av.abs()).max(bv.abs());
        }
        if bad {
            continue;
        }
        // Log-luminance error (with eps).
        let la = LUM_WEIGHTS_REC709[0] * a.data[ai].to_f32()
            + LUM_WEIGHTS_REC709[1] * a.data[ai + 1].to_f32()
            + LUM_WEIGHTS_REC709[2] * a.data[ai + 2].to_f32();
        let lb = LUM_WEIGHTS_REC709[0] * b.data[bi].to_f32()
            + LUM_WEIGHTS_REC709[1] * b.data[bi + 1].to_f32()
            + LUM_WEIGHTS_REC709[2] * b.data[bi + 2].to_f32();
        let dl = (la.max(1e-4)).ln() - (lb.max(1e-4)).ln();
        sum_log_sq += (dl as f64) * (dl as f64);
        counted += 1;
    }

    let count3 = (counted * 3) as f64;
    let mae = (sum_abs / count3) as f32;
    let mse = (sum_sq / count3) as f32;
    let rmse = mse.sqrt();
    let relative_error = (sum_rel / count3) as f32;
    let log_lum_rmse = ((sum_log_sq / counted.max(1) as f64).sqrt()) as f32;

    // PSNR with peak = max observed signal (HDR-appropriate).
    let psnr = if mse > 0.0 && peak > 0.0 {
        20.0 * peak.log10() - 10.0 * mse.log10()
    } else {
        f32::INFINITY
    };

    // Naive global SSIM (single block; not the standard 11x11 Gaussian-windowed
    // SSIM, but a reasonable scalar summary for HDR). dssim-core lands in phase 6.
    let ssim = naive_ssim_luminance(a, b);

    Some(PixelMetrics {
        mae,
        mse,
        rmse,
        psnr,
        max_abs,
        relative_error,
        log_lum_rmse,
        ssim,
        pixels_compared: counted,
    })
}

fn naive_ssim_luminance(a: &HdrImage, b: &HdrImage) -> f32 {
    let n = (a.width as usize) * (a.height as usize);
    let mut mean_a = 0.0_f64;
    let mut mean_b = 0.0_f64;
    let mut counted = 0;
    let mut la_buf = Vec::with_capacity(n);
    let mut lb_buf = Vec::with_capacity(n);
    for i in 0..n {
        let ai = i * 4;
        let la = LUM_WEIGHTS_REC709[0] * a.data[ai].to_f32()
            + LUM_WEIGHTS_REC709[1] * a.data[ai + 1].to_f32()
            + LUM_WEIGHTS_REC709[2] * a.data[ai + 2].to_f32();
        let lb = LUM_WEIGHTS_REC709[0] * b.data[ai].to_f32()
            + LUM_WEIGHTS_REC709[1] * b.data[ai + 1].to_f32()
            + LUM_WEIGHTS_REC709[2] * b.data[ai + 2].to_f32();
        if la.is_finite() && lb.is_finite() {
            mean_a += la as f64;
            mean_b += lb as f64;
            la_buf.push(la);
            lb_buf.push(lb);
            counted += 1;
        }
    }
    if counted == 0 {
        return f32::NAN;
    }
    let mu_a = mean_a / counted as f64;
    let mu_b = mean_b / counted as f64;
    let mut var_a = 0.0_f64;
    let mut var_b = 0.0_f64;
    let mut cov = 0.0_f64;
    for i in 0..counted {
        let da = la_buf[i] as f64 - mu_a;
        let db = lb_buf[i] as f64 - mu_b;
        var_a += da * da;
        var_b += db * db;
        cov += da * db;
    }
    let inv = 1.0 / counted as f64;
    var_a *= inv;
    var_b *= inv;
    cov *= inv;
    let l_dyn = la_buf.iter().chain(lb_buf.iter()).cloned().fold(0.0_f32, |acc, v| acc.max(v));
    let c1 = (0.01 * l_dyn as f64).powi(2);
    let c2 = (0.03 * l_dyn as f64).powi(2);
    let num = (2.0 * mu_a * mu_b + c1) * (2.0 * cov + c2);
    let den = (mu_a * mu_a + mu_b * mu_b + c1) * (var_a + var_b + c2);
    if den.abs() < 1e-12 {
        return f32::NAN;
    }
    (num / den) as f32
}

/// Returns a vector of per-pixel absolute errors and the indices of the
/// top-K crops sorted by error. Used by the "top 1% error regions" feature.
pub fn top_error_crops(
    a: &HdrImage,
    b: &HdrImage,
    crop_size: u32,
    top_k: usize,
) -> Vec<((u32, u32), f32)> {
    if a.width != b.width || a.height != b.height {
        return Vec::new();
    }
    let cols = a.width / crop_size;
    let rows = a.height / crop_size;
    let mut crops = Vec::with_capacity((cols * rows) as usize);
    for cy in 0..rows {
        for cx in 0..cols {
            let mut acc = 0.0_f32;
            let mut n = 0;
            for dy in 0..crop_size {
                for dx in 0..crop_size {
                    let x = cx * crop_size + dx;
                    let y = cy * crop_size + dy;
                    let i = ((y * a.width + x) * 4) as usize;
                    for c in 0..3 {
                        let av = a.data[i + c].to_f32();
                        let bv = b.data[i + c].to_f32();
                        if av.is_finite() && bv.is_finite() {
                            acc += (av - bv).abs();
                            n += 1;
                        }
                    }
                }
            }
            crops.push(((cx * crop_size, cy * crop_size), acc / n.max(1) as f32));
        }
    }
    crops.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    crops.truncate(top_k);
    crops
}
