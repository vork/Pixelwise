// 2× box downsampler. One workgroup per 8×8 destination block.
//
// In: t_src (texture_2d<f32>) at mip N
// Out: t_dst (texture_storage_2d<rgba16float, write>) at mip N+1

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;
@group(0) @binding(2) var t_dst: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn cs(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_size = textureDimensions(t_dst);
    if (gid.x >= dst_size.x || gid.y >= dst_size.y) { return; }
    let inv = 1.0 / vec2<f32>(dst_size);
    let uv = (vec2<f32>(gid.xy) + vec2<f32>(0.5)) * inv;
    let c = textureSampleLevel(t_src, s_src, uv, 0.0);
    textureStore(t_dst, vec2<i32>(gid.xy), c);
}
