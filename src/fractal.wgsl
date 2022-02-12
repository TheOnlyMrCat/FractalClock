struct VertexInput {
    [[builtin(vertex_index)]] index: u32;
    [[location(0)]] position: vec2<f32>;
    [[location(1)]] depth: f32;
};

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] colour: vec4<f32>;
};

// fn hue_to_rgb(hue: f32) -> vec4<f32> {
//     let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
//     let p = abs(fract(vec3<f32>(hue, hue, hue) + K.xyz) * 6.0 - K.www) - K.xxx;
//     //TODO: Re-vectorise this clamping when possible?
//     return vec4<f32>(clamp(p.x, 0.0, 1.0), clamp(p.y, 0.0, 1.0), clamp(p.z, 0.0, 1.0), 1.0);
// }

struct Camera {
    projection: mat4x4<f32>;
};

struct Colours {
    colours: array<vec4<f32>>;
};

[[group(0), binding(0)]] var<uniform> camera: Camera;
[[group(1), binding(0)]] var<storage> colours: Colours;

[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.projection * vec4<f32>(in.position, 0.0, 1.0);
    out.colour = colours.colours[u32(in.depth)];
    return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return in.colour;
}