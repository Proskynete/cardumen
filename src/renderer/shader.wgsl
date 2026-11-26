struct Uniforms {
    screen: vec2<f32>,
    atlas: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct Quad {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) kind: u32,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) kind: u32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, q: Quad) -> Varyings {
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let px = q.pos + corner * q.size;
    var out: Varyings;
    out.position = vec4<f32>(px.x / u.screen.x * 2.0 - 1.0, 1.0 - px.y / u.screen.y * 2.0, 0.0, 1.0);
    out.uv = (q.uv.xy + corner * q.uv.zw) / u.atlas;
    out.color = q.color;
    out.kind = q.kind;
    return out;
}

// kind: 0 = solid quad, 1 = mask glyph tinted by color, 2 = colour glyph (emoji).
@fragment
fn fs_main(v: Varyings) -> @location(0) vec4<f32> {
    let t = textureSampleLevel(atlas_tex, atlas_sampler, v.uv, 0.0);
    if (v.kind == 0u) {
        return v.color;
    }
    if (v.kind == 1u) {
        return vec4<f32>(v.color.rgb, v.color.a * t.a);
    }
    return t;
}
