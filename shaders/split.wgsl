// Side-by-side split / onion-skin / checkerboard compare. Two textures, one
// uniform shared with display.wgsl plus a couple of compare-specific fields.

// {{COMMON}}

struct DisplayParams {
    uv_xform: vec4<f32>,
    piecewise: vec4<f32>,
    tm_extras: vec4<f32>,
    exposure: f32,
    tonemap: u32,
    channel: u32,
    clip_flags: u32,
    output_is_hdr: u32,
    width: u32,
    height: u32,
    false_color_min: f32,
    false_color_max: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

struct CompareParams {
    /// 0 = split, 1 = onion, 3 = diff (formula chosen by diff_mode below).
    mode: u32,
    /// split position 0..1 OR onion alpha 0..1
    blend: f32,
    diff_scale: f32,
    /// DiffMode discriminant: 0 abs, 1 signed, 2 squared, 3 log, 4 rel, 5 rel²
    diff_mode: u32,
};

@group(0) @binding(0) var<uniform> params: DisplayParams;
@group(0) @binding(1) var t_a: texture_2d<f32>;
@group(0) @binding(2) var t_b: texture_2d<f32>;
@group(0) @binding(3) var s_linear: sampler;
@group(0) @binding(4) var s_nearest: sampler;
@group(0) @binding(5) var<uniform> cmp: CompareParams;
@group(0) @binding(6) var t_ramp: texture_1d<f32>;
@group(0) @binding(7) var s_ramp: sampler;
@group(1) @binding(0) var t_lut: texture_3d<f32>;
@group(1) @binding(1) var s_lut: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) uv_px: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) idx: u32) -> VsOut {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let xy = p[idx];
    var out: VsOut;
    out.pos = vec4<f32>(xy, 0.0, 1.0);
    let uv = vec2<f32>(xy.x * params.uv_xform.x, -xy.y * params.uv_xform.y) * 0.5 + vec2<f32>(0.5);
    out.uv = uv + params.uv_xform.zw;
    out.uv_px = out.uv * vec2<f32>(f32(params.width), f32(params.height));
    return out;
}

// Pre-LUT pipeline: scene-linear sample → exposure → channel → tonemap.
// Returns the value that should be fed into the LUT (or shown directly when
// the canvas is HDR and we want to keep scene-linear values).
fn prepare_pre_lut(c: vec4<f32>) -> vec3<f32> {
    let nan = any(c.rgb != c.rgb);
    let inf = is_inf(c.r) || is_inf(c.g) || is_inf(c.b);
    var lin = c.rgb;
    if (!nan && !inf) {
        lin = lin * exp2(params.exposure);
    }
    var rgb = apply_channel(vec4<f32>(lin, c.a), params.channel, t_ramp, s_ramp);
    if (params.output_is_hdr == 0u) {
        rgb = tonemap_to_display(rgb, params.tonemap, params.piecewise, params.tm_extras);
    }
    return rgb;
}

