use std::collections::HashMap;
use std::f32::consts::PI;
use std::fmt::{Debug, Formatter};
use std::hash::BuildHasher;
use std::sync::{Arc, Weak};
use std::sync::atomic::Ordering;
use std::time::Instant;
use glam::{Affine3A, Mat4, UVec2, Vec3, Vec3A};
use image_blp::BlpImage;
use itertools::Itertools;
use log::trace;
use rend3::Renderer;
use rend3::types::{Camera, CameraProjection, Handedness, ObjectHandle, PresentMode, SampleCount, Surface, TextureFormat};
use rend3::util::typedefs::FastHashMap;
use rend3_framework::{DefaultRoutines, Event, Grabber, UserResizeEvent};
use rend3_routine::base::BaseRenderGraph;
use winit::event::{ElementState, KeyboardInput, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;
use sargerust_files::adt::types::ADTAsset;
use crate::game::application::GameApplication;
use crate::rendering::{add_placed_doodads, add_terrain_chunks, add_wmo_groups};
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::highlevel_types::PlacedDoodad;
use crate::rendering::common::types::{Material, Mesh, MeshWithLod};

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
    loaded_tiles: HashMap<(u8, u8), (Arc<ADTAsset>, /* Terrain */ Vec<(Vec3, Mesh)>, Vec<PlacedDoodad>, /* WMO */ Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>, HashMap<String, BlpImage>)>,
    object_list: Vec<ObjectHandle>, // TODO: Refactor to the DAG
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
            object_list: vec![]
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    fn run_updates(&mut self, renderer: &Arc<Renderer>) {
        let app = self.app();
        let mm_lock =  app.game_state.clone().map_manager.clone();
        let mm = mm_lock.read().expect("Read Lock on Map Manager");

        if mm.current_map.is_some() != self.current_map.is_some() /* initial load or unload */ ||
            (mm.current_map.is_some() && &mm.current_map.as_ref().unwrap().0 != self.current_map.as_ref().unwrap()) {
            trace!("Map has changed, discarding everything");
            self.loaded_tiles.clear();
            self.current_map = Some(mm.current_map.as_ref().unwrap().0.clone());

            // TODO: This needs to be more sophisticated, in general it sucks that we just can't call from the packet handler into RenderApplication
            self.camera_location = coordinate_systems::adt_to_blender(*app.game_state.player_location.read().expect("Read Lock on Player Location"));
            self.camera_yaw = *app.game_state.player_orientation.read().expect("Read Lock on Player Orientation") - PI * 0.5;
        }

        let added_tiles = mm.loaded_tiles.iter().filter(|ki| !self.loaded_tiles.contains_key(ki.0)).collect_vec();
        let removed_tiles = self.loaded_tiles.keys().filter(|ki| !mm.loaded_tiles.contains_key(ki))
            .copied().collect_vec();

        for tile in removed_tiles {
            self.loaded_tiles.remove(&tile);
        }

        for (key, value) in added_tiles {
            let val = value.clone();
            self.add_tile(renderer, *key, &val);
            self.loaded_tiles.insert(*key, val);
        }
    }

    fn add_tile(&mut self, renderer: &Arc<Renderer>, tile_pos: (u8, u8), tile: &(Arc<ADTAsset>, Vec<(Vec3, Mesh)>, Vec<PlacedDoodad>, Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>, HashMap<String, BlpImage>)) {
        let (adt, terrain_chunk, placed_doodads, wmos, textures) = tile;
        add_placed_doodads(placed_doodads, renderer, &mut self.object_list);
        add_wmo_groups(wmos.iter().map(|w| (&w.0, &w.1)), textures, renderer, &mut self.object_list);
        add_terrain_chunks(terrain_chunk, renderer, &mut self.object_list);
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


    fn setup(&mut self, event_loop: &EventLoop<UserResizeEvent<()>>, window: &Window, renderer: &Arc<Renderer>, routines: &Arc<DefaultRoutines>, surface_format: TextureFormat) {
        // Push the Renderer into the GameApplication to preload handles.
        if self.app.upgrade().expect("Application to be initialized")
            .renderer.set(renderer.clone()).is_err() {
            panic!("Setting the renderer on Application failed: already initialized");
        }

        self.grabber = Some(Grabber::new(window));
    }

    fn handle_event(&mut self, window: &Window, renderer: &Arc<Renderer>, routines: &Arc<DefaultRoutines>, base_rendergraph: &BaseRenderGraph, surface: Option<&Arc<Surface>>, resolution: UVec2, event: Event<'_, ()>, control_flow: impl FnOnce(ControlFlow)) {
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

                let rotation = glam::Mat3A::from_euler(glam::EulerRot::XYZ, -self.camera_pitch * PI, 0.0 /* roll */ * PI,  -self.camera_yaw * PI);
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
                let view = Mat4::from_euler(glam::EulerRot::XYZ, (-0.5 - self.camera_pitch) * PI, 0.0 /* roll */ * PI,  self.camera_yaw * PI);
                let view = view * Mat4::from_translation((-self.camera_location).into());

                renderer.set_camera_data(Camera {
                    projection: CameraProjection::Perspective { vfov: 90.0, near: 0.1 },
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
                let frame_handle =
                    graph.add_imported_render_target(&frame, 0..1, rend3::graph::ViewportRect::from_size(resolution));
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
                    input: KeyboardInput { scancode, state, .. },
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