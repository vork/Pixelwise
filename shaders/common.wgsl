// Shared helpers: tone mapping, channel isolation, transfer functions.
// Included by display.wgsl, split.wgsl, diff.wgsl via concat at build time.

const TM_LINEAR: u32   = 0u;
const TM_REINHARD: u32 = 1u;
const TM_ACES: u32     = 2u;
const TM_FILMIC: u32   = 3u;
const TM_GAMMA: u32    = 4u;
const TM_HABLE: u32    = 5u;
const TM_SRGB: u32     = 6u;

const CH_RGB: u32      = 0u;
const CH_R: u32        = 1u;
const CH_G: u32        = 2u;
const CH_B: u32        = 3u;
const CH_A: u32        = 4u;
const CH_LUM: u32      = 5u;
const CH_LOGLUM: u32   = 6u;
const CH_SAT: u32      = 7u;
const CH_HUE: u32      = 8u;
const CH_FALSE: u32    = 9u;

const CLIP_OVER: u32   = 1u;
const CLIP_UNDER: u32  = 2u;
const CLIP_NEG: u32    = 4u;
const CLIP_NAN: u32    = 8u;
const CLIP_INF: u32    = 16u;
const CLIP_OOG: u32    = 32u;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// WGSL disallows `1.0/0.0` as a literal, so detect ±Inf via the IEEE-754 bit
// pattern: exponent all-ones, mantissa zero.
fn is_inf(x: f32) -> bool {
    return (bitcast<u32>(x) & 0x7FFFFFFFu) == 0x7F800000u;
}

fn hsv(c: vec3<f32>) -> vec3<f32> {
    let mx = max(c.r, max(c.g, c.b));
    let mn = min(c.r, min(c.g, c.b));
    let d = mx - mn;
    var h: f32 = 0.0;
    if (d > 1e-6) {
        if (mx == c.r)      { h = (c.g - c.b) / d + select(0.0, 6.0, c.g < c.b); }
        else if (mx == c.g) { h = (c.b - c.r) / d + 2.0; }
        else                { h = (c.r - c.g) / d + 4.0; }
        h = h / 6.0;
    }
    let s = select(0.0, d / max(mx, 1e-6), mx > 1e-6);
    return vec3<f32>(h, s, mx);
}

fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let k = vec3<f32>(h * 6.0 + 0.0, h * 6.0 + 4.0, h * 6.0 + 2.0);
    let f = fract(k / 6.0) * 6.0;
    let r = clamp(abs(f - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
    return v * mix(vec3<f32>(1.0), r, vec3<f32>(s));
}

// Extended Reinhard with a configurable white point W: pixels at the white
// point map to 1.0 and the curve passes through (0, 0) with slope 1.
fn tonemap_reinhard(c: vec3<f32>, w: f32) -> vec3<f32> {
    let w2 = max(w * w, 1e-6);
    return c * (vec3<f32>(1.0) + c / w2) / (vec3<f32>(1.0) + c);
}

fn tonemap_aces(c: vec3<f32>) -> vec3<f32> {
    let m1 = mat3x3<f32>(
        vec3<f32>(0.59719, 0.07600, 0.02840),
        vec3<f32>(0.35458, 0.90834, 0.13383),
        vec3<f32>(0.04823, 0.01566, 0.83777),
    );
    let m2 = mat3x3<f32>(
        vec3<f32>( 1.60475, -0.10208, -0.00327),
        vec3<f32>(-0.53108,  1.10813, -0.07276),
        vec3<f32>(-0.07367, -0.00605,  1.07602),
    );
    let v = m1 * c;
    let a = v * (v + vec3<f32>(0.0245786)) - vec3<f32>(0.000090537);
    let b = v * (0.983729 * v + vec3<f32>(0.4329510)) + vec3<f32>(0.238081);
    return clamp(m2 * (a / b), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap_filmic(c: vec3<f32>) -> vec3<f32> {
    let x = max(c - vec3<f32>(0.004), vec3<f32>(0.0));
    return (x * (6.2 * x + vec3<f32>(0.5))) / (x * (6.2 * x + vec3<f32>(1.7)) + vec3<f32>(0.06));
}

// John Hable's piecewise power-curve filmic. Toe / linear / shoulder joined
// with C¹ continuity. See filmicworlds.com/blog/filmic-tonemapping-with-piecewise-power-curves.
// pw = (toe_x, toe_y, shoulder_x, shoulder_y), w = white point.

// Clamp parameters so the three sections always have positive width, the
// curve is strictly increasing, and divisions never blow up. Mirrors the
// CPU `sanitize_piecewise` helper so probe values match the rendered pixel.
fn piecewise_sanitize(pw: vec4<f32>, w_in: f32) -> vec4<f32> {
    let eps = 1e-3;
    var w = max(w_in, 1.0 + eps);
    var x0 = clamp(pw.x, eps, w - 2.0 * eps);
    var x1 = clamp(pw.z, x0 + eps, w - eps);
    var y0 = clamp(pw.y, eps, max(x0, eps));
    var y1 = clamp(pw.w, y0 + eps, 1.0 - eps);
    if (y0 >= y1) { y0 = max(y1 - eps, eps); }
    return vec4<f32>(x0, y0, x1, y1);
}

fn hable_piecewise_scalar(xin: f32, pw_in: vec4<f32>, w_in: f32) -> f32 {
    let w = max(w_in, 1.0 + 1e-3);
    let pw = piecewise_sanitize(pw_in, w);
    let x0 = pw.x;
    let y0 = pw.y;
    let x1 = pw.z;
    let y1 = pw.w;
    let m  = (y1 - y0) / (x1 - x0);
    let b  = y0 - m * x0;
    let toe_b = (m * x0) / y0;
    let toe_a = y0 / pow(x0, toe_b);
    let shx = w - x1;
    let shy = 1.0 - y1;
    let sh_b = (m * shx) / shy;
    let sh_a = shy / pow(shx, sh_b);
    let x = max(xin, 0.0);
    if (x < x0) { return toe_a * pow(x, toe_b); }
    if (x < x1) { return m * x + b; }
    if (x < w)  { return 1.0 - sh_a * pow(w - x, sh_b); }
    return 1.0;
}

fn tonemap_hable(c: vec3<f32>, pw: vec4<f32>, w: f32) -> vec3<f32> {
    return vec3<f32>(
        hable_piecewise_scalar(c.r, pw, w),
        hable_piecewise_scalar(c.g, pw, w),
        hable_piecewise_scalar(c.b, pw, w),
    );
}

// Tone-map then encode for an sRGB display. Some operators already bake the
// display encoding into their formula (Hejl-Burgess and user-gamma); those
// branches must NOT have linear_to_srgb applied a second time. Linear is
// intentionally raw — values are clamped and written straight through, with
// no display encoding, so users get an honest look at scene-linear data.
//
// extras = (piecewise_white, reinhard_white, gamma, _pad).
fn tonemap_to_display(c: vec3<f32>, mode: u32, piecewise: vec4<f32>, extras: vec4<f32>) -> vec3<f32> {
    if (mode == TM_SRGB)     { return linear_to_srgb(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0))); }
    if (mode == TM_REINHARD) { return linear_to_srgb(tonemap_reinhard(c, extras.y)); }
    if (mode == TM_ACES)     { return linear_to_srgb(tonemap_aces(c)); }
    if (mode == TM_FILMIC)   { return tonemap_filmic(c); }
    if (mode == TM_HABLE)    { return linear_to_srgb(tonemap_hable(c, piecewise, extras.x)); }
    if (mode == TM_GAMMA)    {
        let g = max(extras.z, 0.01);
        return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / g));
    }
    // TM_LINEAR (and any unknown mode): raw clamped, no gamma applied.
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let low  = 12.92 * c;
    let high = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(high, low, c <= vec3<f32>(0.0031308));
}

fn apply_channel(orig: vec4<f32>, mode: u32, ramp: texture_1d<f32>, ramp_s: sampler) -> vec3<f32> {
    if (mode == CH_R)      { return vec3<f32>(orig.r, 0.0, 0.0); }
    if (mode == CH_G)      { return vec3<f32>(0.0, orig.g, 0.0); }
    if (mode == CH_B)      { return vec3<f32>(0.0, 0.0, orig.b); }
    if (mode == CH_A)      { return vec3<f32>(orig.a); }
    if (mode == CH_LUM)    { return vec3<f32>(luminance(orig.rgb)); }
    if (mode == CH_LOGLUM) {
        let l = max(luminance(orig.rgb), 1e-6);
        let v = clamp((log2(l) + 8.0) / 16.0, 0.0, 1.0);
        return vec3<f32>(v);
    }
    if (mode == CH_SAT) {
        let s = hsv(orig.rgb);
        return vec3<f32>(s.y);
    }
    if (mode == CH_HUE) {
        let s = hsv(orig.rgb);
        return hsv2rgb(s.x, 0.85, 0.85);
    }
    if (mode == CH_FALSE) {
        let l = luminance(orig.rgb);
        let t = clamp(l, 0.0, 1.0);
        return textureSampleLevel(ramp, ramp_s, t, 0.0).rgb;
    }
    return orig.rgb;
}

