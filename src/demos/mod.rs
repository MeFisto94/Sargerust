use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering;
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::highlevel_types::PlacedDoodad;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod};
use crate::rendering::importer::adt_importer::ADTImporter;
use crate::rendering::importer::m2_importer::M2Importer;
use crate::rendering::loader::blp_loader::BLPLoader;
use crate::rendering::loader::m2_loader::{LoadedM2, M2Loader};
use crate::rendering::loader::wmo_loader::WMOLoader;
use glam::{Affine3A, DVec2, Mat4, Vec3, Vec3A};
use image_blp::BlpImage;
use itertools::Itertools;
use log::{trace, warn};
use rend3::types::SampleCount;
use rend3::util::typedefs::FastHashMap;
use rend3_routine::base::{BaseRenderGraphRoutines, OutputRenderTarget};
use sargerust_files::adt::reader::ADTReader;
use sargerust_files::adt::types::ADTAsset;
use sargerust_files::m2::reader::M2Reader;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Instant;

#[derive(Default)]
struct DemoApplication {
    pub scancode_status: FastHashMap<u32, bool>,
    pub camera_pitch: f32,
    pub camera_yaw: f32,
    pub camera_location: Vec3A,
    pub last_mouse_delta: Option<DVec2>,
}

// since this is a demo, we can safely pass a load of (potentially unused) textures
pub fn render<'a, W>(
    placed_doodads: Vec<PlacedDoodad>,
    wmos: W,
    textures: HashMap<String, BlpImage>,
    terrain_chunk: Vec<(Vec3, Mesh)>,
    camera_location: Vec3A,
) where
    W: IntoIterator<
        Item = (
            &'a Affine3A,
            &'a Vec<(MeshWithLod, Vec<Material> /* per lod */)>,
        ),
    >,
{
    // TODO: shouldn't we expect MeshWithLods already? Could also have a vec<mesh> at least. This conflicts with loading lods on demand, though.
    let mut app = DemoApplication::default();

    // Create event loop and window
    let event_loop = winit::event_loop::EventLoop::new();
    let window = {
        let mut builder = winit::window::WindowBuilder::new();
        builder = builder.with_title("Sargerust: Wrath of the Rust King");
        builder.build(&event_loop).expect("Could not build window")
    };

    let window_size = window.inner_size();

    // Create the Instance, Adapter, and Device. We can specify preferred backend,
    // device name, or rendering profile. In this case we let rend3 choose for us.
    let iad = pollster::block_on(rend3::create_iad(None, None, None, None)).unwrap();

    // The one line of unsafe needed. We just need to guarentee that the window
    // outlives the use of the surface.
    //
    // SAFETY: this surface _must_ not be used after the `window` dies. Both the
    // event loop and the renderer are owned by the `run` closure passed to winit,
    // so rendering work will stop after the window dies.
    let surface = Arc::new(unsafe { iad.instance.create_surface(&window) }.unwrap());
    // Get the preferred format for the surface.
    let caps = surface.get_capabilities(&iad.adapter);
    let preferred_format = caps.formats[0];

    // Configure the surface to be ready for rendering.
    rend3::configure_surface(
        &surface,
        &iad.device,
        preferred_format,
        glam::UVec2::new(window_size.width, window_size.height),
        rend3::types::PresentMode::Fifo,
    );

    // Make us a renderer.
    let renderer = rend3::Renderer::new(
        iad,
        rend3::types::Handedness::Right,
        Some(window_size.width as f32 / window_size.height as f32),
    )
    .unwrap();

    // Create the shader preprocessor with all the default shaders added.
    let mut spp = rend3::ShaderPreProcessor::new();
    rend3_routine::builtin_shaders(&mut spp);

    // Create the base rendergraph.
    let base_rendergraph = rend3_routine::base::BaseRenderGraph::new(&renderer, &spp);

    let mut data_core = renderer.data_core.lock();
    let pbr_routine = rend3_routine::pbr::PbrRoutine::new(
        &renderer,
        &mut data_core,
        &spp,
        &base_rendergraph.interfaces,
        &base_rendergraph.gpu_culler.culling_buffer_map_handle,
    );
    drop(data_core);
    let tonemapping_routine = rend3_routine::tonemapping::TonemappingRoutine::new(
        &renderer,
        &spp,
        &base_rendergraph.interfaces,
        preferred_format,
    );

    let mut object_list = Vec::new(); // we need to prevent object handles from getting dropped.

    rendering::add_placed_doodads(&placed_doodads, &renderer, &mut object_list);
    rendering::add_wmo_groups(wmos, &textures, &renderer, &mut object_list);
    rendering::add_terrain_chunks(&terrain_chunk, &renderer, &mut object_list);

    app.camera_location = camera_location;

    let quat = glam::Quat::from_euler(glam::EulerRot::XYZ, 0.5 * PI, 0.0 * PI, 0.0 * PI);
    let view = Mat4::from_rotation_translation(quat, (-app.camera_location).into());

    // Set camera's location
    renderer.set_camera_data(rend3::types::Camera {
        projection: rend3::types::CameraProjection::Perspective {
            vfov: 90.0,
            near: 0.1,
        },
        view,
    });

    // Create a single directional light
    //
    // We need to keep the directional light handle alive.
    let _directional_handle = renderer.add_directional_light(rend3::types::DirectionalLight {
        color: glam::Vec3::ONE,
        intensity: 10.0,
        // Direction will be normalized
        direction: Vec3::new(0.0, 0.0, -1.0),
        distance: 400.0,
        resolution: 2048,
    });

    let mut resolution = glam::UVec2::new(window_size.width, window_size.height);

    event_loop.run(move |event, _, control| match event {
        // Close button was clicked, we should close.
        winit::event::Event::WindowEvent {
            event: winit::event::WindowEvent::CloseRequested,
            ..
        } => {
            *control = winit::event_loop::ControlFlow::Exit;
        }
        // Window was resized, need to resize renderer.
        winit::event::Event::WindowEvent {
            event: winit::event::WindowEvent::Resized(physical_size),
            ..
        } => {
            resolution = glam::UVec2::new(physical_size.width, physical_size.height);
            // Reconfigure the surface for the new size.
            rend3::configure_surface(
                &surface,
                &renderer.device,
                preferred_format,
                glam::UVec2::new(resolution.x, resolution.y),
                rend3::types::PresentMode::Fifo,
            );
            // Tell the renderer about the new aspect ratio.
            renderer.set_aspect_ratio(resolution.x as f32 / resolution.y as f32);
        }
        // Render!
        winit::event::Event::MainEventsCleared => {
            // Don't ask me why the rotation is so different here, _but_ the camera by default looks down,
            // so it is rotated differently than the carthesian that moves it.
            let rotation = glam::Mat3A::from_euler(
                glam::EulerRot::XYZ,
                -app.camera_pitch * PI,
                0.0 /* roll */ * PI,
                -app.camera_yaw * PI,
            );
            let forward: Vec3A = rotation.y_axis;
            let right: Vec3A = rotation.x_axis;
            let up: Vec3A = rotation.z_axis;

            if *app.scancode_status.get(&17u32).unwrap_or(&false) {
                // W
                app.camera_location += forward * 30.0 / 60.0 * 5.0; // fake delta time
            }
            if *app.scancode_status.get(&31u32).unwrap_or(&false) {
                // S
                app.camera_location -= forward * 30.0 / 60.0; // fake delta time
            }
            if *app.scancode_status.get(&30u32).unwrap_or(&false) {
                // A
                app.camera_location -= right * 20.0 / 60.0;
            }
            if *app.scancode_status.get(&32u32).unwrap_or(&false) {
                // D
                app.camera_location += right * 20.0 / 60.0;
            }
            if *app.scancode_status.get(&42u32).unwrap_or(&false) {
                // LSHIFT
                app.camera_location += up * 20.0 / 60.0;
            }
            if *app.scancode_status.get(&29u32).unwrap_or(&false) {
                app.camera_location -= up * 20.0 / 60.0;
            }
            if *app.scancode_status.get(&57421u32).unwrap_or(&false) {
                // arrow right
                app.camera_yaw += 1.0 / 60.0;
            }
            if *app.scancode_status.get(&57419u32).unwrap_or(&false) {
                app.camera_yaw -= 1.0 / 60.0;
            }

            // TODO: the following is under redraw requested in https://github.com/BVE-Reborn/rend3/blob/trunk/examples/scene-viewer/src/lib.rs#L572
            // technically, we could also invert the view rotation (remember this is not the cams matrix, but the _view_ matrix, so how do you transform
            // the world to get to the screen (i.e. 0, 0). Hence we also need to invert the camera_location. Inverting the rotation isn't a deal though,
            // as we can just control the input angles.
            let view = Mat4::from_euler(
                glam::EulerRot::XYZ,
                (-0.5 - app.camera_pitch) * PI,
                0.0 /* roll */ * PI,
                app.camera_yaw * PI,
            );
            let view = view * Mat4::from_translation((-app.camera_location).into());

            // Set camera's location
            renderer.set_camera_data(rend3::types::Camera {
                projection: rend3::types::CameraProjection::Perspective {
                    vfov: 90.0,
                    near: 0.1,
                },
                view,
            });

            // Get a frame
            let frame = surface.get_current_texture().unwrap();

            // Swap the instruction buffers so that our frame's changes can be processed.
            renderer.swap_instruction_buffers();
            // Evaluate our frame's world-change instructions
            let mut eval_output = renderer.evaluate_instructions();

            // Build a rendergraph
            let mut graph = rend3::graph::RenderGraph::new();

            // Import the surface texture into the render graph.
            let frame_handle = graph.add_imported_render_target(
                &frame,
                0..1,
                0..1,
                rend3::graph::ViewportRect::from_size(resolution),
            );
            // Add the default rendergraph without a skybox
            base_rendergraph.add_to_graph(
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
                        resolution,
                        samples: SampleCount::One,
                    },
                },
                rend3_routine::base::BaseRenderGraphSettings {
                    ambient_color: glam::Vec4::ZERO,
                    clear_color: glam::Vec4::new(0.10, 0.05, 0.10, 1.0), // Nice scene-referred purple
                },
            );

            // Dispatch a render using the built up rendergraph!
            graph.execute(&renderer, &mut eval_output);

            // Present the frame
            frame.present();
        }
        winit::event::Event::WindowEvent {
            event:
                winit::event::WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            scancode, state, ..
                        },
                    ..
                },
            ..
        } => {
            if scancode != 17
                && (scancode < 30 || scancode > 32)
                && scancode != 29
                && scancode != 42
                && scancode != 57419
                && scancode != 57421
            {
                dbg!(scancode);
            }

            app.scancode_status.insert(
                scancode,
                match state {
                    winit::event::ElementState::Pressed => true,
                    winit::event::ElementState::Released => false,
                },
            );
        }
        // Other events we don't care about
        _ => {}
    });
}

