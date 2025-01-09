use crate::rendering::rend3_backend::material::terrain::terrain_material::TerrainMaterial;
use rend3::RendererProfile::GpuDriven;
use rend3::{Renderer, RendererDataCore, ShaderConfig, ShaderPreProcessor, ShaderVertexBufferConfig};
use rend3_routine::common::{PerMaterialArchetypeInterface, WholeFrameInterfaces};
use rend3_routine::forward::{ForwardRoutine, ForwardRoutineCreateArgs, RoutineType, ShaderModulePair};
use rend3_routine::pbr::TransparencyType;
use rust_embed::RustEmbed;
use std::borrow::Cow;
use std::sync::Arc;
use wgpu::{BlendState, ShaderModuleDescriptor, ShaderSource};

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/shaders/src"]
struct SargerustShaderSources;

pub struct TerrainRoutine {
    pub opaque_routine: ForwardRoutine<TerrainMaterial>,
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

        let module = renderer
            .device
            .create_shader_module(ShaderModuleDescriptor {
                label: Some("terrain opaque sm"),
                source: ShaderSource::Wgsl(Cow::Owned(
                    spp.render_shader(
                        "sargerust/terrain-opaque.wgsl",
                        &ShaderConfig {
                            //profile: Some(renderer.profile),
                            profile: Some(GpuDriven),
                            ..Default::default()
                        },
                        Some(&ShaderVertexBufferConfig::from_material::<TerrainMaterial>()),
                    )
                    .unwrap(),
                )),
            });

        let routine_type = RoutineType::Forward;
        let transparency = TransparencyType::Opaque;

        let opaque_routine = ForwardRoutine::new(ForwardRoutineCreateArgs {
            name: &format!("Terrain {routine_type:?} {transparency:?}"),
            renderer,
            data_core,
            spp,
            interfaces,
            per_material: &per_material,
            material_key: transparency as u64,
            routine_type,
            shaders: ShaderModulePair {
                vs_entry: "vs_main",
                vs_module: &module,
                fs_entry: "fs_main",
                fs_module: &module,
            },
            extra_bgls: &[],
            descriptor_callback: Some(&|desc, targets| {
                if transparency == TransparencyType::Blend {
                    desc.depth_stencil.as_mut().unwrap().depth_write_enabled = false;
                    targets[0].as_mut().unwrap().blend = Some(BlendState::ALPHA_BLENDING)
                }
            }),
        });

        Self {
            opaque_routine,
            per_material,
        }
    }
}
