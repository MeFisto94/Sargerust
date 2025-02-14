use arc_swap::ArcSwapOption;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::hash::BuildHasher;
use std::ops::DerefMut;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::Instant;
use winit::event::Event;

use crate::game::application::GameApplication;
use crate::game::map_light_settings_provider::interpolate_color;
use crate::rendering::asset_graph::m2_generator::M2Generator;
use crate::rendering::asset_graph::nodes::adt_node::{ADTNode, DoodadReference};
use crate::rendering::asset_graph::resolver::Resolver;
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::types::{AlbedoType, Material, TransparencyType};
use crate::rendering::rend3_backend::material::terrain::terrain_material::TerrainMaterial;
use crate::rendering::rend3_backend::material::terrain::terrain_routine::TerrainRoutine;
use crate::rendering::rend3_backend::material::units::units_material::{UnitsAlbedo, UnitsMaterial};
use crate::rendering::rend3_backend::material::units::units_routine::UnitsRoutine;
use crate::rendering::rend3_backend::{
    IRM2Material, IRMaterial, IRTexture, IRTextureReference, Rend3BackendConverter, gpu_loaders,
};
use glam::{Mat4, UVec2, Vec3A, Vec4, Vec4Swizzles};
use itertools::Itertools;
use log::{trace, warn};
use rend3::graph::RenderGraph;
use rend3::types::{
    Camera, CameraProjection, Handedness, MaterialHandle, PresentMode, SampleCount, Texture, Texture2DHandle,
};
use rend3::util::typedefs::FastHashMap;
use rend3::{Renderer, ShaderPreProcessor};
use rend3_framework::{EventContext, Grabber, RedrawContext, SetupContext};
use rend3_routine::base::{
    BaseRenderGraph, BaseRenderGraphInputs, BaseRenderGraphIntermediateState, BaseRenderGraphRoutines,
    BaseRenderGraphSettings, OutputRenderTarget,
};
use rend3_routine::common::CameraSpecifier;
use rend3_routine::forward::ForwardRoutineArgs;
use rend3_routine::{clear, forward};
use sargerust_files::m2::types::{M2TextureFlags, M2TextureType};
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// #[derive(Debug)] // TODO: Ensure Grabber implements Display
pub struct RenderingApplication {
    scancode_status: FastHashMap<KeyCode, bool>,
    camera_pitch: f32,
    camera_yaw: f32,
    camera_location: Vec3A,
    //last_mouse_delta: Option<DVec2>,
    timestamp_last_frame: Instant,
    grabber: Option<Grabber>,
    app: Weak<GameApplication>,

    // mirroring the state of the MapManager.
    current_map: Option<String>,
    tile_graph: HashMap<(u8, u8), Arc<ADTNode>>,
    missing_texture_material: Option<MaterialHandle>,
    texture_still_loading_material: Option<MaterialHandle>,
    fly_cam: bool,

    terrain_routine: Option<Mutex<TerrainRoutine>>,
    units_routine: Option<Mutex<UnitsRoutine>>,
}