pub fn main_simple_m2(loader: &MPQLoader) -> Result<(), anyhow::Error> {
    // This method demonstrates very simple m2 rendering (not in the context of wmos or adts).
    // It typically makes more sense to use load_m2_doodad, however this a) shows the process involved
    // and b) overrides the tex_path (Talbuks, but Creatures in general) have color variations

    let m2_path = r"Creature\talbuk\Talbuk.m2";
    let skin_path = r"Creature\talbuk\Talbuk00.skin";
    let tex_path = r"Creature\talbuk\TalbukSkinBrown.blp";

    let m2 = M2Reader::parse_asset(&mut std::io::Cursor::new(
        loader.load_raw_owned(m2_path).unwrap(),
    ))?;
    let skin = M2Reader::parse_skin_profile(&mut std::io::Cursor::new(
        loader.load_raw_owned(skin_path).unwrap(),
    ))?;
    let blp_opt = BLPLoader::load_blp_from_ldr(loader, tex_path);
    let imported_mesh = M2Importer::create_mesh(&m2, &skin);
    let mat = M2Importer::create_material(&blp_opt);

    let dad = PlacedDoodad {
        transform: Affine3A::IDENTITY,
        m2: Arc::new(LoadedM2 {
            mesh: imported_mesh,
            material: mat,
            blp_opt,
        }),
    };

    // Note: This API is already a bad monstrosity, it WILL go, but it makes prototyping easier.
    render(
        vec![dad],
        vec![],
        HashMap::new(),
        vec![],
        Vec3A::new(0.0, -4.0, 2.0),
    );
    Ok(())
}

