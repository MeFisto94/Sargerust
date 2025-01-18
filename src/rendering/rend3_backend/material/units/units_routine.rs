use crate::rendering::rend3_backend::material::SargerustShaderSources;
use crate::rendering::rend3_backend::material::units::units_material::UnitsMaterial;
use rend3::RendererProfile::GpuDriven;
use rend3::{Renderer, RendererDataCore, ShaderConfig, ShaderPreProcessor, ShaderVertexBufferConfig};
use rend3_routine::common::{PerMaterialArchetypeInterface, WholeFrameInterfaces};
use rend3_routine::forward::{ForwardRoutine, ForwardRoutineCreateArgs, RoutineType, ShaderModulePair};
use rend3_routine::pbr::TransparencyType;
use std::borrow::Cow;
use std::sync::Arc;
use wgpu::{BlendState, ShaderModuleDescriptor, ShaderSource};

pub struct UnitsRoutine {
    pub opaque_routine: ForwardRoutine<UnitsMaterial>,
    pub per_material: PerMaterialArchetypeInterface<UnitsMaterial>,
}

impl UnitsRoutine {
    pub fn new(
        renderer: &Arc<Renderer>,
        data_core: &mut RendererDataCore,
        spp: &mut ShaderPreProcessor,
        interfaces: &WholeFrameInterfaces,
    ) -> Self {
        // TODO: This is not really in-sync with how the other shaders do it, but:
        // TODO: Pull this out, somehow somewhere more central, otherwise we uselessly read and overwrite the entries.
        spp.add_shaders_embed::<SargerustShaderSources>("sargerust");
        // profiling::scope!("UnitsRoutine::new");

        // This ensures the BGLs for the material are created
        data_core
            .material_manager
            .ensure_archetype::<UnitsMaterial>(&renderer.device, renderer.profile);

        let per_material = PerMaterialArchetypeInterface::<UnitsMaterial>::new(&renderer.device);

        let module = renderer
            .device
            .create_shader_module(ShaderModuleDescriptor {
                label: Some("units opaque sm"),
                source: ShaderSource::Wgsl(Cow::Owned(
                    spp.render_shader(
                        "sargerust/units-opaque.wgsl",
                        &ShaderConfig {
                            //profile: Some(renderer.profile),
                            profile: Some(GpuDriven),
                            ..Default::default()
                        },
                        Some(&ShaderVertexBufferConfig::from_material::<UnitsMaterial>()),
                    )
                    .unwrap(),
                )),
            });

        let routine_type = RoutineType::Forward;
        let transparency = TransparencyType::Opaque;

        let opaque_routine = ForwardRoutine::new(ForwardRoutineCreateArgs {
            name: &format!("Units {routine_type:?} {transparency:?}"),
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
