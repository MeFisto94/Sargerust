{{include "rend3-routine/structures.wgsl"}}
{{include "rend3-routine/structures_object.wgsl"}}
{{include "rend3-routine/material.wgsl"}}
{{include "rend3-routine/math/brdf.wgsl"}}
{{include "rend3-routine/math/color.wgsl"}}
{{include "rend3-routine/math/matrix.wgsl"}}

// whole frame uniform bind group
@group(0) @binding(0)
var primary_sampler: sampler;
@group(0) @binding(1)
var nearest_sampler: sampler;
@group(0) @binding(2)
var comparison_sampler: sampler_comparison;
@group(0) @binding(3)
var<uniform> uniforms: UniformData;

// per material bind group
@group(1) @binding(0)
var<storage> object_buffer: array<Object>;
@group(1) @binding(1)
var<storage> vertex_buffer: array<u32>;
@group(1) @binding(2)
var<storage> per_camera_uniform: PerCameraUniform;

{{
    vertex_fetch
    object_buffer
    position
    normal
}}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
}

@vertex
fn vs_main(@builtin(instance_index) instance_index: u32, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let indices = Indices(instance_index, vertex_index);
    let data = object_buffer[indices.object];
    let vs_in = get_vertices(indices);

    let model = object_buffer[indices.object].transform;
    let model_view_proj = per_camera_uniform.view_proj * model;
    let position_vec4 = vec4<f32>(vs_in.position, 1.0);

    let m_mat3 = mat3x3<f32>(model[0].xyz, model[1].xyz, model[2].xyz);
    let inv_scale_sq = mat3_inv_scale_squared(m_mat3);

    var vs_out: VertexOutput;
    vs_out.position = model_view_proj * position_vec4;
    vs_out.normal = m_mat3 * (inv_scale_sq * vs_in.normal);
    return vs_out;
}

@fragment
fn fs_main(vs_out: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vs_out.normal, 0);
}