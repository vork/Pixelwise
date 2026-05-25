// Display "uber" shader: fullscreen pass for single-image inspect mode.
//
// Inputs:
//   group 0 binding 0: DisplayParams (uniform)
//   group 0 binding 1: t_image (texture_2d<f32>, rgba16float)
//   group 0 binding 2: s_linear (linear sampler, with mipmaps)
//   group 0 binding 3: s_nearest (nearest sampler, no filtering)
//   group 0 binding 4: t_ramp (texture_1d<f32>, false-color LUT)
//   group 0 binding 5: s_ramp (linear sampler)

// {{COMMON}}

struct DisplayParams {
    uv_xform: vec4<f32>,        // xy = scale, zw = offset (clip -> image UV)
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

@group(0) @binding(0) var<uniform> params: DisplayParams;
@group(0) @binding(1) var t_image: texture_2d<f32>;
@group(0) @binding(2) var s_linear: sampler;
@group(0) @binding(3) var s_nearest: sampler;
@group(0) @binding(4) var t_ramp: texture_1d<f32>;
@group(0) @binding(5) var s_ramp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) uv_px: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) idx: u32) -> VsOut {
    // Fullscreen triangle.
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

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // Out-of-image: solid panel color.
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
        let cell = floor(in.pos.xy / 24.0);
        let s = (cell.x + cell.y) - 2.0 * floor((cell.x + cell.y) * 0.5);
        let bg = mix(vec3<f32>(0.07, 0.07, 0.10), vec3<f32>(0.10, 0.10, 0.13), s);
        return vec4<f32>(bg, 1.0);
    }

    // Sampler choice based on screen-space derivative: when one source pixel
    // covers more than ~6 screen pixels, switch to nearest for the crisp
    // per-pixel look.
    let d = max(length(dpdx(in.uv_px)), length(dpdy(in.uv_px)));
    let c = select(
        textureSample(t_image, s_nearest, uv),
        textureSample(t_image, s_linear, uv),
        d > 0.16,
    );

    var flags = 0u;
    let nan = any(c.rgb != c.rgb);
    let inf = any(c.rgb == vec3<f32>(1.0 / 0.0)) || any(c.rgb == vec3<f32>(-1.0 / 0.0));
    let neg = any(c.rgb < vec3<f32>(0.0));
    if (nan) { flags |= CLIP_NAN; }
    if (inf) { flags |= CLIP_INF; }
    if (neg) { flags |= CLIP_NEG; }

    // Exposure (skip on NaN/Inf to avoid propagation).
    var lin = c.rgb;
    if (!nan && !inf) {
        lin = lin * exp2(params.exposure);
    }

    let raw_over = any(lin > vec3<f32>(1.0));
    let raw_under = all(lin < vec3<f32>(0.001));

    var rgb = apply_channel(vec4<f32>(lin, c.a), params.channel, t_ramp, s_ramp);

    // Tonemap → SDR encode (only in SDR path; HDR canvas wants scene-linear).
    if (params.output_is_hdr == 0u) {
        rgb = tonemap(rgb, params.tonemap);
        rgb = linear_to_srgb(rgb);
    } else {
        // In HDR mode, mild compression of extreme highlights so we don't
        // blow out the panel. Reinhard-soft at 4×SDR.
        let knee = 4.0;
        rgb = rgb / (1.0 + max(rgb - vec3<f32>(knee), vec3<f32>(0.0)) / knee);
    }

    // Clipping overlays.
    if ((params.clip_flags & CLIP_NAN) != 0u && nan) {
        rgb = vec3<f32>(1.0, 0.0, 1.0); // magenta
    } else if ((params.clip_flags & CLIP_INF) != 0u && inf) {
        rgb = vec3<f32>(0.0, 1.0, 1.0); // cyan
    } else if ((params.clip_flags & CLIP_NEG) != 0u && neg) {
        rgb = stripes(in.uv_px, 6.0, vec3<f32>(1.0, 0.2, 0.2));
    } else {
        if ((params.clip_flags & CLIP_OVER) != 0u && raw_over) {
            rgb = checker(in.uv_px, 6.0, vec3<f32>(1.0, 0.9, 0.1));
        }
        if ((params.clip_flags & CLIP_UNDER) != 0u && raw_under) {
            rgb = checker(in.uv_px, 6.0, vec3<f32>(0.2, 0.4, 1.0));
        }
    }

    return vec4<f32>(rgb, 1.0);
}