pub fn main_simple_wmo(loader: &MPQLoader) -> Result<(), anyhow::Error> {
    // This method demonstrates very simple wmo rendering (not in the context of adts).
    // let wmo_path = r"World\wmo\Dungeon\AZ_Subway\Subway.wmo";
    // let wmo_path = r"World\wmo\Azeroth\Buildings\GoldshireInn\GoldshireInn.wmo"; // good example of how we need to filter doodad sets
    // let wmo_path = r"World\wmo\Azeroth\Buildings\GriffonAviary\GriffonAviary.WMO"; // <-- orange color, no textures?
    let wmo_path = r"World\wmo\Azeroth\Buildings\GoldshireInn\GoldshireInn_closed.WMO";
    let loaded = WMOLoader::load(loader, wmo_path)?;

    // TODO: currently, only WMO makes use of texture names, M2s load their textures in load_m2_doodad (when the doodad becomes placeable).
    let textures = loaded
        .loaded_groups
        .iter()
        .flat_map(|(_, mats)| mats)
        .filter_map(|mat| match &mat.albedo {
            AlbedoType::TextureWithName(tex_name) => Some(tex_name.clone()),
            _ => None,
        })
        .collect_vec();

    let mut texture_map = HashMap::new();
    for texture in textures {
        if !texture_map.contains_key(&texture) {
            let blp = BLPLoader::load_blp_from_ldr(loader, &texture).expect("Texture loading error");
            texture_map.insert(texture, blp);
        }
    }

    let mut m2_cache = HashMap::new();
    let dooads = loaded
        .doodads
        .iter()
        // Resolve references
        .map(|dad| PlacedDoodad {
            transform: dad.transform,
            m2: load_m2_doodad(loader, &mut m2_cache, &dad.m2_ref),
        })
        .collect_vec();

    let group_list = loaded.loaded_groups;
    let wmos = vec![(Affine3A::IDENTITY, group_list)];

    // Note: This API is already a bad monstrosity, it WILL go, but it makes prototyping easier.
    render(
        dooads,
        wmos.iter().map(|wmo| (&wmo.0, &wmo.1)),
        texture_map,
        vec![],
        Vec3A::new(0.0, -4.0, 2.0),
    );
    Ok(())
}