fn checker(uv_px: vec2<f32>, size: f32, col: vec3<f32>) -> vec3<f32> {
    let cell = floor(uv_px / size);
    let s = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) * 0.5);
    return mix(col, col * 0.5, s);
}

fn stripes(uv_px: vec2<f32>, period: f32, col: vec3<f32>) -> vec3<f32> {
    let s = fract((uv_px.x + uv_px.y) / period);
    return mix(col, col * 0.3, step(0.5, s));
}

// Draw image-pixel grid lines on top of an already-display-encoded color.
// Picks a contrast color from the fragment brightness so the lines stay
// visible against any background (Framewise-style). Auto-fades out when the
// zoom isn't high enough for the grid to make sense.
//
// `footprint` is image-pixels-per-screen-pixel, computed by the caller in
// UNIFORM control flow so the WGSL `dpdx`/`dpdy` rules are satisfied.
fn apply_pixel_grid(
    rgb_in: vec3<f32>,
    uv: vec2<f32>,
    width: u32,
    height: u32,
    footprint: f32,
) -> vec3<f32> {
    if (footprint > 0.08) {
        return rgb_in;
    }
    let uv_px = uv * vec2<f32>(f32(width), f32(height));
    let cell = fract(uv_px);
    let edge_img = min(cell, vec2<f32>(1.0) - cell);
    let edge_px = min(edge_img.x, edge_img.y) / max(footprint, 1e-6);
    if (edge_px > 1.5) {
        return rgb_in;
    }
    let bright = (rgb_in.r + rgb_in.g + rgb_in.b) / 3.0;
    let grid_color = select(vec3<f32>(1.0), vec3<f32>(0.0), bright > 0.5);
    let line = 1.0 - smoothstep(0.0, 1.5, edge_px);
    let zoom_fade = 1.0 - smoothstep(0.04, 0.08, footprint);
    return mix(rgb_in, grid_color, line * 0.7 * zoom_fade);
}

// Apply NaN/Inf/Neg/Over/Under clipping overlays on top of an already-display-
// encoded RGB triple. `raw_lin` is the post-exposure scene-linear value (used
// for the over/under tests). Returns the overlay-modified RGB.
fn apply_clipping(
    rgb_in: vec3<f32>,
    raw_lin: vec3<f32>,
    uv_px: vec2<f32>,
    clip_flags: u32,
) -> vec3<f32> {
    let nan = any(raw_lin != raw_lin);
    let inf = is_inf(raw_lin.r) || is_inf(raw_lin.g) || is_inf(raw_lin.b);
    let neg = any(raw_lin < vec3<f32>(0.0));
    let raw_over = any(raw_lin > vec3<f32>(1.0));
    let raw_under = all(raw_lin < vec3<f32>(0.001));
    var rgb = rgb_in;
    if ((clip_flags & CLIP_NAN) != 0u && nan) {
        rgb = vec3<f32>(1.0, 0.0, 1.0);
    } else if ((clip_flags & CLIP_INF) != 0u && inf) {
        rgb = vec3<f32>(0.0, 1.0, 1.0);
    } else if ((clip_flags & CLIP_NEG) != 0u && neg) {
        rgb = stripes(uv_px, 6.0, vec3<f32>(1.0, 0.2, 0.2));
    } else {
        if ((clip_flags & CLIP_OVER) != 0u && raw_over) {
            rgb = checker(uv_px, 6.0, vec3<f32>(1.0, 0.9, 0.1));
        }
        if ((clip_flags & CLIP_UNDER) != 0u && raw_under) {
            rgb = checker(uv_px, 6.0, vec3<f32>(0.2, 0.4, 1.0));
        }
    }
    return rgb;
}
