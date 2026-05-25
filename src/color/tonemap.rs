//! CPU reference implementations of tone-map operators. The GPU shader is
//! the source of truth for the on-screen image; these exist for testing
//! and for the pixel-probe "display-referred" readout.

#[inline]
pub fn linear(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|c| c.clamp(0.0, 1.0))
}

#[inline]
pub fn reinhard(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|c| c / (1.0 + c))
}

/// ACES Fitted (Stephen Hill).
#[inline]
pub fn aces_fitted(rgb: [f32; 3]) -> [f32; 3] {
    fn rrt_and_odt(v: f32) -> f32 {
        let a = v * (v + 0.0245786) - 0.000090537;
        let b = v * (0.983729 * v + 0.4329510) + 0.238081;
        a / b
    }
    let [r, g, b] = rgb;
    let r2 = 0.59719 * r + 0.35458 * g + 0.04823 * b;
    let g2 = 0.07600 * r + 0.90834 * g + 0.01566 * b;
    let b2 = 0.02840 * r + 0.13383 * g + 0.83777 * b;
    let r3 = rrt_and_odt(r2);
    let g3 = rrt_and_odt(g2);
    let b3 = rrt_and_odt(b2);
    [
        (1.60475 * r3 + -0.53108 * g3 + -0.07367 * b3).clamp(0.0, 1.0),
        (-0.10208 * r3 + 1.10813 * g3 + -0.00605 * b3).clamp(0.0, 1.0),
        (-0.00327 * r3 + -0.07276 * g3 + 1.07602 * b3).clamp(0.0, 1.0),
    ]
}

/// Hejl-Burgess "Filmic" — cheap, no shoulder.
#[inline]
pub fn filmic_hejl(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|c| {
        let x = (c - 0.004).max(0.0);
        (x * (6.2 * x + 0.5)) / (x * (6.2 * x + 1.7) + 0.06)
    })
}

#[inline]
pub fn gamma(rgb: [f32; 3], g: f32) -> [f32; 3] {
    rgb.map(|c| c.max(0.0).powf(1.0 / g))
}
