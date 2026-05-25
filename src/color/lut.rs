//! False-color LUT generation. We bake a Viridis ramp into a 1D texture (via
//! a 256x1x1 RGBA8 buffer) and let the shader sample it.

/// A reasonable HDR-friendly viridis-style 7-stop ramp.
const STOPS: &[[f32; 3]] = &[
    [0.0, 0.0, 0.0],          // black
    [0.10, 0.10, 0.40],       // deep blue
    [0.10, 0.40, 0.70],       // blue
    [0.10, 0.70, 0.50],       // cyan-green
    [0.50, 0.85, 0.20],       // green-yellow
    [1.00, 0.80, 0.10],       // yellow
    [1.00, 0.30, 0.10],       // orange-red
    [1.00, 0.95, 0.90],       // hot white
];

pub fn viridis_ramp(n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let pos = t * (STOPS.len() - 1) as f32;
        let lo = pos.floor() as usize;
        let hi = (lo + 1).min(STOPS.len() - 1);
        let f = pos - lo as f32;
        let a = STOPS[lo];
        let b = STOPS[hi];
        let r = a[0] + (b[0] - a[0]) * f;
        let g = a[1] + (b[1] - a[1]) * f;
        let bl = a[2] + (b[2] - a[2]) * f;
        out.push((r.clamp(0.0, 1.0) * 255.0) as u8);
        out.push((g.clamp(0.0, 1.0) * 255.0) as u8);
        out.push((bl.clamp(0.0, 1.0) * 255.0) as u8);
        out.push(255);
    }
    out
}
