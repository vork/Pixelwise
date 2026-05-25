// Shared helpers: tone mapping, channel isolation, transfer functions.
// Included by display.wgsl, split.wgsl, diff.wgsl via concat at build time.

const TM_LINEAR: u32   = 0u;
const TM_REINHARD: u32 = 1u;
const TM_ACES: u32     = 2u;
const TM_FILMIC: u32   = 3u;
const TM_GAMMA: u32    = 4u;

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

fn tonemap_reinhard(c: vec3<f32>) -> vec3<f32> {
    return c / (vec3<f32>(1.0) + c);
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

fn tonemap(c: vec3<f32>, mode: u32) -> vec3<f32> {
    if (mode == TM_REINHARD) { return tonemap_reinhard(c); }
    if (mode == TM_ACES)     { return tonemap_aces(c); }
    if (mode == TM_FILMIC)   { return tonemap_filmic(c); }
    if (mode == TM_GAMMA)    { return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)); }
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