impl RenderingApplication {
    pub fn new(app: Weak<GameApplication>) -> Self {
        Self {
            app,
            scancode_status: FastHashMap::default(),
            camera_pitch: 0.0,
            camera_yaw: 0.0,
            camera_location: Vec3A::new(0.0, 0.0, 0.0),
            timestamp_last_frame: Instant::now(),
            grabber: None,
            current_map: None,
            tile_graph: HashMap::new(),
            missing_texture_material: None,
            texture_still_loading_material: None,
            fly_cam: false,
            terrain_routine: None,
            units_routine: None,
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    fn run_updates(&mut self, renderer: &Arc<Renderer>, delta_time: f32, delta_movement: Vec3A) {
        if self.missing_texture_material.is_none() {
            self.init_missing_texture_material(renderer);
        }

        let app = self.app();

        // TODO: A lot of the things that are done here, are game logic and should belong to the game application (e.g. physics)
        app.logic_update(delta_time);

        app.game_state
            .physics_state
            .notify_delta_movement(delta_movement);

        if !self.fly_cam {
            // TODO: Third Person controls.
            // TODO: if this is required, this is a sign that we're missing adt_to_blender calls on the inputs to the physics simulation,
            //  at least for the player start transform, but potentially also for the terrain meshes
            let mut player_loc = *app.game_state.player_location.read().expect("");
            player_loc += Vec3A::new(0.0, 0.0, 3.0); // TODO: Find out why this number. Capsule Height is barely 2.
            self.camera_location = coordinate_systems::adt_to_blender(player_loc);
        }

        let mm_lock = app.game_state.clone().map_manager.clone();
        {
            let mm = mm_lock.read().expect("Read Lock on Map Manager");
            if mm.current_map.is_some() != self.current_map.is_some() /* initial load or unload */ ||
                (mm.current_map.is_some() && &mm.current_map.as_ref().unwrap().0 != self.current_map.as_ref().unwrap())
            {
                trace!("Map has changed, discarding everything");
                self.tile_graph.clear();
                self.current_map = Some(mm.current_map.as_ref().unwrap().0.clone());

                // TODO: This needs to be more sophisticated, in general it sucks that we just can't call from the packet handler into RenderApplication
                self.camera_location = coordinate_systems::adt_to_blender(
                    *app.game_state
                        .player_location
                        .read()
                        .expect("Read Lock on Player Location"),
                );

                self.camera_yaw = PI
                    - *app
                        .game_state
                        .player_orientation
                        .read()
                        .expect("Read Lock on Player Orientation");
            }

            let added_tiles = mm
                .tile_graph
                .iter()
                .filter(|ki| !self.tile_graph.contains_key(ki.0))
                .collect_vec();
            let removed_tiles = self
                .tile_graph
                .keys()
                .filter(|ki| !mm.tile_graph.contains_key(ki))
                .copied()
                .collect_vec();

            for tile in removed_tiles {
                self.tile_graph.remove(&tile);
            }

            for (key, value) in added_tiles {
                let val = value.clone();
                self.add_tile_graph(renderer, *key, &val);
                self.tile_graph.insert(*key, val);
            }

            for (key, value) in &self.tile_graph {
                // currently, we only update doodads
                self.update_tile_graph(renderer, *key, value);
            }
        }

        // a) We need to drop mm first and b) this should happen after the camera_location has been initially set
        mm_lock
            .write()
            .expect("Write lock on map manager")
            .update_camera(coordinate_systems::blender_to_adt(self.camera_location));
    }

    fn init_missing_texture_material(&mut self, renderer: &Arc<Renderer>) {
        let mat = Material {
            albedo: AlbedoType::Value(Vec4::new(0.22, 1.0, 0.0, 1.0)), // neon/lime green
            transparency: TransparencyType::Opaque,
        };

        let render_mat = Rend3BackendConverter::create_material_from_ir(&mat, None);
        self.missing_texture_material = Some(renderer.add_material(render_mat));

        let mat_loading = Material {
            albedo: AlbedoType::Value(Vec4::new(0.4, 0.4, 0.4, 1.0)),
            transparency: TransparencyType::Opaque,
        };

        self.texture_still_loading_material = Some(renderer.add_material(
            Rend3BackendConverter::create_material_from_ir(&mat_loading, None),
        ))
    }

    #[profiling::function]
    fn update_tile_graph(&self, renderer: &Arc<Renderer>, _tile_pos: (u8, u8), graph: &Arc<ADTNode>) {
        // TODO: All this doesn't have to happen on the render thread. It could even happen inside of
        //  map_manager with interior knowledge of what has changed. One could even chain the
        //  resolver calls to load calls to gpu_load.
        self.load_terrain_chunks(renderer, graph);
        self.load_doodads(renderer, &graph.doodads, None);
        self.load_wmos(renderer, graph);
    }

    fn add_tile_graph(&mut self, renderer: &Arc<Renderer>, tile_pos: (u8, u8), graph: &Arc<ADTNode>) {
        trace!("add_tile_graph: {}, {}", tile_pos.0, tile_pos.1);
        self.update_tile_graph(renderer, tile_pos, graph);
    }

    #[profiling::function]
    fn load_wmos(&self, renderer: &Arc<Renderer>, graph: &Arc<ADTNode>) {
        for wmo_ref in &graph.wmos {
            let wmo = {
                let wmo_arc = wmo_ref.reference.reference.load();
                if wmo_arc.is_none() {
                    continue; // WMO is not resolved yet.
                }

                wmo_arc
                    .as_ref()
                    .expect("WMO has to be loaded (see lines above)")
                    .clone()
            };

            self.load_doodads(renderer, &wmo.doodads, Some(wmo_ref.transform.into()));
            let all_tex_loaded = Self::are_all_textures_loaded(&wmo.tex_references);

            if !all_tex_loaded {
                continue; // TODO: implement delay loading of textures
            }

            for material in &wmo.materials {
                let missing = self
                    .missing_texture_material
                    .as_ref()
                    .expect("Missing Texture Material to be initialized already")
                    .clone();
                Self::load_material(missing, renderer, material, &wmo.tex_references);
            }

            if wmo_ref.obj_handles.read().expect("Obj Handles").is_empty() {
                // First load, we'll be so kind and preallocate
                {
                    let mut handles = Vec::new();
                    for _ in &wmo.subgroups {
                        handles.push(RwLock::new(Vec::new()));
                    }

                    let mut obj_handles_wlock = wmo_ref.obj_handles.write().expect("Obj Handles");
                    *obj_handles_wlock.deref_mut() = handles;
                }
            }

            for (subgroup_id, subgroup_ref) in wmo.subgroups.iter().enumerate() {
                {
                    let handles_lock = wmo_ref.obj_handles.read().expect("Obj Handles");
                    let wmoref_rlock = handles_lock[subgroup_id]
                        .read()
                        .expect("Subgroup Obj Handle Write Lock");

                    if !wmoref_rlock.is_empty() {
                        continue; // This is our "sign", that this subgroup has been rendered already.
                        // TODO: Allow for textures to be delay loaded, similar to doodads.
                    }
                }

                let subgroup = {
                    let subgroup_arc = subgroup_ref.reference.load();

                    if subgroup_arc.is_none() {
                        // not loaded yet
                        continue;
                    }

                    subgroup_arc
                        .as_ref()
                        .expect("Subgroup has to be loaded (see lines above)")
                        .clone()
                };

                let mut object_handles = Vec::with_capacity(subgroup.mesh_batches.len());

                // TODO: probably we should merge all batches into one object
                for (idx, batch) in subgroup.mesh_batches.iter().enumerate() {
                    let mat_id = subgroup.material_ids[idx];

                    // TODO: This may still fail async, we haven't ensured that all required materials (and especially their textures) are resolved.
                    let material_handle = if mat_id != 0xFF {
                        let mat_rw = wmo.materials[mat_id as usize]
                            .read()
                            .expect("Material read lock");

                        mat_rw
                            .handle
                            .as_ref()
                            .expect("Material to be loaded (right above)")
                            .clone()
                    } else {
                        // TODO: this is not exactly correct, we should probably have a "no mat" material.
                        //  and especially for WMO Groups, they probably have a default material anyway
                        self.texture_still_loading_material
                            .as_ref()
                            .expect("Texture Still Loading Material to be initialized")
                            .clone()
                    };

                    let mesh_handle = gpu_loaders::gpu_load_mesh(renderer, batch);
                    let object = rend3::types::Object {
                        mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
                        material: material_handle.clone(),
                        transform: wmo_ref.transform.into(),
                    };

                    object_handles.push(renderer.add_object(object));
                }

                {
                    let handles_lock = wmo_ref.obj_handles.read().expect("Obj Handles");
                    let mut wmoref_wlock = handles_lock[subgroup_id]
                        .write()
                        .expect("Subgroup Obj Handle Write Lock");
                    *wmoref_wlock.deref_mut() = object_handles;
                }
            }
        }
    }

    #[profiling::function]
    fn load_terrain_chunks(&self, renderer: &Arc<Renderer>, graph: &Arc<ADTNode>) {
        for tile in &graph.terrain {
            {
                let rlock = tile.object_handle.read().expect("Object Handle Read Lock");
                if rlock.is_some() {
                    continue;
                }
            }

            let loaded_texture_layers = tile
                .texture_layers
                .iter()
                .map(|layer| {
                    let base_layer =
                        gpu_loaders::gpu_load_texture(renderer, &layer.base_texture_ref.reference).unwrap();

                    let alpha_layer = layer.alpha_map_ref.as_ref().map(|alpha_ref| {
                        // TODO: Since this code is completely ugly anyway, we can also right away take the write lock instead of checking for previous success.
                        //  the whole principle and "API" will probably need a good overhaul anyway.

                        let mut wlock = alpha_ref.write().expect("Alpha Handle Write Lock");

                        let alpha_tex = Texture {
                            label: Some(format!("Alpha Layer Terrain {}", tile.position)),
                            data: wlock.data.clone(),
                            format: rend3::types::TextureFormat::R8Unorm,
                            size: UVec2::new(64, 64),
                            mip_count: rend3::types::MipmapCount::ONE,
                            mip_source: rend3::types::MipmapSource::Uploaded,
                        };

                        let alpha_handle = renderer
                            .add_texture_2d(alpha_tex)
                            .expect("Texture creation successful");

                        wlock.handle = Some(alpha_handle.clone());
                        alpha_handle
                    });

                    (base_layer, alpha_layer)
                })
                .collect_vec();

            let mut wlock = tile
                .object_handle
                .write()
                .expect("Object Handle Write Lock");

            assert!(
                !loaded_texture_layers.is_empty(),
                "At least one texture layer has to be present"
            );

            let base_texture = loaded_texture_layers[0].0.clone();
            let mut additional_layers = [const { None }; 6];

            for (idx, (base, alpha_opt)) in loaded_texture_layers.iter().skip(1).enumerate() {
                if idx > 2 {
                    warn!("Terrain: Skipping texture layer {}, only 4 supported", idx);
                    break;
                }

                if let Some(alpha) = alpha_opt {
                    additional_layers[2 * idx] = Some(base.clone());
                    additional_layers[2 * idx + 1] = Some(alpha.clone());
                } else {
                    warn!("Terrain: Skipping texture layer {}, missing alpha map", idx);
                }
            }

            let material = TerrainMaterial {
                base_texture,
                additional_layers,
            };
            let material_handle = renderer.add_material(material);
            let mesh_handle = gpu_loaders::gpu_load_mesh(renderer, &tile.mesh);

            let object = rend3::types::Object {
                mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
                material: material_handle.clone(),
                transform: coordinate_systems::adt_to_blender_transform(Vec3A::new(
                    tile.position.x,
                    tile.position.y,
                    tile.position.z,
                )),
            };

            *wlock.deref_mut() = Some(renderer.add_object(object));
        }
    }

    #[profiling::function]
    fn load_doodads(
        &self,
        renderer: &Arc<Renderer>,
        doodads: &Vec<Arc<DoodadReference>>,
        parent_transform: Option<Mat4>,
    ) {
        for doodad in doodads {
            // TODO: we need a better logic to express the desire to actually render something, because then we can explicitly load to the gpu

            if doodad.renderer_is_complete.load(Ordering::Acquire) {
                continue;
            }

            // TODO: technically we have a race condition here, while we load the stuff on the GPU, it may have changed loader side. In general we have no concept of updating yet.
            let m2 = {
                // TODO: This pattern is so common, but due to the continue I think we can't deduplicate this into a method
                let doodad_arc = doodad.reference.reference.load();
                if doodad_arc.is_none() {
                    continue;
                }

                doodad_arc
                    .as_ref()
                    .expect("previous is_none check.")
                    .clone()
            };

            //  TODO: This ignores dynamic textures, but I think they can't be supported here anyway.
            let all_tex_loaded = Self::are_all_textures_loaded(&m2.tex_reference);

            // We did the first pass, creating an object with a "loading texture" material, so we are waiting for the
            // second, but unfortunately, textures still aren't loaded.
            if doodad.renderer_waiting_for_textures.load(Ordering::Acquire) && !all_tex_loaded {
                continue;
            }

            let mm = self.app().game_state.clone().map_manager.clone();
            let tex_resolver = {
                mm.read()
                    .expect("Map Manager Read poisoned")
                    .tex_resolver
                    .clone()
            };

            for (mesh, material, _) in m2.meshes_and_materials.iter() {
                let material_handle = if all_tex_loaded {
                    let missing = self
                        .missing_texture_material
                        .as_ref()
                        .expect("Missing Texture Material to be initialized already")
                        .clone();

                    Self::load_material_m2(missing, renderer, material, &tex_resolver, &[])
                } else {
                    self.texture_still_loading_material
                        .as_ref()
                        .expect("Material already initialized")
                        .clone()
                };

                // TODO: handle the absence of the tex_reference. Currently this will render the missing texture style, but I guess when we _know_ the texture is not ready yet, we should load an albedo grey material.

                let mesh_handle = gpu_loaders::gpu_load_mesh(renderer, mesh);
                let object = rend3::types::Object {
                    mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
                    material: material_handle.clone(),
                    transform: parent_transform.unwrap_or(Mat4::IDENTITY) * doodad.transform,
                };

                let mut handle_writer = doodad.renderer_object_handles.blocking_write();
                handle_writer.push(renderer.add_object(object));

                doodad
                    .renderer_waiting_for_textures
                    .store(!all_tex_loaded, Ordering::SeqCst);

                if all_tex_loaded {
                    doodad.renderer_is_complete.store(true, Ordering::SeqCst);
                }
            }
        }
    }

    pub fn are_all_textures_loaded(tex_reference: &Vec<Arc<IRTextureReference>>) -> bool {
        !tex_reference
            .iter()
            .any(|tex| tex.reference.load().is_none())
    }

    pub fn load_material(
        missing_texture_material: MaterialHandle,
        renderer: &Arc<Renderer>,
        material: &RwLock<IRMaterial>,
        tex_references: &Vec<Arc<IRTextureReference>>,
    ) -> MaterialHandle {
        // I think here we have the first important "lazy" design: we'll only gpu load the
        // texture that we need for our material.
        let tex_name_opt = {
            let mat_rlock = material.read().expect("Material read lock");
            match &mat_rlock.data.albedo {
                AlbedoType::TextureWithName(name) => Some(name.clone()),
                _ => None,
            }
        };

        let texture_handle_opt: Option<Texture2DHandle> = match &tex_name_opt {
            Some(tex_name) => tex_references
                .iter()
                .find(|tex_ref| tex_name.eq(&tex_ref.reference_str))
                .and_then(|tex_ref| gpu_loaders::gpu_load_texture(renderer, &tex_ref.reference)),
            _ => None,
        };

        if tex_name_opt.is_some() && texture_handle_opt.is_none() {
            // warn!(
            //     "Failed loading texture {}, falling back",
            //     tex_name_opt.unwrap()
            // );
            missing_texture_material
        } else {
            gpu_loaders::gpu_load_material(renderer, material, texture_handle_opt)
        }
    }

    pub fn load_material_m2(
        missing_texture_material: MaterialHandle,
        renderer: &Arc<Renderer>,
        material: &RwLock<IRM2Material>,
        tex_resolver: &Resolver<M2Generator, RwLock<Option<IRTexture>>>,
        dynamic_textures: &[(
            M2TextureType,
            M2TextureFlags,
            Arc<RwLock<Option<IRTexture>>>,
        )],
    ) -> MaterialHandle {
        let mut write = material.write().expect("Material read lock poisoned");
        let textures = write
            .data
            .textures
            .iter()
            .filter_map(|tex| {
                if tex.texture_type == M2TextureType::None {
                    Some(tex_resolver.resolve(tex.filename.clone()))
                } else {
                    // The assumption here is that texture types are unique
                    dynamic_textures
                        .iter()
                        .find(|(dyn_tex_type, _, _)| *dyn_tex_type == tex.texture_type)
                        .map(|(_, _, tex)| tex.clone())
                }
            })
            .map(|tex| gpu_loaders::gpu_load_texture(renderer, &ArcSwapOption::new(Some(tex))))
            .collect_vec();

        // TODO: There's still the edge-case where loading may have failed and it's thus declared as None. But that requires
        //  refactoring at some point.
        let all_tex_loaded = textures.iter().all(|tex| tex.is_some());

        if !all_tex_loaded {
            return missing_texture_material;
        }

        if textures.len() > 3 {
            warn!("UnitsMaterial is currently not supporting more than 3 textures");
        }

        let mut rend_material = UnitsMaterial {
            albedo: UnitsAlbedo::Textures([const { None }; 3]),
            alpha_cutout: Some(0.5), // TODO: Derive from M2Material that we haven't even implemented.
        };

        for (idx, tex) in textures.into_iter().enumerate() {
            match &mut rend_material.albedo {
                UnitsAlbedo::Textures(texture_layers) => texture_layers[idx] = tex,
                _ => (),
            }
        }

        let handle = renderer.add_material(rend_material);
        write.handle = Some(handle.clone());
        handle
    }
}

fn button_pressed<Hash: BuildHasher>(map: &HashMap<KeyCode, bool, Hash>, key: KeyCode) -> bool {
    map.get(&key).is_some_and(|b| *b)
}

impl rend3_framework::App for RenderingApplication {
    const HANDEDNESS: Handedness = Handedness::Right;

    fn register_logger(&mut self) {
        // intentionally no-opped.
    }

    // On android, we need to somehow take the event loop we get from the entry point.
    #[cfg(target_os = "android")]
    fn create_window(&mut self, builder: WindowBuilder) -> Result<(EventLoop<()>, Window), EventLoopError> {}

    fn create_base_rendergraph(&mut self, renderer: &Arc<Renderer>, spp: &mut ShaderPreProcessor) -> BaseRenderGraph {
        let mut data_core = renderer.data_core.lock();
        let render_graph = BaseRenderGraph::new(renderer, spp);
        self.terrain_routine = Some(Mutex::new(TerrainRoutine::new(
            renderer,
            &mut data_core,
            spp,
            &render_graph.interfaces,
        )));

        self.units_routine = Some(Mutex::new(UnitsRoutine::new(
            renderer,
            &mut data_core,
            spp,
            &render_graph.interfaces,
        )));

        drop(data_core);

        render_graph
    }

    fn sample_count(&self) -> SampleCount {
        SampleCount::One // No MSAA yet
    }

    fn present_mode(&self) -> PresentMode {
        PresentMode::AutoVsync
    }

    fn setup(&mut self, context: SetupContext<'_, ()>) {
        // Push the Renderer into the GameApplication to preload handles.
        if self
            .app
            .upgrade()
            .expect("Application to be initialized")
            .renderer
            .set(context.renderer.clone())
            .is_err()
        {
            panic!("Setting the renderer on Application failed: already initialized");
        }

        self.grabber = context
            .windowing
            .map(|windowing| Grabber::new(windowing.window));
    }

    // TODO: Look at the lifecycles again, compare e.g. https://github.com/BVE-Reborn/rend3/blob/trunk/examples/scene-viewer/src/lib.rs#L572
    fn handle_event(&mut self, context: EventContext<'_>, event: Event<()>) {
        match event {
            // Close button was clicked, we should close.
            Event::LoopExiting => {
                if let Some(app) = self.app.upgrade() {
                    app.close_requested.store(true, Ordering::SeqCst);

                    // TODO: How do we want to design shutdowns? Shouldn't every loop just honor the close_requested flag?
                    //  but then there may be valid reasons to have a separate conditional especially for physics, but
                    //  requesting a close should also call stop there.
                    app.game_state.physics_state.stop();
                };
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(focus),
                ..
            } => {
                if !focus {
                    self.grabber
                        .as_mut()
                        .unwrap()
                        .request_ungrab(context.window.as_ref().unwrap());
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key,
                                state,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                let PhysicalKey::Code(scancode) = physical_key else {
                    warn!("Non physical (?) key {:?}", physical_key);
                    return;
                };

                self.scancode_status.insert(
                    scancode,
                    match state {
                        ElementState::Pressed => true,
                        ElementState::Released => false,
                    },
                );
            }
            // Other events we don't care about
            _ => {}
        }
    }

    fn handle_redraw(&mut self, context: RedrawContext<'_>) {
        profiling::scope!("RedrawRequested");

        let app = self.app();

        {
            profiling::scope!("Update Logic");
            let now = Instant::now();
            let delta_time = now - self.timestamp_last_frame;
            self.timestamp_last_frame = now;

            let rotation = if self.fly_cam {
                glam::Mat3A::from_euler(
                    glam::EulerRot::XYZ,
                    -self.camera_pitch * PI,
                    0.0 /* roll */ * PI,
                    -self.camera_yaw,
                )
            } else {
                // We don't want our forward movement to be dictated by the pitch, this is, at best, useful for the fly cam.
                glam::Mat3A::from_euler(
                    glam::EulerRot::XYZ,
                    0.0, /* pitch */
                    0.0 /* roll */ * PI,
                    -self.camera_yaw,
                )
            };

            let forward: Vec3A = rotation.y_axis; // TODO: Only if fly cam do we want to use pitch. Probably we never want that at all?
            let right: Vec3A = rotation.x_axis;
            let up: Vec3A = rotation.z_axis;

            let fwd_speed = if self.fly_cam { 30.0 } else { 7.0 };
            let strafe_speed = if self.fly_cam { 20.0 } else { 7.0 };
            let back_speed = if self.fly_cam { 20.0 } else { 4.5 };

            let mut delta: Vec3A = Vec3A::new(0.0, 0.0, 0.0);
            let mut yaw = 0.0;

            if button_pressed(&self.scancode_status, KeyCode::KeyW) {
                delta += forward * fwd_speed * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::KeyS) {
                delta -= forward * back_speed * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::KeyA) {
                delta -= right * strafe_speed * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::KeyD) {
                delta += right * strafe_speed * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::KeyF) {
                self.fly_cam = !self.fly_cam;
            }
            if button_pressed(&self.scancode_status, KeyCode::ShiftLeft) {
                delta += up * 10.0 * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::ControlLeft) {
                delta -= up * 10.0 * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::ArrowRight) {
                yaw += PI * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::ArrowLeft) {
                yaw -= PI * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::ArrowUp) {
                self.camera_pitch += 0.25 * delta_time.as_secs_f32();
            }
            if button_pressed(&self.scancode_status, KeyCode::ArrowDown) {
                self.camera_pitch -= 0.25 * delta_time.as_secs_f32();
            }

            if self.fly_cam {
                self.camera_location += delta;
                self.camera_yaw += yaw;
            } else {
                self.camera_yaw += yaw;

                // location: we will consider the delta for the physics movement and then mirror that to the cam
                let mut player_wlock = app
                    .game_state
                    .player_orientation
                    .write()
                    .expect("Player orientation tainted");

                let orientation = player_wlock.deref_mut();
                *orientation -= yaw;

                // fancy clamping from -PI to +PI.
                while *orientation < -PI {
                    *orientation += 2.0 * PI;
                }

                while *orientation > PI {
                    *orientation -= 2.0 * PI;
                }
            }

            self.run_updates(
                context.renderer,
                delta_time.as_secs_f32(),
                if self.fly_cam { Vec3A::ZERO } else { delta },
            );
        }

        context.window.unwrap().request_redraw();

        let (clear_color, ambient_color) = {
            let mut mm = app
                .game_state
                .map_manager
                .write()
                .expect("Map Manager Write Lock");

            let game_time = app.game_state.game_time.as_30s_ticks();

            if let Some((ambient, diffuse)) = mm.current_light_settings.as_ref().map(|settings| {
                (
                    interpolate_color(&settings.clear.ambient_color.data, game_time),
                    interpolate_color(&settings.clear.diffuse_color.data, game_time),
                )
            }) {
                mm.sunlight
                    .update(context.renderer, game_time, diffuse.xyz());
                // ensure we don't go pitch black.
                let base_ambient = Vec4::new(0.03, 0.03, 0.03, 1.0);
                Some((diffuse, ambient.max(base_ambient)))
            } else {
                None
            }
        }
        .unwrap_or((
            Vec4::new(0.10, 0.05, 0.10, 1.0), // Nice scene-referred purple
            Vec4::new(0.25, 0.25, 0.25, 1.0),
        ));

        // technically, we could also invert the view rotation (remember this is not the cams matrix, but the _view_ matrix, so how do you transform
        // the world to get to the screen (i.e. 0, 0). Hence we also need to invert the camera_location. Inverting the rotation isn't a deal though,
        // as we can just control the input angles.

        //let view = Mat4::from_euler(glam::EulerRot::XYZ, -self.camera_pitch + 0.5 * PI, -self.camera_yaw, 0.0);
        let view = Mat4::from_euler(
            glam::EulerRot::XYZ,
            (-0.5 - self.camera_pitch) * PI,
            0.0 /* roll */ * PI,
            self.camera_yaw,
        );
        let view = view * Mat4::from_translation((-self.camera_location).into());

        context.renderer.set_camera_data(Camera {
            projection: CameraProjection::Perspective {
                vfov: 90.0,
                near: 0.1,
            },
            view,
        });

        // Swap the instruction buffers so that our frame's changes can be processed.
        context.renderer.swap_instruction_buffers();
        // Evaluate our frame's world-change instructions
        let mut eval_output = context.renderer.evaluate_instructions();

        // Lock the routines
        let pbr_routine = rend3_framework::lock(&context.routines.pbr);
        let tonemapping_routine = rend3_framework::lock(&context.routines.tonemapping);
        let terrain_routine = self
            .terrain_routine
            .as_ref()
            .expect("terrain to be setup")
            .lock()
            .expect("Terrain Routine Lock");
        let units_routine = self
            .units_routine
            .as_ref()
            .expect("units routine to be setup")
            .lock()
            .expect("Units Routine Lock");

        // Build a rendergraph
        let mut graph = rend3::graph::RenderGraph::new();

        // Import the surface texture into the render graph.
        let frame_handle = graph.add_imported_render_target(
            context.surface_texture,
            0..1,
            0..1,
            rend3::graph::ViewportRect::from_size(context.resolution),
        );

        base_rendergraph_add_to_graph(
            context.base_rendergraph,
            &mut graph,
            rend3_routine::base::BaseRenderGraphInputs {
                eval_output: &eval_output,
                routines: BaseRenderGraphRoutines {
                    pbr: &pbr_routine,
                    skybox: None,
                    tonemapping: &tonemapping_routine,
                },
                target: OutputRenderTarget {
                    handle: frame_handle,
                    resolution: context.resolution,
                    samples: SampleCount::One,
                },
            },
            rend3_routine::base::BaseRenderGraphSettings {
                ambient_color,
                clear_color,
            },
            &terrain_routine,
            &units_routine,
        );

        // Dispatch a render using the built up rendergraph!
        graph.execute(context.renderer, &mut eval_output);

        profiling::finish_frame!();
    }
}

#[allow(clippy::too_many_arguments)]
fn base_rendergraph_add_to_graph<'node>(
    base_graph: &'node BaseRenderGraph,
    graph: &mut RenderGraph<'node>,
    inputs: BaseRenderGraphInputs<'_, 'node>,
    settings: BaseRenderGraphSettings,
    terrain_routine: &'node TerrainRoutine,
    units_routine: &'node UnitsRoutine,
) {
    // Create the data and handles for the graph.
    let mut state = BaseRenderGraphIntermediateState::new(graph, inputs, settings);

    // Clear the shadow buffers. This, as an explicit node, must be done as a limitation of the graph dependency system.
    // state.clear_shadow_buffers();
    clear::add_depth_clear_to_graph(state.graph, state.shadow, 0.0);

    // Prepare all the uniforms that all shaders need access to.
    state.create_frame_uniforms(base_graph);

    // Perform compute based skinning.
    state.skinning(base_graph);

    // Render all the shadows to the shadow map.
    state.pbr_shadow_rendering();

    units_routine
        .opaque_routine
        .add_forward_to_graph(ForwardRoutineArgs {
            graph: state.graph,
            label: "Units Forward Pass",
            camera: CameraSpecifier::Viewport,
            binding_data: forward::ForwardRoutineBindingData {
                whole_frame_uniform_bg: state.forward_uniform_bg,
                per_material_bgl: &units_routine.per_material,
                extra_bgs: None,
            },
            samples: state.inputs.target.samples,
            renderpass: state.primary_renderpass.clone(),
        });

    // Render after units, for less overdraw.
    terrain_routine
        .opaque_routine
        .add_forward_to_graph(ForwardRoutineArgs {
            graph: state.graph,
            label: "Terrain Forward Pass",
            camera: CameraSpecifier::Viewport,
            binding_data: forward::ForwardRoutineBindingData {
                whole_frame_uniform_bg: state.forward_uniform_bg,
                per_material_bgl: &terrain_routine.per_material,
                extra_bgs: None,
            },
            samples: state.inputs.target.samples,
            renderpass: state.primary_renderpass.clone(),
        });

    // Do the first pass, rendering the predicted triangles from last frame.
    state.pbr_render();

    // Render the skybox.
    state.skybox();

    // Render all transparent objects.
    //
    // This _must_ happen after culling, as all transparent objects are
    // considered "residual".
    state.pbr_forward_rendering_transparent();

    // Tonemap the HDR inner buffer to the output buffer.
    state.tonemapping();
}
