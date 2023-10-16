use std::collections::HashMap;
use std::f32::consts::PI;
use std::hash::BuildHasher;
use std::ops::DerefMut;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock, Weak};
use std::time::Instant;

use glam::{Mat4, UVec2, Vec3A, Vec4};
use itertools::Itertools;
use log::trace;
use rend3::types::{
    Camera, CameraProjection, Handedness, MaterialHandle, PresentMode, SampleCount, Surface, Texture2DHandle,
    TextureFormat,
};
use rend3::util::typedefs::FastHashMap;
use rend3::Renderer;
use rend3_framework::{DefaultRoutines, Event, Grabber, UserResizeEvent};
use rend3_routine::base::BaseRenderGraph;
use winit::event::{ElementState, KeyboardInput, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

use crate::game::application::GameApplication;
use crate::rendering::asset_graph::nodes::adt_node::{ADTNode, DoodadReference, IRMaterial, IRTextureReference};
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::types::{AlbedoType, Material, TransparencyType};
use crate::rendering::rend3_backend::{gpu_loaders, Rend3BackendConverter};

// pub trait RendererCommand {
//     fn run(&self);
// }

// #[derive(Debug)] // TODO: Ensure Grabber implements Display
pub struct RenderingApplication {
    scancode_status: FastHashMap<u32, bool>,
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
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    fn run_updates(&mut self, renderer: &Arc<Renderer>) {
        if self.missing_texture_material.is_none() {
            self.init_missing_texture_material(renderer);
        }

        let app = self.app();
        let mm_lock = app.game_state.clone().map_manager.clone();
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
            self.camera_yaw = *app
                .game_state
                .player_orientation
                .read()
                .expect("Read Lock on Player Orientation")
                - PI * 0.5;
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

    fn init_missing_texture_material(&mut self, renderer: &Arc<Renderer>) {
        let mat = Material {
            is_unlit: true,
            albedo: AlbedoType::Value(Vec4::new(0.22, 1.0, 0.0, 1.0)), // neon/lime green
            transparency: TransparencyType::Opaque,
        };

        let render_mat = Rend3BackendConverter::create_material_from_ir(&mat, None);
        self.missing_texture_material = Some(renderer.add_material(render_mat));

        let mat_loading = Material {
            is_unlit: true,
            albedo: AlbedoType::Value(Vec4::new(0.4, 0.4, 0.4, 1.0)),
            transparency: TransparencyType::Opaque,
        };

        self.texture_still_loading_material = Some(renderer.add_material(
            Rend3BackendConverter::create_material_from_ir(&mat_loading, None),
        ))
    }

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

    fn load_wmos(&self, renderer: &Arc<Renderer>, graph: &Arc<ADTNode>) {
        for wmo_ref in &graph.wmos {
            let wmo = {
                let wmo_rlock = wmo_ref.reference.reference.read().expect("WMO Read Lock");
                if wmo_rlock.is_none() {
                    continue; // WMO is not loaded yet.
                }

                wmo_rlock
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
                self.load_material(renderer, material, &wmo.tex_references);
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
                    let subgroup_rlock = subgroup_ref.reference.read().expect("Subgroup Read Lock");

                    if subgroup_rlock.is_none() {
                        // not loaded yet
                        continue;
                    }

                    subgroup_rlock
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

    fn load_terrain_chunks(&self, renderer: &Arc<Renderer>, graph: &Arc<ADTNode>) {
        // TODO: Currently one hardcoded material per adt
        let _material = Material {
            is_unlit: true,
            albedo: AlbedoType::Vertex { srgb: true },
            transparency: TransparencyType::Opaque,
        };
        let material = Rend3BackendConverter::create_material_from_ir(&_material, None);
        let material_handle = renderer.add_material(material);

        for tile in &graph.terrain {
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

            let mut wlock = tile
                .object_handle
                .write()
                .expect("Object Handle Write Lock");
            *wlock.deref_mut() = Some(renderer.add_object(object));
        }
    }

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
                // TODO: Async aware RwLock
                let m2_rlock = doodad.reference.reference.read().expect("M2 Read Lock");
                if m2_rlock.is_none() {
                    continue;
                }

                m2_rlock.as_ref().expect("previous is_none check.").clone()
            };

            let all_tex_loaded = Self::are_all_textures_loaded(&m2.tex_reference);
            let has_object_handle = { doodad.renderer_object_handle.blocking_read().is_some() };

            if has_object_handle && !all_tex_loaded {
                // We're waiting on textures and that hasn't changed yet.
                continue;
            }

            let material_handle = if all_tex_loaded {
                self.load_material(renderer, &m2.material, &m2.tex_reference)
            } else {
                self.texture_still_loading_material
                    .as_ref()
                    .expect("Material already initialized")
                    .clone()
            };

            // TODO: handle the absence of the tex_reference. Currently this will render the missing texture style, but I guess when we _know_ the texture is not ready yet, we should load an albedo grey material.

            let mesh_handle = gpu_loaders::gpu_load_mesh(renderer, &m2.mesh);
            let object = rend3::types::Object {
                mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
                material: material_handle.clone(),
                transform: (parent_transform.unwrap_or(Mat4::IDENTITY) * doodad.transform),
            };

            let mut handle_writer = doodad.renderer_object_handle.blocking_write();
            *handle_writer.deref_mut() = Some(renderer.add_object(object));

            if all_tex_loaded {
                doodad.renderer_is_complete.store(true, Ordering::SeqCst);
            }
        }
    }

    fn are_all_textures_loaded(tex_reference: &Vec<Arc<IRTextureReference>>) -> bool {
        !tex_reference.iter().any(|tex| {
            tex.reference
                .read()
                .expect("tex reference read lock")
                .is_none()
        })
    }

    fn load_material(
        &self,
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

        let material_handle = if tex_name_opt.is_some() && texture_handle_opt.is_none() {
            // warn!(
            //     "Failed loading texture {}, falling back",
            //     tex_name_opt.unwrap()
            // );
            self.missing_texture_material
                .as_ref()
                .expect("Missing Texture Material to be initialized already")
                .clone()
        } else {
            gpu_loaders::gpu_load_material(renderer, material, texture_handle_opt)
        };
        material_handle
    }
}

fn button_pressed<Hash: BuildHasher>(map: &HashMap<u32, bool, Hash>, key: u32) -> bool {
    map.get(&key).map_or(false, |b| *b)
}

impl rend3_framework::App for RenderingApplication {
    const HANDEDNESS: Handedness = Handedness::Right;

    fn register_logger(&mut self) {
        // intentionally no-opped.
    }

    fn sample_count(&self) -> SampleCount {
        SampleCount::One // No MSAA yet
    }

    fn present_mode(&self) -> PresentMode {
        PresentMode::AutoVsync
    }

    fn setup(
        &mut self,
        _event_loop: &EventLoop<UserResizeEvent<()>>,
        window: &Window,
        renderer: &Arc<Renderer>,
        _routines: &Arc<DefaultRoutines>,
        _surface_format: TextureFormat,
    ) {
        // Push the Renderer into the GameApplication to preload handles.
        if self
            .app
            .upgrade()
            .expect("Application to be initialized")
            .renderer
            .set(renderer.clone())
            .is_err()
        {
            panic!("Setting the renderer on Application failed: already initialized");
        }

        self.grabber = Some(Grabber::new(window));
    }

    fn handle_event(
        &mut self,
        window: &Window,
        renderer: &Arc<Renderer>,
        routines: &Arc<DefaultRoutines>,
        base_rendergraph: &BaseRenderGraph,
        surface: Option<&Arc<Surface>>,
        resolution: UVec2,
        event: Event<'_, ()>,
        control_flow: impl FnOnce(ControlFlow),
    ) {
        match event {
            // Close button was clicked, we should close.
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                control_flow(ControlFlow::Exit);
                if let Some(app) = self.app.upgrade() {
                    app.close_requested.store(true, Ordering::SeqCst)
                };
            }
            Event::MainEventsCleared => {
                let now = Instant::now();
                let delta_time = now - self.timestamp_last_frame;
                self.timestamp_last_frame = now;

                let rotation = glam::Mat3A::from_euler(
                    glam::EulerRot::XYZ,
                    -self.camera_pitch * PI,
                    0.0 /* roll */ * PI,
                    -self.camera_yaw * PI,
                );
                let forward: Vec3A = rotation.y_axis;
                let right: Vec3A = rotation.x_axis;
                let up: Vec3A = rotation.z_axis;

                // TODO: https://github.com/BVE-Reborn/rend3/blob/trunk/examples/scene-viewer/src/platform.rs. Make platform independent and also add more, or search other crate, rather.
                if button_pressed(&self.scancode_status, 17u32) {
                    // W
                    self.camera_location += forward * 30.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 31u32) {
                    // S
                    self.camera_location -= forward * 20.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 30u32) {
                    // A
                    self.camera_location -= right * 20.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 32u32) {
                    // D
                    self.camera_location += right * 20.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 42u32) {
                    // LSHIFT
                    self.camera_location += up * 10.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 29u32) {
                    self.camera_location -= up * 10.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 57421u32) {
                    // arrow right
                    self.camera_yaw += 0.5 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 57419u32) {
                    // arrow left
                    self.camera_yaw -= 0.5 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 57416u32) {
                    self.camera_pitch += 0.25 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 57424u32) {
                    self.camera_pitch -= 0.25 * delta_time.as_secs_f32();
                }

                self.run_updates(renderer);

                window.request_redraw();
            }
            // Render!
            Event::RedrawRequested(_) => {
                // technically, we could also invert the view rotation (remember this is not the cams matrix, but the _view_ matrix, so how do you transform
                // the world to get to the screen (i.e. 0, 0). Hence we also need to invert the camera_location. Inverting the rotation isn't a deal though,
                // as we can just control the input angles.

                //let view = Mat4::from_euler(glam::EulerRot::XYZ, -self.camera_pitch + 0.5 * PI, -self.camera_yaw, 0.0);
                let view = Mat4::from_euler(
                    glam::EulerRot::XYZ,
                    (-0.5 - self.camera_pitch) * PI,
                    0.0 /* roll */ * PI,
                    self.camera_yaw * PI,
                );
                let view = view * Mat4::from_translation((-self.camera_location).into());

                renderer.set_camera_data(Camera {
                    projection: CameraProjection::Perspective {
                        vfov: 90.0,
                        near: 0.1,
                    },
                    view,
                });

                // Get a frame
                let frame = surface.unwrap().get_current_texture().unwrap();

                // Swap the instruction buffers so that our frame's changes can be processed.
                renderer.swap_instruction_buffers();
                // Evaluate our frame's world-change instructions
                let mut eval_output = renderer.evaluate_instructions();

                // Lock the routines
                let pbr_routine = rend3_framework::lock(&routines.pbr);
                let tonemapping_routine = rend3_framework::lock(&routines.tonemapping);

                // Build a rendergraph
                let mut graph = rend3::graph::RenderGraph::new();

                // Import the surface texture into the render graph.
                let frame_handle = graph.add_imported_render_target(
                    &frame,
                    0..1,
                    rend3::graph::ViewportRect::from_size(resolution),
                );
                // Add the default rendergraph without a skybox
                base_rendergraph.add_to_graph(
                    &mut graph,
                    &eval_output,
                    &pbr_routine,
                    None,
                    &tonemapping_routine,
                    frame_handle,
                    resolution,
                    self.sample_count(),
                    glam::Vec4::ZERO,
                    glam::Vec4::new(0.10, 0.05, 0.10, 1.0), // Nice scene-referred purple
                );

                // Dispatch a render using the built up rendergraph!
                graph.execute(renderer, &mut eval_output);

                // Present the frame
                frame.present();
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(focus),
                ..
            } => {
                if !focus {
                    self.grabber.as_mut().unwrap().request_ungrab(window);
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            scancode, state, ..
                        },
                        ..
                    },
                ..
            } => {
                //log::trace!("WE scancode {:x}", scancode);
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
}
