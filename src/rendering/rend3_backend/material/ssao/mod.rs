use rand::Rng;
use rend3::Renderer;
use rend3::util::bind_merge::BindGroupLayoutBuilder;
use rend3_routine::common::WholeFrameInterfaces;
use rend3_routine::compute::ComputeRoutine;
use std::ops::Mul;
use std::sync::Arc;
use wgpu::{BindingType, ShaderStages, StorageTextureAccess, TextureSampleType, include_wgsl};

pub const SAMPLE_SIZE: usize = 64; // changes currently not replicated into the shader

pub fn build_main_routine(renderer: &Arc<Renderer>) -> ComputeRoutine<([glam::Vec3; SAMPLE_SIZE], [glam::Vec3; 16])> {
    // TODO: no Render Shader here, we don't benefit from all the includes and things in CS (yet!!)
    let sm = renderer.device.create_shader_module(include_wgsl!(
        "../../../../../shaders/src/ssao/ssao-main.wgsl"
    ));

    let mut samples = Vec::<glam::Vec3>::with_capacity(SAMPLE_SIZE);
    let mut noise = Vec::<glam::Vec3>::with_capacity(16);
    let mut rng = rand::rng();

    for i in 0..SAMPLE_SIZE {
        let mut sample = glam::Vec3::new(
            rng.random::<f32>() * 2.0 - 1.0,
            rng.random::<f32>() * 2.0 - 1.0,
            rng.random::<f32>(),
        )
        .normalize()
        .mul(rng.random::<f32>());

        let scale = i as f32 / SAMPLE_SIZE as f32;
        sample *= 0.1 + (scale * scale) * 0.9; // lerp(0.1, 1.0, scale * scale);
        samples.push(sample);
    }

    for _ in 0..16 {
        noise.push(glam::Vec3::new(
            rng.random::<f32>() * 2.0 - 1.0,
            rng.random::<f32>() * 2.0 - 1.0,
            0.0,
        ));
    }

    ComputeRoutine::new(
        "SSAO",
        renderer,
        &sm,
        "ssao_main",
        |_| {
            let mut builder = BindGroupLayoutBuilder::new();
            builder
                .append(
                    ShaderStages::COMPUTE,
                    BindingType::Texture {
                        sample_type: TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    None,
                )
                .append(
                    ShaderStages::COMPUTE,
                    BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    None,
                )
                .append(
                    ShaderStages::COMPUTE,
                    BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    None,
                )
                .append_buffer(
                    ShaderStages::COMPUTE,
                    wgpu::BufferBindingType::Uniform,
                    false,
                    16 * SAMPLE_SIZE as u64, // for some reason padded to 16 bytes instead of 12
                )
                .append(
                    ShaderStages::COMPUTE,
                    BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    None,
                );

            let interfaces = WholeFrameInterfaces::new(&renderer.device);

            vec![
                interfaces.forward_uniform_bgl,
                builder.build(&renderer.device, Some("SSAO Textures")),
            ]
        },
        (samples.try_into().unwrap(), noise.try_into().unwrap()),
    )
}

fn build_blur(renderer: &Arc<Renderer>) -> ComputeRoutine<()> {
    let sm = renderer.device.create_shader_module(include_wgsl!(
        "../../../../../shaders/src/ssao/ssao-blur.wgsl"
    ));

    ComputeRoutine::new(
        "SSAO Blur",
        renderer,
        &sm,
        "blur_main",
        |_| {
            let mut builder = BindGroupLayoutBuilder::new();
            builder
                .append(
                    ShaderStages::COMPUTE,
                    BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    None,
                )
                .append(
                    ShaderStages::COMPUTE,
                    BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    None,
                );
            vec![builder.build(&renderer.device, Some("SSAO Blur Textures"))]
        },
        (),
    )
}

pub struct SSAORoutines {
    pub main: ComputeRoutine<([glam::Vec3; SAMPLE_SIZE], [glam::Vec3; 16])>,
    pub blur: ComputeRoutine<()>,
}

impl SSAORoutines {
    pub fn new(renderer: &Arc<Renderer>) -> Self {
        Self {
            main: build_main_routine(renderer),
            blur: build_blur(renderer),
        }
    }
}
