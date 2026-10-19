// Prenesenie hotového obrazu do výrezu okna.

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

struct BlitOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_blit(@builtin(vertex_index) i: u32) -> BlitOut {
    var o: BlitOut;
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    o.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    return o;
}

@fragment
fn fs_blit(i: BlitOut) -> @location(0) vec4<f32> {
    return textureSample(image, image_sampler, i.uv);
}
