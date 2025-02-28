// frustum.wgsl
struct Plane {
    inner: vec4<f32>,
}

struct Frustum {
    left: Plane,
    right: Plane,
    top: Plane,
    bottom: Plane,
    near: Plane,
}

// {{include "rend3-routine/structures.wgsl"}}
struct UniformData {
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    origin_view_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    inv_origin_view_proj: mat4x4<f32>,
    frustum: Frustum,
    ambient: vec4<f32>,
    resolution: vec2<u32>,
}

@group(0) @binding(3) var<uniform> uniforms: UniformData;
@group(0) @binding(1) var nearest_sampler: sampler;

@group(1) @binding(0) var input_depth: texture_depth_multisampled_2d;
@group(1) @binding(1) var normals: texture_multisampled_2d<f32>;
@group(1) @binding(2) var output_occlusion: texture_storage_2d<r8unorm, write>;
@group(1) @binding(3) var<uniform> samples: array<vec4<f32>, 64>; // alignment requires us to go vec4 somehow.
@group(1) @binding(4) var noise_tex: texture_2d<f32>;

fn screen_to_view_space(screen_pos: vec2<f32>, depth: f32) -> vec3<f32> {
    let clip_space = vec4(screen_pos * 2.0 - 1.0, depth, 1.0); // 0..1 to -1..1, besides depth.
    let view_space = uniforms.view * uniforms.inv_view_proj * clip_space; // clip -> world -> view
    return view_space.xyz / view_space.w; // perspective division: renormalizing to w=1, only required when projecting.
}

fn world_space_normals_to_view_space(world_pos: vec3<f32>) -> vec3<f32> {
    // w = 0: At infinity there's no translation / dropping the fourth row that is translation. Thus also no division
    let view_space = uniforms.view * vec4(world_pos, 0.0);
    return view_space.xyz;
}

fn view_space_to_screen(view_pos: vec3<f32>) -> vec2<f32> {
    let clip_space = uniforms.view_proj * uniforms.inv_view * vec4(view_pos, 1.0); // view -> world -> clip
    return clip_space.xy / clip_space.w * 0.5 + 0.5; // perspective division and then to screen space
}

// TODO: Convert to uniforms
const kernelSize: u32 = 64;
const radius = 0.5;
const bias = 0.025;
const gain = 1.0;
const contrast = 1.0;

@compute
@workgroup_size(8, 8)
fn ssao_main(@builtin(global_invocation_id) global_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>) {
    let resolution = vec2<f32>(textureDimensions(input_depth));
    let uv = vec2<f32>(global_id.xy) / resolution;
    let depth = textureLoad(input_depth, vec2<i32>(global_id.xy), 0);
    let normal = textureLoad(normals, vec2<i32>(global_id.xy), 0).xyz;

    let view_space_pos = screen_to_view_space(uv, depth);
    let view_space_normal = world_space_normals_to_view_space(normal);

    // % nb of pixels of the noise texture for tiling (to not depend on samplers)
    let random_vec = normalize(textureLoad(noise_tex, vec2<i32>(global_id.xy % 4), 0).xyz);

    let tangent = normalize(random_vec - view_space_normal * dot(random_vec, view_space_normal)); // orthogonal to normal, on a plan of normal and random_vec?
    let bitangent = cross(view_space_normal, tangent);
    let tbn = mat3x3(tangent, bitangent, view_space_normal);

    var occlusion: f32;
    for (var i: u32 = 0; i < kernelSize; i++) {
        var _sample = tbn * samples[i].xyz; // tangent space -> view space
        _sample = view_space_pos + _sample * radius;

        let sample_uv = view_space_to_screen(_sample);
        let occluder_pos = screen_to_view_space(sample_uv, textureLoad(input_depth, vec2<i32>(sample_uv * resolution), 0));

        if occluder_pos.z >= _sample.z + bias {
            let range_check = smoothstep(0.0, 1.0, radius / abs(view_space_pos.z - occluder_pos.z));
            occlusion += range_check;
        }
    }

    occlusion /= f32(kernelSize);

    // Technically the opposite of occlusion so that we have the lighting to just mutiply
    let adjusted_occlusion = contrast * (pow(occlusion, gain) - 0.5) + 0.5;
    textureStore(output_occlusion, vec2<i32>(global_id.xy), vec4(vec3(1.0 - adjusted_occlusion), 1.0));
}