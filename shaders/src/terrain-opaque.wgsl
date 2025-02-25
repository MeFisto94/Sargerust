{{include "rend3-routine/structures.wgsl"}}
{{include "rend3-routine/structures_object.wgsl"}}
{{include "rend3-routine/material.wgsl"}}
{{include "rend3-routine/math/brdf.wgsl"}}
{{include "rend3-routine/math/color.wgsl"}}
{{include "rend3-routine/math/matrix.wgsl"}}
{{include "rend3-routine/shadow/pcf.wgsl"}}

// TODO: Why don't we required the 16 byte padding here?
struct GpuTerrainData {
    base_texture: u32,
    additional_layers: array<u32, 6>,
    flags: u32,
}

// whole frame uniform bind group
@group(0) @binding(0)
var primary_sampler: sampler;
@group(0) @binding(1)
var nearest_sampler: sampler;
@group(0) @binding(2)
var comparison_sampler: sampler_comparison;
@group(0) @binding(3)
var<uniform> uniforms: UniformData;
@group(0) @binding(4)
var<storage> directional_lights: DirectionalLightData;
@group(0) @binding(5)
var<storage> point_lights: PointLightData;
@group(0) @binding(6)
var shadows: texture_depth_2d;

// per material bind group
@group(1) @binding(0)
var<storage> object_buffer: array<Object>;
@group(1) @binding(1)
var<storage> vertex_buffer: array<u32>;
@group(1) @binding(2)
var<storage> per_camera_uniform: PerCameraUniform;
@group(1) @binding(3)
var<storage> materials: array<GpuTerrainData>;

// texture bind group
@group(2) @binding(0)
var textures: binding_array<texture_2d<f32>>;

{{
    vertex_fetch
    object_buffer
    position
    normal
}}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_relative: vec3<f32>,
    @location(1) world_space_normal: vec3<f32>,
    @location(2) @interpolate(flat) material: u32,
    @location(3) view_position: vec4<f32>,
    @location(4) normal: vec3<f32>,
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
    let GRID_SIZE_ADJUSTED = 100.0 / 28.0 * -9.33333333; // The factor 9.333 has been chosen experimentally. We would've expected it to be exactly 8 or 9.
    // divide by GRID_SIZE which is 100/28, vertices are always 0..-9, so we transpose to 0..1 in the end.

    var xy = vs_in.position.xy / GRID_SIZE_ADJUSTED;
    let z = (vs_in.position.z / GRID_SIZE_ADJUSTED) * 0.5 + 0.5; // unlike the grid vertices, height goes from -n..n

    // Note: Normals want "up" to be y, also note that x and y will be negative (due to the vertices being 0..8*GRID_SIZE)
    vs_out.vertex_relative = vec3(xy.x, xy.y, z);

    vs_out.material = data.material_index;
    vs_out.world_space_normal = normalize(vs_in.normal);
    vs_out.normal = normalize(mv_mat3 * (inv_scale_sq * vs_in.normal));
    vs_out.position = model_view_proj * position_vec4;
    vs_out.view_position = model_view * position_vec4;
    return vs_out;
}