fn finish(c: vec4<f32>, in_uv_px: vec2<f32>, lut_out: vec3<f32>, pre_lut: vec3<f32>) -> vec3<f32> {
    let lin = c.rgb * exp2(params.exposure);
    var rgb = pre_lut;
    if (params.output_is_hdr == 0u) {
        rgb = lut_out;
    }
    return apply_clipping(rgb, lin, in_uv_px, params.clip_flags);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // Derivative & texture samples must run in uniform control flow.
    let d = max(length(dpdx(in.uv_px)), length(dpdy(in.uv_px)));
    let a_nearest = textureSample(t_a, s_nearest, uv);
    let a_linear  = textureSample(t_a, s_linear, uv);
    let b_nearest = textureSample(t_b, s_nearest, uv);
    let b_linear  = textureSample(t_b, s_linear, uv);
    let samp_a = select(a_nearest, a_linear, d > 0.16);
    let samp_b = select(b_nearest, b_linear, d > 0.16);

    // Tonemap each side then LUT lookup — all in uniform control flow so
    // textureSample(t_lut, ...) is legal.
    let pre_lut_a = prepare_pre_lut(samp_a);
    let pre_lut_b = prepare_pre_lut(samp_b);
    let lut_a = textureSample(t_lut, s_lut, clamp(pre_lut_a, vec3<f32>(0.0), vec3<f32>(1.0))).rgb;
    let lut_b = textureSample(t_lut, s_lut, clamp(pre_lut_b, vec3<f32>(0.0), vec3<f32>(1.0))).rgb;

    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
        let cell = floor(in.pos.xy / 24.0);
        let s = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) * 0.5);
        let bg = mix(vec3<f32>(0.07, 0.07, 0.10), vec3<f32>(0.10, 0.10, 0.13), s);
        return vec4<f32>(bg, 1.0);
    }
    let rgb_a = finish(samp_a, in.uv_px, lut_a, pre_lut_a);
    let rgb_b = finish(samp_b, in.uv_px, lut_b, pre_lut_b);

    var out: vec3<f32>;
    if (cmp.mode == 0u) {
        let use_b = step(cmp.blend, in.uv.x);
        out = mix(rgb_a, rgb_b, use_b);
        // 1px brand-gradient seam
        let dx = abs(in.uv.x - cmp.blend);
        if (dx < (1.0 / max(f32(params.width), 1.0)) * 1.5) {
            let g = mix(vec3<f32>(0.866, 0.525, 1.0), vec3<f32>(0.968, 0.647, 0.260), in.uv.y);
            out = g;
        }
    } else if (cmp.mode == 1u) {
        out = mix(rgb_a, rgb_b, cmp.blend);
    } else if (cmp.mode == 2u) {
        let cell = floor(in.uv_px / 24.0);
        let s = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) * 0.5);
        out = mix(rgb_a, rgb_b, s);
    } else if (cmp.mode == 3u) {
        // Per-pixel error visualization. The exact formula is chosen by
        // diff_mode and matches Framewise's MediaEngine.ErrorMetric set.
        let eps = vec3<f32>(0.01);
        let diff = samp_a.rgb - samp_b.rgb;
        var d3: vec3<f32>;
        if (cmp.diff_mode == 1u) {
            // Signed: red = b > a, blue = a > b, brightness = magnitude.
            let s = diff * cmp.diff_scale;
            let pos = max(luminance(s), 0.0);
            let neg = max(-luminance(s), 0.0);
            out = vec3<f32>(pos, 0.0, neg);
            // Skip the colorize-via-ramp branch below.
            out = apply_pixel_grid(out, in.uv, params.width, params.height, d);
            return vec4<f32>(out, 1.0);
        } else if (cmp.diff_mode == 2u) {
            d3 = diff * diff * cmp.diff_scale;
        } else if (cmp.diff_mode == 3u) {
            // Log-luminance: |log10(|a|+ε) − log10(|b|+ε)|
            let la = log(abs(samp_a.rgb) + vec3<f32>(0.001));
            let lb = log(abs(samp_b.rgb) + vec3<f32>(0.001));
            d3 = abs(la - lb) * (cmp.diff_scale / log(10.0));
        } else if (cmp.diff_mode == 4u) {
            d3 = abs(diff) / (abs(samp_b.rgb) + eps) * cmp.diff_scale;
        } else if (cmp.diff_mode == 5u) {
            d3 = (diff * diff) / (samp_b.rgb * samp_b.rgb + eps) * cmp.diff_scale;
        } else {
            // 0 = absolute
            d3 = abs(diff) * cmp.diff_scale;
        }
        let l = luminance(d3);
        out = textureSampleLevel(t_ramp, s_ramp, clamp(l, 0.0, 1.0), 0.0).rgb;
    } else {
        out = mix(rgb_a, rgb_b, 0.5);
    }

    out = apply_pixel_grid(out, in.uv, params.width, params.height, d);
    return vec4<f32>(out, 1.0);
}
