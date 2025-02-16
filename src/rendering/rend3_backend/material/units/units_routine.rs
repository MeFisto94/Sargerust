use crate::rendering::rend3_backend::material::SargerustShaderSources;
use crate::rendering::rend3_backend::material::units::units_material::UnitsMaterial;
use rend3::RendererProfile::GpuDriven;
use rend3::{Renderer, RendererDataCore, RendererProfile, ShaderConfig, ShaderPreProcessor, ShaderVertexBufferConfig};
use rend3_routine::common::{PerMaterialArchetypeInterface, WholeFrameInterfaces};
use rend3_routine::forward::{ForwardRoutine, ForwardRoutineCreateArgs, RoutineType, ShaderModulePair};
use serde::Serialize;
use std::borrow::Cow;
use std::sync::Arc;
use wgpu::{ShaderModule, ShaderModuleDescriptor, ShaderSource};

#[derive(Debug, Default, Serialize)]
struct UnitsDepthConfig {
    pub discard: bool,
    pub profile: Option<RendererProfile>,
    pub position_attribute_offset: usize,
}

pub struct UnitsRoutine {
    pub opaque_routine: ForwardRoutine<UnitsMaterial>,
    pub cutout_routine: ForwardRoutine<UnitsMaterial>,
    pub opaque_depth: ForwardRoutine<UnitsMaterial>,
    pub cutout_depth: ForwardRoutine<UnitsMaterial>,
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
        profiling::scope!("UnitsRoutine::new");

        // This ensures the BGLs for the material are created
        data_core
            .material_manager
            .ensure_archetype::<UnitsMaterial>(&renderer.device, renderer.profile);

        let per_material = PerMaterialArchetypeInterface::<UnitsMaterial>::new(&renderer.device);

        let opaque_module = renderer
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

        let depth_sm = renderer
            .device
            .create_shader_module(ShaderModuleDescriptor {
                label: Some("units depth sm"),
                source: ShaderSource::Wgsl(Cow::Owned(
                    spp.render_shader(
                        "sargerust/units-depth.wgsl",
                        &UnitsDepthConfig {
                            profile: Some(GpuDriven),
                            discard: false,
                            ..Default::default()
                        },
                        Some(&ShaderVertexBufferConfig::from_material::<UnitsMaterial>()),
                    )
                    .unwrap(),
                )),
            });

        let cutout_depth_sm = renderer
            .device
            .create_shader_module(ShaderModuleDescriptor {
                label: Some("units depth sm"),
                source: ShaderSource::Wgsl(Cow::Owned(
                    spp.render_shader(
                        "sargerust/units-depth.wgsl",
                        &UnitsDepthConfig {
                            profile: Some(GpuDriven),
                            discard: true,
                            ..Default::default()
                        },
                        Some(&ShaderVertexBufferConfig::from_material::<UnitsMaterial>()),
                    )
                    .unwrap(),
                )),
            });

        Self {
            opaque_routine: sm_to_routine(
                "Units Forward Opaque",
                &per_material,
                0,
                RoutineType::Forward,
                &opaque_module,
                renderer,
                data_core,
                spp,
                interfaces,
            ),
            cutout_routine: sm_to_routine(
                "Units Forward Cutout",
                &per_material,
                1,
                RoutineType::Forward,
                &opaque_module,
                renderer,
                data_core,
                spp,
                interfaces,
            ),
            opaque_depth: sm_to_routine(
                "Units Depth Opaque",
                &per_material,
                0,
                RoutineType::Depth,
                &depth_sm,
                renderer,
                data_core,
                spp,
                interfaces,
            ),
            cutout_depth: sm_to_routine(
                "Units Depth Cutout",
                &per_material,
                1,
                RoutineType::Depth,
                &cutout_depth_sm,
                renderer,
                data_core,
                spp,
                interfaces,
            ),
            per_material,
        }
    }
}

pub fn sm_to_routine<T: rend3::types::Material>(
    name: &str,
    per_material: &PerMaterialArchetypeInterface<T>,
    material_key: u64,
    routine_type: RoutineType,
    shader_module: &ShaderModule,
    renderer: &Arc<Renderer>,
    data_core: &mut RendererDataCore,
    spp: &mut ShaderPreProcessor,
    interfaces: &WholeFrameInterfaces,
) -> ForwardRoutine<T> {
    ForwardRoutine::new(ForwardRoutineCreateArgs {
        name,
        renderer,
        data_core,
        spp,
        interfaces,
        per_material,
        material_key,
        routine_type,
        shaders: ShaderModulePair {
            vs_entry: "vs_main",
            vs_module: &shader_module,
            fs_entry: "fs_main",
            fs_module: &shader_module,
        },
        extra_bgls: &[],
        descriptor_callback: Some(&|_desc, _targets| {}),
    })
}