pub fn main_simple_adt(loader: &MPQLoader) -> Result<(), anyhow::Error> {
    let adt = ADTReader::parse_asset(&mut std::io::Cursor::new(
        loader
            .load_raw_owned(r"World\Maps\Kalimdor\Kalimdor_1_1.adt")
            .unwrap(),
    ))?;

    let mut m2_cache = HashMap::new();
    let mut render_list = Vec::new();
    let mut texture_map = HashMap::new();
    let mut wmos = Vec::new();

    let terrain_chunk = handle_adt(
        loader,
        &adt,
        &mut m2_cache,
        &mut render_list,
        &mut texture_map,
        &mut wmos,
    )?;
    render(
        render_list,
        wmos.iter().map(|wmo| (&wmo.0, &wmo.1)),
        texture_map,
        terrain_chunk,
        coordinate_systems::adt_to_blender(Vec3A::new(16000.0, 16000.0, 42.0)),
    );
    Ok(())
}

pub fn main_multiple_adt(loader: &MPQLoader) -> Result<(), anyhow::Error> {
    let now = Instant::now();
    // technically, wdt loading doesn't differ all too much, because if it has terrain, it doesn't have it's own dooads
    // and then all you have to check is for existing adt files (MAIN chunk)
    let map_name = r"World\Maps\Kalimdor\Kalimdor";
    let mut m2_cache = HashMap::new();
    let mut render_list = Vec::new();
    let mut texture_map = HashMap::new();
    let mut wmos = Vec::new();
    let mut terrain_chunks: Vec<(Vec3, Mesh)> = Vec::new();

    for row in 0..2 {
        for column in 0..2 {
            let adt = ADTReader::parse_asset(&mut std::io::Cursor::new(
                loader
                    .load_raw_owned(&format!("{}_{}_{}.adt", map_name, row, column))
                    .unwrap(),
            ))?;
            terrain_chunks.extend(handle_adt(
                loader,
                &adt,
                &mut m2_cache,
                &mut render_list,
                &mut texture_map,
                &mut wmos,
            )?);
        }
    }

    warn!("Loading took {}ms", now.elapsed().as_millis());
    render(
        render_list,
        wmos.iter().map(|wmo| (&wmo.0, &wmo.1)),
        texture_map,
        terrain_chunks,
        coordinate_systems::adt_to_blender(Vec3A::new(16000.0, 16000.0, 42.0)),
    );
    Ok(())
}

