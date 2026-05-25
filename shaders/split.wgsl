// Side-by-side split / onion-skin / checkerboard compare. Two textures, one
// uniform shared with display.wgsl plus a couple of compare-specific fields.

// {{COMMON}}

struct DisplayParams {
    uv_xform: vec4<f32>,
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
    /// 0 = split, 1 = onion, 2 = checkerboard, 3 = abs diff, 4 = signed diff
    mode: u32,
    /// split position 0..1 OR onion alpha 0..1
    blend: f32,
    diff_scale: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> params: DisplayParams;
@group(0) @binding(1) var t_a: texture_2d<f32>;
@group(0) @binding(2) var t_b: texture_2d<f32>;
@group(0) @binding(3) var s_linear: sampler;
@group(0) @binding(4) var s_nearest: sampler;
@group(0) @binding(5) var<uniform> cmp: CompareParams;
@group(0) @binding(6) var t_ramp: texture_1d<f32>;
@group(0) @binding(7) var s_ramp: sampler;

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

fn process(c: vec4<f32>, in_uv_px: vec2<f32>) -> vec3<f32> {
    var lin = c.rgb * exp2(params.exposure);
    var rgb = apply_channel(vec4<f32>(lin, c.a), params.channel, t_ramp, s_ramp);
    if (params.output_is_hdr == 0u) {
        rgb = tonemap(rgb, params.tonemap);
        rgb = linear_to_srgb(rgb);
    }
    return rgb;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
        let cell = floor(in.pos.xy / 24.0);
        let s = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) * 0.5);
        let bg = mix(vec3<f32>(0.07, 0.07, 0.10), vec3<f32>(0.10, 0.10, 0.13), s);
        return vec4<f32>(bg, 1.0);
    }
    let d = max(length(dpdx(in.uv_px)), length(dpdy(in.uv_px)));
    let samp_a = select(textureSample(t_a, s_nearest, uv),
                        textureSample(t_a, s_linear, uv), d > 0.16);
    let samp_b = select(textureSample(t_b, s_nearest, uv),
                        textureSample(t_b, s_linear, uv), d > 0.16);

    let rgb_a = process(samp_a, in.uv_px);
    let rgb_b = process(samp_b, in.uv_px);

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
        let d3 = abs(samp_a.rgb - samp_b.rgb) * cmp.diff_scale;
        let l = luminance(d3);
        out = textureSampleLevel(t_ramp, s_ramp, clamp(l, 0.0, 1.0), 0.0).rgb;
    } else {
        // signed diff: red = b > a, blue = a > b, brightness = magnitude
        let d3 = (samp_b.rgb - samp_a.rgb) * cmp.diff_scale;
        let pos = max(luminance(d3), 0.0);
        let neg = max(-luminance(d3), 0.0);
        out = vec3<f32>(pos, 0.0, neg);
    }

    return vec4<f32>(out, 1.0);
}
