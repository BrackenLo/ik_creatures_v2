//====================================================================
// Uniforms

struct Camera {
    projection: mat4x4<f32>,
    position: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

//====================================================================

struct VertexIn {
    // Vertex
    @location(0) vertex_pos: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

//====================================================================

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;

    out.clip_position =
        camera.projection
        * vec4<f32>(in.vertex_pos, 0., 1.);


    out.color = in.vertex_color;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}

//====================================================================