fn handle_adt(
    loader: &MPQLoader,
    adt: &ADTAsset,
    m2_cache: &mut HashMap<String, Arc<LoadedM2>>,
    render_list: &mut Vec<PlacedDoodad>,
    texture_map: &mut HashMap<String, BlpImage>,
    wmos: &mut Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>,
) -> Result<Vec<(Vec3, Mesh)>, anyhow::Error> {
    for wmo_ref in adt.modf.mapObjDefs.iter() {
        let name = &adt.mwmo.filenames[*adt
            .mwmo
            .offsets
            .get(&adt.mwid.mwmo_offsets[wmo_ref.nameId as usize])
            .unwrap()];
        trace!("WMO {} has been referenced from ADT", name);

        if name.ends_with("STORMWIND.WMO") {
            continue; // TODO: Temporary performance optimization
        }

        let loaded = WMOLoader::load(loader, name)?;
        // TODO: currently, only WMO makes use of texture names, M2s load their textures in load_m2_doodad (when the doodad becomes placeable).
        let textures = loaded
            .loaded_groups
            .iter()
            .flat_map(|(_, mats)| mats)
            .filter_map(|mat| match &mat.albedo {
                AlbedoType::TextureWithName(tex_name) => Some(tex_name.clone()),
                _ => None,
            })
            .collect_vec();

        for texture in textures {
            if !texture_map.contains_key(&texture) {
                let blp = BLPLoader::load_blp_from_ldr(loader, &texture).expect("Texture loading error");
                texture_map.insert(texture, blp);
            }
        }

        let transform = crate::transform_for_wmo_ref(wmo_ref);
        for dad in loaded.doodads {
            // NOTE: Here we loose the relationship between DAD and wmo, that is required for parenting.
            // Since rend3 does not have a scenegraph, we "fake" the parenting for now.
            // Also we need to resolve m2 references.
            render_list.push(PlacedDoodad {
                transform: transform * dad.transform,
                m2: load_m2_doodad(loader, m2_cache, &dad.m2_ref),
            });
        }

        wmos.push((transform, loaded.loaded_groups));
    }

    // TODO: deduplicate with collect doodads (at least the emitter and m2 name replacement)
    for dad_ref in &adt.mddf.doodadDefs {
        let name = &adt.mmdx.filenames[*adt
            .mmdx
            .offsets
            .get(&adt.mmid.mmdx_offsets[dad_ref.nameId as usize])
            .unwrap()];
        trace!("M2 {} has been referenced from ADT", name);

        // fix name: currently it ends with .mdx but we need .m2
        let name = name
            .to_lowercase()
            .replace(".mdx", ".m2")
            .replace(".mdl", ".m2");
        if name.to_lowercase().contains("emitter") {
            continue;
        }

        let entry = load_m2_doodad(loader, m2_cache, &name);
        render_list.push(PlacedDoodad {
            transform: crate::transform_for_doodad_ref(dad_ref),
            m2: entry,
        });
    }

    let mut terrain_chunk = vec![];
    for mcnk in &adt.mcnks {
        terrain_chunk.push(ADTImporter::create_mesh(mcnk, false)?);
    }

    Ok(terrain_chunk)
}

#[allow(deprecated)] // only for the demo, we load m2s in non-graph style, which is easier for us
fn load_m2_doodad(loader: &MPQLoader, m2_cache: &mut HashMap<String, Arc<LoadedM2>>, name: &str) -> Arc<LoadedM2> {
    // Caching M2s is unavoidable in some way, especially when loading multiple chunks in parallel.
    // Otherwise, m2s could be loaded multiple times, but the important thing is to deduplicate
    // them before sending them to the render thread. Share meshes and textures!
    let entry = m2_cache
        .entry(name.to_string())
        .or_insert_with(|| Arc::new(M2Loader::load_no_lod(loader, name)));
    entry.clone()
}
