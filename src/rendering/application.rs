use crate::game::application::GameApplication;
use crate::rendering::asset_graph::nodes::adt_node::{
    ADTNode, DoodadReference, IRMaterial, IRMesh, IRTexture, IRTextureReference, M2Node,
};
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::highlevel_types::PlacedDoodad;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod, TransparencyType};
use crate::rendering::rend3_backend::Rend3BackendConverter;
use crate::rendering::{add_placed_doodads, add_terrain_chunks, add_wmo_groups};
use glam::{Affine3A, Mat4, UVec2, Vec3, Vec3A, Vec4};
use image_blp::BlpImage;
use itertools::Itertools;
use log::trace;
use rend3::types::{
    Camera, CameraProjection, Handedness, MaterialHandle, MaterialTag, MeshHandle, ObjectHandle, PresentMode,
    ResourceHandle, SampleCount, Surface, Texture2DHandle, TextureFormat,
};
use rend3::util::typedefs::FastHashMap;
use rend3::Renderer;
use rend3_framework::{DefaultRoutines, Event, Grabber, UserResizeEvent};
use rend3_routine::base::BaseRenderGraph;
use sargerust_files::adt::types::ADTAsset;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::hash::BuildHasher;
use std::ops::DerefMut;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock, Weak};
use std::time::Instant;
use winit::event::{ElementState, KeyboardInput, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

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
    loaded_tiles: HashMap<
        (u8, u8),
        (
            Arc<ADTAsset>,
            /* Terrain */ Vec<(Vec3, Mesh)>,
            Vec<PlacedDoodad>,
            /* WMO */ Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>,
            HashMap<String, BlpImage>,
        ),
    >,
    object_list: Vec<ObjectHandle>, // TODO: Refactor to the DAG
    tile_graph: HashMap<(u8, u8), Arc<ADTNode>>,
    missing_texture_material: Option<MaterialHandle>,
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
            loaded_tiles: HashMap::new(),
            object_list: vec![],
            tile_graph: HashMap::new(),
            missing_texture_material: None,
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
            self.loaded_tiles.clear();
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
            //.loaded_tiles
            .tile_graph
            .iter()
            //.filter(|ki| !self.loaded_tiles.contains_key(ki.0))
            .filter(|ki| !self.tile_graph.contains_key(ki.0))
            .collect_vec();
        let removed_tiles = self
            //.loaded_tiles
            .tile_graph
            .keys()
            //.filter(|ki| !mm.loaded_tiles.contains_key(ki))
            .filter(|ki| !mm.tile_graph.contains_key(ki))
            .copied()
            .collect_vec();

        for tile in removed_tiles {
            //self.loaded_tiles.remove(&tile);
            self.tile_graph.remove(&tile);
        }

        for (key, value) in added_tiles {
            let val = value.clone();
            //self.add_tile(renderer, *key, &val);
            self.add_tile_graph(renderer, *key, &val);
            //self.loaded_tiles.insert(*key, val);
            self.tile_graph.insert(*key, val);
        }
    }

    fn init_missing_texture_material(&mut self, renderer: &Arc<Renderer>) {
        let mat = Material {
            is_unlit: true,
            albedo: AlbedoType::Value(Vec4::new(1.0, 0.0, 0.5, 1.0)), // screaming pink
            transparency: TransparencyType::Opaque,
        };

        let render_mat = Rend3BackendConverter::create_material_from_ir(&mat, None);
        self.missing_texture_material = Some(renderer.add_material(render_mat));
    }

    fn add_tile(
        &mut self,
        renderer: &Arc<Renderer>,
        tile_pos: (u8, u8),
        tile: &(
            Arc<ADTAsset>,
            Vec<(Vec3, Mesh)>,
            Vec<PlacedDoodad>,
            Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>,
            HashMap<String, BlpImage>,
        ),
    ) {
        let (adt, terrain_chunk, placed_doodads, wmos, textures) = tile;
        add_placed_doodads(placed_doodads, renderer, &mut self.object_list);
        add_wmo_groups(
            wmos.iter().map(|w| (&w.0, &w.1)),
            textures,
            renderer,
            &mut self.object_list,
        );
        add_terrain_chunks(terrain_chunk, renderer, &mut self.object_list);
    }

    fn add_tile_graph(&mut self, renderer: &Arc<Renderer>, tile_pos: (u8, u8), graph: &Arc<ADTNode>) {
        trace!("add_tile_graph: {}, {}", tile_pos.0, tile_pos.1);
        {
            // TODO: Currently one hardcoded material per adt
            let _material = Material {
                is_unlit: true,
                albedo: AlbedoType::Vertex { srgb: true },
                transparency: TransparencyType::Opaque,
            };
            let material = Rend3BackendConverter::create_material_from_ir(&_material, None);
            let material_handle = renderer.add_material(material);

            for (position, mesh) in &graph.terrain {
                let mesh_handle = Self::gpu_load_mesh(renderer, mesh);

                let object = rend3::types::Object {
                    mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
                    material: material_handle.clone(),
                    transform: coordinate_systems::adt_to_blender_transform(Vec3A::new(
                        position.x, position.y, position.z,
                    )),
                };

                // TODO: the object handle has to reside in the graph for auto freeing.
                let _object_handle = renderer.add_object(object);
                self.object_list.push(_object_handle);
            }
        }

        self.load_doodads(renderer, &graph.doodads, None);

        for wmo_ref in &graph.wmos {
            if let Some(wmo) = wmo_ref
                .reference
                .reference
                .read()
                .expect("WMO Read Lock")
                .as_ref()
            {
                self.load_doodads(renderer, &wmo.doodads, Some(wmo_ref.transform.into()));
                for material in &wmo.materials {
                    self.load_material(renderer, material, &wmo.tex_references);
                }

                for subgroup_ref in wmo.subgroups.iter() {
                    if let Some(subgroup) = subgroup_ref
                        .reference
                        .read()
                        .expect("Subgroup Read Lock")
                        .as_ref()
                    {
                        for (idx, batch) in subgroup.mesh_batches.iter().enumerate() {
                            let mat_id = subgroup.material_ids[idx];

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
                                self.missing_texture_material
                                    .as_ref()
                                    .expect("Missing Texture Material to be initialized")
                                    .clone()
                            };

                            let mesh_handle = Self::gpu_load_mesh(renderer, batch);
                            let object = rend3::types::Object {
                                mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
                                material: material_handle.clone(),
                                transform: wmo_ref.transform.into(),
                            };

                            // TODO: the object handle has to reside in the graph for auto freeing.
                            let _object_handle = renderer.add_object(object);
                            self.object_list.push(_object_handle);
                        }
                    }
                }
            } // else: not loaded yet?
        }
    }

    fn load_doodads(
        &mut self,
        renderer: &Arc<Renderer>,
        doodads: &Vec<Arc<DoodadReference>>,
        parent_transform: Option<Mat4>,
    ) {
        for doodad in doodads {
            // TODO: we need a better logic to express the desire to actually render something, because then we can explicitly load to the gpu

            if let Some(m2) = doodad
                .reference
                .reference
                .read()
                .expect("M2 Read Lock")
                .as_ref()
            {
                let mesh_handle = Self::gpu_load_mesh(renderer, &m2.mesh);

                let material_handle = self.load_material(renderer, &m2.material, &m2.tex_reference);

                let object = rend3::types::Object {
                    mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
                    material: material_handle.clone(),
                    transform: (parent_transform.unwrap_or(Mat4::IDENTITY) * doodad.transform),
                };

                // TODO: the object handle has to reside in the graph for auto freeing.
                let _object_handle = renderer.add_object(object);
                self.object_list.push(_object_handle);
            } else {
                log::warn!(
                    "Doodad couldn't be rendered because it wasn't resolved yet. Solve when going async. {}",
                    &doodad.reference.reference_str
                );
            }
        }
    }

    fn load_material(
        &mut self,
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
                .and_then(|tex_ref| Self::gpu_load_texture(renderer, &tex_ref.reference)),
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
            Self::gpu_load_material(renderer, material, texture_handle_opt)
        };
        material_handle
    }

    fn gpu_load_mesh(renderer: &Arc<Renderer>, mesh: &RwLock<IRMesh>) -> MeshHandle {
        {
            if let Some(handle) = mesh.read().expect("Mesh Read Lock").handle.as_ref() {
                return handle.clone();
            }
        }

        let mut mesh_lock = mesh.write().expect("Mesh Write Lock");
        let render_mesh =
            Rend3BackendConverter::create_mesh_from_ir(&mesh_lock.data).expect("Mesh building successful");
        let mesh_handle = renderer.add_mesh(render_mesh);
        mesh_lock.deref_mut().handle = Some(mesh_handle.clone());
        mesh_handle
    }

    fn gpu_load_material(
        renderer: &Arc<Renderer>,
        material: &RwLock<IRMaterial>,
        texture_handle: Option<Texture2DHandle>,
    ) -> MaterialHandle {
        {
            if let Some(handle) = material.read().expect("Material Read Lock").handle.as_ref() {
                return handle.clone();
            }
        }
        let mut material_lock = material.write().expect("Material Write Lock");
        let render_mat = Rend3BackendConverter::create_material_from_ir(&material_lock.data, texture_handle);
        let material_handle = renderer.add_material(render_mat);
        material_lock.deref_mut().handle = Some(material_handle.clone());
        material_handle
    }

    fn gpu_load_texture(
        renderer: &Arc<Renderer>,
        texture_reference: &RwLock<Option<Arc<RwLock<Option<IRTexture>>>>>,
    ) -> Option<Texture2DHandle> {
        {
            let tex_arc = texture_reference.read().expect("Texture Read Lock");
            if let Some(opt_handle) = tex_arc.as_ref() {
                {
                    let tex_lock = opt_handle.read().expect("Texture Read Lock 2");
                    if let Some(tex_handle) = tex_lock.as_ref() {
                        if let Some(handle) = tex_handle.handle.as_ref() {
                            return Some(handle.clone());
                        } // else: texture not added to the GPU yet - continue with the write lock
                    } else {
                        // texture loading error?
                        return None;
                    }
                }
            } else {
                // else: texture (reference?) not loaded yet.
                // TODO: the caller should prevent calling in that case and unwrap the lock? The caller should at least distinguish between texture not loaded (grey diffuse color) and texture loading error (pink!)
                return None;
            }
        }

        let tex_wlock = texture_reference.write().expect("Texture Write Lock");
        let mut tex_iwlock = tex_wlock
            .as_ref()
            .expect("unreachable!")
            .as_ref()
            .write()
            .expect("Texture internal write lock");

        let tex = tex_iwlock.as_mut().expect("unreachable!");
        let texture = Rend3BackendConverter::create_texture_from_ir(&tex.data, 0);
        let texture_handle = renderer.add_texture_2d(texture);
        tex.handle = Some(texture_handle.clone());
        Some(texture_handle)
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
        event_loop: &EventLoop<UserResizeEvent<()>>,
        window: &Window,
        renderer: &Arc<Renderer>,
        routines: &Arc<DefaultRoutines>,
        surface_format: TextureFormat,
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
                    self.camera_location += forward * 15.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 31u32) {
                    // S
                    self.camera_location -= forward * 10.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 30u32) {
                    // A
                    self.camera_location -= right * 10.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 32u32) {
                    // D
                    self.camera_location += right * 10.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 42u32) {
                    // LSHIFT
                    self.camera_location += up * 5.0 * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, 29u32) {
                    self.camera_location -= up * 5.0 * delta_time.as_secs_f32();
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
