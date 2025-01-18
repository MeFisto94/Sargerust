{{include "rend3-routine/structures.wgsl"}}
{{include "rend3-routine/structures_object.wgsl"}}
{{include "rend3-routine/material.wgsl"}}
{{include "rend3-routine/math/brdf.wgsl"}}
{{include "rend3-routine/math/color.wgsl"}}
{{include "rend3-routine/math/matrix.wgsl"}}

// TODO: Why don't we required the 16 byte padding here?
struct GpuUnitsData{
    texture_layers: array<u32, 3>,
    flags: u32,
}

// whole frame uniform bind group
@group(0) @binding(0)
var primary_sampler: sampler;
@group(0) @binding(1)
var nearest_sampler: sampler;

// per material bind group
@group(1) @binding(0)
var<storage> object_buffer: array<Object>;
@group(1) @binding(1)
var<storage> vertex_buffer: array<u32>;
@group(1) @binding(2)
var<storage> per_camera_uniform: PerCameraUniform;
@group(1) @binding(3)
var<storage> materials: array<GpuUnitsData>;

// texture bind group
@group(2) @binding(0)
var textures: binding_array<texture_2d<f32>>;

{{
    vertex_fetch
    object_buffer
    position
    texture_coords_0
}}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_relative: vec3<f32>,
    @location(1) @interpolate(flat) material: u32,
    @location(2) coords0: vec2<f32>,
}

@vertex
fn vs_main(@builtin(instance_index) instance_index: u32, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let indices = Indices(instance_index, vertex_index);
    let data = object_buffer[indices.object];
    let vs_in = get_vertices(indices);

    let model_view = per_camera_uniform.view * object_buffer[indices.object].transform;
    let model_view_proj = per_camera_uniform.view_proj * object_buffer[indices.object].transform;

    let position_vec4 = vec4<f32>(vs_in.position, 1.0);
    let mv_mat3 = mat3x3<f32>(model_view[0].xyz, model_view[1].xyz, model_view[2].xyz);

    let inv_scale_sq = mat3_inv_scale_squared(mv_mat3);

    var vs_out: VertexOutput;
    vs_out.coords0 = vs_in.texture_coords_0;
    vs_out.material = data.material_index;
    vs_out.position = model_view_proj * position_vec4;
    return vs_out;
}

@fragment
fn fs_main(vs_out: VertexOutput) -> @location(0) vec4<f32> {
    var material = materials[vs_out.material]; // needs to be var, otherwise accessing additional_layers[i] won't work.

    let coords = vs_out.coords0;
    let uvdx = dpdx(coords);
    let uvdy = dpdy(coords);

    if (material.texture_layers[0] == 0) {
        return vec4<f32>(0.22, 1.0, 0.0, 1.0); // lime green
    }

    var albedo_sum = vec4(0.0);

    for (var i = 0; i < 3; i++) {
        let tex_index = material.texture_layers[i];
        if (tex_index == 0u) {
            break;
        }

        let albedo = textureSampleGrad(textures[tex_index - 1u], primary_sampler, coords, uvdx, uvdy);
        albedo_sum = mix(albedo_sum, albedo, albedo.a);
    }

    if (albedo_sum.a <= 0.1) {
        discard;
    }

    return albedo_sum;
}