@fragment
fn fs_main(vs_out: VertexOutput) -> @location(0) vec4<f32> {
    var material = materials[vs_out.material]; // needs to be var, otherwise accessing additional_layers[i] won't work.
    if (material.base_texture == 0u) {
        return vec4<f32>(0.5, 0.0, 0.0, 1.0);
    }

    let tex_scale = -9.3333333;
    let blend_sharpness = 25.0;
    var blend_weights = pow(abs(vs_out.world_space_normal), vec3(blend_sharpness));
    blend_weights /= blend_weights.x + blend_weights.y + blend_weights.z;
    // blend_weights = vec3(0.0, 0.0, 1.0); // TODO: Use this to disable triplanar mapping, I have yet to encounter a mountain where it improves visuals...

    // Since wgsl doesn't like "var" textures and others, we duplicate the code a bit, but it doesn't hurt readability here anyway.
    let base_tex = textures[material.base_texture - 1u];
    let tex_x = textureSample(base_tex, primary_sampler, vs_out.vertex_relative.yz * tex_scale);
    let tex_y = textureSample(base_tex, primary_sampler, vs_out.vertex_relative.xz * tex_scale);
    let tex_z = textureSample(base_tex, primary_sampler, vs_out.vertex_relative.yx * tex_scale);
    var albedo_sum = tex_x * blend_weights.x + tex_y * blend_weights.y + tex_z * blend_weights.z;

    for (var i = 0; i < 3; i++) {
        let tex_index = material.additional_layers[2 * i];
        let alpha_index = material.additional_layers[2 * i + 1];

        if (tex_index != 0 && alpha_index == 0) {
            return vec4(1.0, 0.0, 0.0, 1.0);
        }

        if (tex_index == 0u || alpha_index == 0u) {
            continue; // TODO: continue or break?
        }

        let albedo_tex = textures[tex_index - 1u];
        let alpha_tex = textures[alpha_index - 1u];

        let tex_x = textureSample(albedo_tex, primary_sampler, vs_out.vertex_relative.yz * tex_scale);
        let tex_y = textureSample(albedo_tex, primary_sampler, vs_out.vertex_relative.xz * tex_scale);
        let tex_z = textureSample(albedo_tex, primary_sampler, vs_out.vertex_relative.yx * tex_scale);
        let albedo = tex_x * blend_weights.x + tex_y * blend_weights.y + tex_z * blend_weights.z;

        let alpha = textureSample(alpha_tex, primary_sampler, vs_out.vertex_relative.yx).r;
        albedo_sum = mix(albedo_sum, albedo, alpha);
    }

    let combined_albedo = vec4(albedo_sum.xyz, 1.0);

    // Diffuse color / albedo has been determined, now it's time to apply directional light and shadows.
    var color = vec3(0.0);

    // View vector
    let v = -normalize(vs_out.view_position.xyz);
    // Transform vectors into view space
    let view_mat3 = mat3x3<f32>(uniforms.view[0].xyz, uniforms.view[1].xyz, uniforms.view[2].xyz);
    for (var i = 0; i < i32(directional_lights.count); i += 1) {
        let light = directional_lights.data[i];

        // Get the shadow ndc coordinates, then convert to texture sample coordinates
        let shadow_ndc = (light.view_proj * uniforms.inv_view * vs_out.view_position).xyz;
        let shadow_flipped = (shadow_ndc.xy * 0.5) + 0.5;
        let shadow_local_coords = vec2<f32>(shadow_flipped.x, 1.0 - shadow_flipped.y);

        // Texture sample coordinates of
        var top_left = light.offset;
        var top_right = top_left + light.size;
        let shadow_coords = mix(top_left, top_right, shadow_local_coords);

        // The shadow is stored in an atlas, so we need to make sure we don't linear blend
        // across atlasses. We move our conditional borders in a half a pixel for standard
        // linear blending (so we're hitting texel centers on the edge). We move it an additional
        // pixel in so that our pcf5 offsets don't move off the edge of the atlasses.
        let shadow_border = light.inv_resolution * 1.5;
        top_left += shadow_border;
        top_right -= shadow_border;

        var shadow_value = 1.0;
        if (
            any(shadow_flipped >= top_left) && // XY lower
            any(shadow_flipped <= top_right) && // XY upper
            shadow_ndc.z >= 0.0 && // Z lower
            shadow_ndc.z <= 1.0 // Z upper
        ) {
            shadow_value = shadow_sample_pcf5(shadows, comparison_sampler, shadow_coords, shadow_ndc.z);
        }

        // Calculate light source vector
        let l = normalize(view_mat3 * -light.direction);
        color += surface_shading(l, light.color, v, shadow_value, combined_albedo.xyz, vs_out.normal);
    }

    return clamp_ambient_color(combined_albedo, color);
}

fn clamp_ambient_color(albedo: vec4<f32>, shaded_color: vec3<f32>) -> vec4<f32> {
    let ambient = uniforms.ambient * albedo;
    let shaded = vec4<f32>(shaded_color, albedo.a); // TODO: Understand why the shaded color isn't already alphaed
    return max(ambient, shaded);
}

// We don't use PBR here
fn surface_shading(inv_light_dir: vec3<f32>, intensity: vec3<f32>, view_pos: vec3<f32>, occlusion: f32, diffuse_color: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let nol = saturate(dot(normal, inv_light_dir));

    // specular
    let fr = vec3(0.0);

    // diffuse
    let fd = diffuse_color * brdf_fd_lambert();

    let energy_comp = 1.0;
    let color = fd + fr * energy_comp;

    let light_attenuation = 1.0; // directional lights -> no
    return (color * intensity) * (light_attenuation * nol * occlusion /* shadows */);
}