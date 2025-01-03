{{include "rend3-routine/structures.wgsl"}}
{{include "rend3-routine/structures_object.wgsl"}}
{{include "rend3-routine/material.wgsl"}}
{{include "rend3-routine/math/brdf.wgsl"}}
{{include "rend3-routine/math/color.wgsl"}}
{{include "rend3-routine/math/matrix.wgsl"}}

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

//    tangent
//    texture_coords_0
//    texture_coords_1
//    color_0

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
//    @location(0) view_position: vec4<f32>,
    @location(1) normal: vec3<f32>,
//    @location(2) tangent: vec3<f32>,
//    @location(3) coords0: vec2<f32>,
//    @location(4) coords1: vec2<f32>,
//    @location(6) color: vec4<f32>,
    //@location(7) @interpolate(flat) material: u32,
}


@vertex
fn vs_main(@builtin(instance_index) instance_index: u32, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let indices = Indices(instance_index, vertex_index);

    //let data = object_buffer[indices.object];

    let vs_in = get_vertices(indices);

    let model_view = per_camera_uniform.view * object_buffer[indices.object].transform;
    let model_view_proj = per_camera_uniform.view_proj * object_buffer[indices.object].transform;

    let position_vec4 = vec4<f32>(vs_in.position, 1.0);
    let mv_mat3 = mat3x3<f32>(model_view[0].xyz, model_view[1].xyz, model_view[2].xyz);

    let inv_scale_sq = mat3_inv_scale_squared(mv_mat3);

    var vs_out: VertexOutput;
    //vs_out.material = data.material_index;
//    vs_out.view_position = model_view * position_vec4;
     // This would produce view space normals, but we want world space normals
    // vs_out.normal = normalize(mv_mat3 * (inv_scale_sq * vs_in.normal));
    vs_out.normal = vs_in.normal;
//    vs_out.tangent = normalize(mv_mat3 * (inv_scale_sq * vs_in.tangent));
//    vs_out.color = vs_in.color_0;
//    vs_out.coords0 = vs_in.texture_coords_0;
//    vs_out.coords1 = vs_in.texture_coords_1;
    vs_out.position = model_view_proj * position_vec4;

    return vs_out;
}

@fragment
fn fs_main(vs_out: VertexOutput) -> @location(0) vec4<f32> {
    // let material = materials[vs_out.material];

    //return vec4(0.0, 0.0, 1.0, 1.0);
    return vec4(vs_out.normal, 1.0);
}