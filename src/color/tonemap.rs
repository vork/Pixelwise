//! CPU reference implementations of tone-map operators. These match what the
//! GPU shader produces for the pixel-probe "display-referred" readout, i.e.
//! the value actually written to the framebuffer for an sRGB display.
//!
//! Linear / Reinhard / ACES / Piecewise return scene-linear [0,1] values
//! that need to be sRGB-encoded for the display. Hejl-Burgess Filmic and the
//! user Gamma curve already bake a display encoding in, so they skip the
//! extra sRGB step. The public functions return display-encoded values for
//! every variant so the probe matches the pixel.

#[inline]
pub fn linear_to_srgb_one(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.max(0.0).powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
fn encode_srgb(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(linear_to_srgb_one)
}

/// Raw scalar evaluators for the curve-preview UI. These return the
/// operator's intrinsic transformation only — no extra sRGB step applied to
/// Linear / Reinhard / ACES / Piecewise (the renderer adds that downstream).
/// Filmic-Hejl and Gamma already bake their encoding into the formula.
pub mod curve {
    use super::*;

    #[inline] pub fn linear(x: f32) -> f32 { x.clamp(0.0, 1.0) }

    #[inline] pub fn srgb(x: f32) -> f32 { linear_to_srgb_one(x.clamp(0.0, 1.0)) }

    #[inline]
    pub fn reinhard(x: f32, white: f32) -> f32 {
        let x = x.max(0.0);
        let w2 = (white * white).max(1e-6);
        x * (1.0 + x / w2) / (1.0 + x)
    }

    #[inline]
    pub fn aces(x: f32) -> f32 {
        // Per-channel ACES Fitted, identity matrix (since we plot scalar).
        let v = x.max(0.0);
        let a = v * (v + 0.0245786) - 0.000090537;
        let b = v * (0.983729 * v + 0.4329510) + 0.238081;
        (a / b).clamp(0.0, 1.0)
    }

    #[inline]
    pub fn filmic_hejl(x: f32) -> f32 {
        let v = (x - 0.004).max(0.0);
        (v * (6.2 * v + 0.5)) / (v * (6.2 * v + 1.7) + 0.06)
    }

    #[inline]
    pub fn gamma(x: f32, g: f32) -> f32 {
        x.max(0.0).powf(1.0 / g.max(0.01))
    }

    #[inline]
    pub fn hable(x: f32, pw: [f32; 4], white: f32) -> f32 {
        let (pw, w) = sanitize_piecewise(pw, white);
        let x0 = pw[0];
        let y0 = pw[1];
        let x1 = pw[2];
        let y1 = pw[3];
        let m = (y1 - y0) / (x1 - x0);
        let b = y0 - m * x0;
        let toe_b = (m * x0) / y0;
        let toe_a = y0 / x0.powf(toe_b);
        let shx = w - x1;
        let shy = 1.0 - y1;
        let sh_b = (m * shx) / shy;
        let sh_a = shy / shx.powf(sh_b);
        let xc = x.max(0.0);
        if xc < x0 {
            toe_a * xc.powf(toe_b)
        } else if xc < x1 {
            m * xc + b
        } else if xc < w {
            1.0 - sh_a * (w - xc).powf(sh_b)
        } else {
            1.0
        }
    }
}

/// Identity passthrough — clamp to [0,1], no display encoding. The viewer's
/// monitor still applies its sRGB curve, so this is a "raw" view (looks dark
/// and contrasty on an sRGB display, but truthful to the scene-linear data).
#[inline]
pub fn linear(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|c| c.clamp(0.0, 1.0))
}

/// Standard sRGB OETF — clamp to [0,1] then apply the official sRGB encoding.
/// Use this when you want "linear data presented correctly on an sRGB display"
/// without any tone curve.
#[inline]
pub fn srgb(rgb: [f32; 3]) -> [f32; 3] {
    encode_srgb(rgb.map(|c| c.clamp(0.0, 1.0)))
}

#[inline]
pub fn reinhard(rgb: [f32; 3], white: f32) -> [f32; 3] {
    let w2 = (white * white).max(1e-6);
    encode_srgb(rgb.map(|c| c * (1.0 + c / w2) / (1.0 + c)))
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
    encode_srgb([
        (1.60475 * r3 + -0.53108 * g3 + -0.07367 * b3).clamp(0.0, 1.0),
        (-0.10208 * r3 + 1.10813 * g3 + -0.00605 * b3).clamp(0.0, 1.0),
        (-0.00327 * r3 + -0.07276 * g3 + 1.07602 * b3).clamp(0.0, 1.0),
    ])
}

