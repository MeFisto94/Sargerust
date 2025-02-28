@group(0) @binding(0) var input_ssao: texture_2d<f32>;
@group(0) @binding(1) var output_blurred: texture_storage_2d<r16float, write>;

@compute
@workgroup_size(8, 8)
fn blur_main(@builtin(global_invocation_id) global_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>) {
    let screen_size = textureDimensions(input_ssao);
    if (global_id.x >= screen_size.x - 2 || global_id.y >= screen_size.y - 2 || global_id.x < 2 || global_id.y < 2) {
        textureStore(output_blurred, vec2<i32>(global_id.xy), vec4(textureLoad(input_ssao, vec2<i32>(global_id.xy), 0).r));
        return;
    }

    var amount: f32 = 0.0;
    for (var x = -2; x < 2; x++) {
        for (var y = -2; y < 2; y++) {
            let uv = vec2<i32>(global_id.xy) + vec2<i32>(x, y);
            amount += textureLoad(input_ssao, uv, 0).r;
        }
    }

    textureStore(output_blurred, vec2<i32>(global_id.xy), vec4(amount / 16.0f));
}