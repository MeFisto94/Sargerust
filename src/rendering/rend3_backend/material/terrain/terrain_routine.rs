use crate::rendering::rend3_backend::material::SargerustShaderSources;
use crate::rendering::rend3_backend::material::terrain::terrain_material::TerrainMaterial;
use crate::rendering::rend3_backend::material::units::units_routine;
use rend3::RendererProfile::GpuDriven;
use rend3::util::bind_merge::BindGroupLayoutBuilder;
use rend3::{Renderer, RendererDataCore, ShaderConfig, ShaderPreProcessor, ShaderVertexBufferConfig};
use rend3_routine::common::{PerMaterialArchetypeInterface, WholeFrameInterfaces};
use rend3_routine::forward::{ForwardRoutine, RoutineType};
use std::borrow::Cow;
use std::sync::Arc;
use wgpu::{BindingType, ShaderModuleDescriptor, ShaderSource, ShaderStages, TextureSampleType};

pub struct TerrainRoutine {
    pub opaque_routine: ForwardRoutine<TerrainMaterial>,
    pub depth_routine: ForwardRoutine<TerrainMaterial>,
    pub per_material: PerMaterialArchetypeInterface<TerrainMaterial>,
}

impl TerrainRoutine {
    pub fn new(
        renderer: &Arc<Renderer>,
        data_core: &mut RendererDataCore,
        spp: &mut ShaderPreProcessor,
        interfaces: &WholeFrameInterfaces,
    ) -> Self {
        // TODO: This is not really in-sync with how the other shaders do it, but:
        spp.add_shaders_embed::<SargerustShaderSources>("sargerust");
        // profiling::scope!("TerrainRoutine::new");

        // This ensures the BGLs for the material are created
        data_core
            .material_manager
            .ensure_archetype::<TerrainMaterial>(&renderer.device, renderer.profile);

        let per_material = PerMaterialArchetypeInterface::<TerrainMaterial>::new(&renderer.device);

        let ssao_bgl = BindGroupLayoutBuilder::new()
            .append(
                ShaderStages::FRAGMENT,
                BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                None,
            )
            .build(&renderer.device, Some("SSAO Textures"));

        let sm_opaque = renderer
            .device
            .create_shader_module(ShaderModuleDescriptor {
                label: Some("terrain opaque sm"),
                source: ShaderSource::Wgsl(Cow::Owned(
                    spp.render_shader(
                        "sargerust/terrain-opaque.wgsl",
                        &ShaderConfig {
                            // TODO: support CpuDriven
                            //profile: Some(renderer.profile),
                            profile: Some(GpuDriven),
                            ..Default::default()
                        },
                        Some(&ShaderVertexBufferConfig::from_material::<TerrainMaterial>()),
                    )
                    .unwrap(),
                )),
            });

        let sm_depth = renderer
            .device
            .create_shader_module(ShaderModuleDescriptor {
                label: Some("terrain depth sm"),
                source: ShaderSource::Wgsl(Cow::Owned(
                    spp.render_shader(
                        "sargerust/terrain-depth.wgsl",
                        &ShaderConfig {
                            // TODO: support CpuDriven
                            //profile: Some(renderer.profile),
                            profile: Some(GpuDriven),
                            ..Default::default()
                        },
                        Some(&ShaderVertexBufferConfig::from_material::<TerrainMaterial>()),
                    )
                    .unwrap(),
                )),
            });

        Self {
            opaque_routine: units_routine::sm_to_routine(
                "Terrain Opaque",
                &per_material,
                0,
                RoutineType::Forward,
                &sm_opaque,
                renderer,
                data_core,
                spp,
                interfaces,
                &[&ssao_bgl],
            ),
            depth_routine: units_routine::sm_to_routine(
                "Terrain Depth",
                &per_material,
                0,
                RoutineType::Depth,
                &sm_depth,
                renderer,
                data_core,
                spp,
                interfaces,
                &[],
            ),
            per_material,
        }
    }
}