/// Hejl-Burgess "Filmic" — cheap, no shoulder. Already display-encoded.
#[inline]
pub fn filmic_hejl(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|c| {
        let x = (c - 0.004).max(0.0);
        (x * (6.2 * x + 0.5)) / (x * (6.2 * x + 1.7) + 0.06)
    })
}

/// User-controlled display gamma encoding. Already display-encoded.
#[inline]
pub fn gamma(rgb: [f32; 3], g: f32) -> [f32; 3] {
    rgb.map(|c| c.max(0.0).powf(1.0 / g.max(0.01)))
}

/// Convert Hable's user-facing knobs to the four segment knots and white
/// point that drive the piecewise power curve. Port of his
/// `CalcDirectParamsFromUser`. Each input is independently bounded — the
/// resulting (x0, y0, x1, y1, W) tuple is always well-formed.
#[inline]
pub fn piecewise_segments_from_user(
    toe_strength: f32,
    toe_length: f32,
    shoulder_strength: f32,
    shoulder_length: f32,
) -> ([f32; 4], f32) {
    let ts = toe_strength.clamp(0.0, 1.0);
    let tl = toe_length.clamp(0.0, 1.0);
    let ss = shoulder_strength.max(0.0);
    let sl = shoulder_length.clamp(0.0, 1.0);

    let x0 = tl * 0.5;
    let y0 = (1.0 - ts) * x0;
    let remaining_y = (1.0 - y0).max(1e-6);
    let initial_w = x0 + remaining_y;
    let y1_offset = (1.0 - sl) * remaining_y;
    let x1 = x0 + y1_offset;
    let y1 = y0 + y1_offset;
    let extra_w = (ss).exp2() - 1.0;
    let w = initial_w + extra_w;
    ([x0, y0, x1, y1], w)
}

/// Clamp the user-tunable piecewise parameters so that the resulting curve
/// stays well-formed (strictly increasing, no division by zero, all three
/// sections have positive width, shoulder has room to reach 1). Mirrors the
/// guard in the WGSL implementation so CPU readout matches GPU pixels.
#[inline]
fn sanitize_piecewise(pw: [f32; 4], white: f32) -> ([f32; 4], f32) {
    const EPS: f32 = 1e-3;
    let mut w = white.max(1.0 + EPS);
    // x0 must leave room for both the linear and shoulder sections.
    let mut x0 = pw[0].clamp(EPS, w - 2.0 * EPS);
    // x1 in (x0, white) with at least a tiny linear/shoulder gap.
    let mut x1 = pw[2].clamp(x0 + EPS, w - EPS);
    // y0 in (0, x0] — keeps the toe sub-linear (saturating from 0).
    let mut y0 = pw[1].clamp(EPS, x0.max(EPS));
    // y1 in (y0, 1) — strictly above y0 and strictly below 1 so the shoulder
    // can still climb to display white.
    let mut y1 = pw[3].clamp(y0 + EPS, 1.0 - EPS);
    // After clamping y1 we may have violated y0<y1; nudge y0 down if so.
    if y0 >= y1 {
        y0 = (y1 - EPS).max(EPS);
    }
    // Same for x: if the clamps collapse, push white out.
    if x1 <= x0 {
        x1 = (x0 + EPS).min(w - EPS);
    }
    if w <= x1 {
        w = x1 + EPS;
    }
    ([x0, y0, x1, y1], w)
}

/// John Hable's piecewise power-curve filmic. Three sections — toe / linear /
/// shoulder — joined with C¹ continuity.
/// `pw = (toe_x, toe_y, shoulder_x, shoulder_y)`, `white` = upper crossover.
#[inline]
pub fn hable_piecewise(rgb: [f32; 3], pw: [f32; 4], white: f32) -> [f32; 3] {
    let (pw, white) = sanitize_piecewise(pw, white);
    let x0 = pw[0];
    let y0 = pw[1];
    let x1 = pw[2];
    let y1 = pw[3];
    let m = (y1 - y0) / (x1 - x0);
    let b = y0 - m * x0;
    let toe_b = (m * x0) / y0;
    let toe_a = y0 / x0.powf(toe_b);
    let shx = white - x1;
    let shy = 1.0 - y1;
    let sh_b = (m * shx) / shy;
    let sh_a = shy / shx.powf(sh_b);
    encode_srgb(rgb.map(|c| {
        let x = c.max(0.0);
        if x < x0 {
            toe_a * x.powf(toe_b)
        } else if x < x1 {
            m * x + b
        } else if x < white {
            1.0 - sh_a * (white - x).powf(sh_b)
        } else {
            1.0
        }
    }))
}
