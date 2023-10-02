use std::collections::HashMap;
use std::f32::consts::PI;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use glam::{Affine3A, DVec2, Mat4, Vec2, Vec3, Vec3A, Vec4};
use image_blp::BlpImage;
use image_blp::convert::blp_to_image;
use itertools::Itertools;
use rend3::Renderer;
use rend3::types::{Backend, MaterialHandle, MeshHandle, Object, ObjectHandle, Texture2DHandle};
use rend3::util::typedefs::FastHashMap;
use sargerust_files::common::types::{C3Vector, CImVector};

use crate::rendering::common::coordinate_systems;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod, TransparencyType, VertexBuffers};
use crate::rendering::rend3_backend::Rend3BackendConverter;

pub mod common;
pub mod importer;
pub mod rend3_backend;

fn create_texture_rgba8(blp: &BlpImage, mipmap_level: usize) -> rend3::types::Texture {
  let image = blp_to_image(blp, mipmap_level).expect("decode");
  let image_dims = glam::UVec2::new(image.width(), image.height());
  let image_data = image.into_rgba8();

  rend3::types::Texture {
    label: None,
    data: image_data.into_raw(),
    format: rend3::types::TextureFormat::Rgba8UnormSrgb,
    size: image_dims,
    mip_count: rend3::types::MipmapCount::ONE,
    mip_source: rend3::types::MipmapSource::Uploaded
  }
}

fn create_object(transform: Affine3A, mesh_handle: MeshHandle, material_handle: MaterialHandle) -> Object {
  rend3::types::Object {
    mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
    material: material_handle,
    transform: transform.into()
  }
}

#[derive(Default)]
struct App {
  pub scancode_status: FastHashMap<u32, bool>,
  pub camera_pitch: f32,
  pub camera_yaw: f32,
  pub camera_location: Vec3A,
  pub last_mouse_delta: Option<DVec2>,
}

// since the current impl doesn't care about RAM (see the mpq crate force-loading all mpqs), we can safely pass a load of (potentially unused) textures
pub fn render<'a, W>(placed_doodads: Vec<(Affine3A, Rc<(Mesh, Material, Option<BlpImage>)>)>, wmos: W,
                     textures: HashMap<String, BlpImage>,
                     terrain_chunk: Vec<(C3Vector, Vec<(Vec3, CImVector)>, Vec<u32>)>)
where
  W: IntoIterator<Item = (&'a Affine3A, &'a Vec<(MeshWithLod, Vec<Material> /* per lod */)>)>,
{
  // TODO: shouldn't we expect MeshWithLods already? Could also have a vec<mesh> at least. This conflicts with loading lods on demand, though.
  let mut app = App::default();

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
  let iad = pollster::block_on(rend3::create_iad(Some(Backend::Vulkan),
                                                 None, None, None)).unwrap();

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
  ).unwrap();

  // Create the shader preprocessor with all the default shaders added.
  let mut spp = rend3::ShaderPreProcessor::new();
  rend3_routine::builtin_shaders(&mut spp);

  // Create the base rendergraph.
  let base_rendergraph = rend3_routine::base::BaseRenderGraph::new(&renderer, &spp);

  let mut data_core = renderer.data_core.lock();
  let pbr_routine =
      rend3_routine::pbr::PbrRoutine::new(&renderer, &mut data_core, &spp, &base_rendergraph.interfaces);
  drop(data_core);
  let tonemapping_routine = rend3_routine::tonemapping::TonemappingRoutine::new(
    &renderer,
    &spp,
    &base_rendergraph.interfaces,
    preferred_format,
  );

  let mut object_list = Vec::new(); // we need to prevent object handles from getting dropped.

  add_placed_doodads(placed_doodads, &renderer, &mut object_list);
  add_wmo_groups(wmos, textures, &renderer, &mut object_list);

  let had_terrain = !terrain_chunk.is_empty();
  for chunk in terrain_chunk {
    let (position, verts, indices) = chunk;

    // TODO: Submethod
    let mesh_verts = verts.iter().map(|(v, _)| Vec3::new(v.x, v.y, v.z)).collect_vec();
    let mesh_col = verts.iter().map(|(_, col)| [col.r, col.g, col.b, col.a]).collect_vec();

    let _mesh = Mesh {
      vertex_buffers: VertexBuffers {
        position_buffer: mesh_verts,
        vertex_color_0: mesh_col,

        normals_buffer: vec![],
        tangents_buffer: vec![],
        texcoord_buffer_0: vec![],
        texcoord_buffer_1: vec![],
      },
      index_buffer: indices
    };

    let mut mesh = Rend3BackendConverter::create_mesh_from_ir(&_mesh).unwrap();
    mesh.flip_winding_order(); // it would be better if the mesh came pre-flipped, I guess (especially since the IR is cached).
    let mesh_handle = renderer.add_mesh(mesh);

    let _material = Material {
      is_unlit: true,
      albedo: AlbedoType::Vertex { srgb: true },
      transparency: TransparencyType::Opaque
    };
    let material = Rend3BackendConverter::create_material_from_ir(&_material, None);
    let material_handle = renderer.add_material(material);

    // Don't ask me where flipping the z and the heightmap values comes from.
    // Actually, I think I flipped everything there is now, for consistency with ADT and where it should belong (i.e. 16k, 16k; not negative area)
    let tt = coordinate_systems::adt_to_blender_transform(Vec3A::new(position.x, position.y, position.z));
    let object = Object {
      mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
      material: material_handle,
      // I think mat * translation rotates our translation and as such is basically always wrong. It can't have ever rotated things as a side effect?
      //transform: tt * Mat4::from_euler(glam::EulerRot::XYZ, 0.0 * PI, 1.0 * PI, 0.75 * PI)
      //transform: Mat4::from_euler(glam::EulerRot::XYZ, 0.0 * PI, 1.0 * PI, 0.75 * PI) * tt
      transform: tt
    };

    let _object_handle = renderer.add_object(object);
    object_list.push(_object_handle);
  }

  if !had_terrain {
    app.camera_location = Vec3A::new(0.0, -4.0, 2.0);
  } else {
    // For the GM Island, we move to it's world location for convenience
    app.camera_location = coordinate_systems::adt_to_blender(Vec3A::new(16000.0, 16000.0, 42.0));
  }

  let quat = glam::Quat::from_euler(glam::EulerRot::XYZ, 0.5 * PI, 0.0 * PI, 0.0 * PI);
  let view = Mat4::from_rotation_translation(quat, (-app.camera_location).into());

  // Set camera's location
  renderer.set_camera_data(rend3::types::Camera {
    projection: rend3::types::CameraProjection::Perspective { vfov: 90.0, near: 0.1 },
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
      let rotation = glam::Mat3A::from_euler(glam::EulerRot::XYZ, -app.camera_pitch * PI, 0.0 /* roll */ * PI,  -app.camera_yaw * PI);
      let forward: Vec3A = rotation.y_axis;
      let right: Vec3A = rotation.x_axis;
      let up: Vec3A = rotation.z_axis;

      if *app.scancode_status.get(&17u32).unwrap_or(&false)  {
        // W
        app.camera_location += forward * 30.0/60.0 * 5.0; // fake delta time
      }
      if *app.scancode_status.get(&31u32).unwrap_or(&false)  {
        // S
        app.camera_location -= forward * 30.0/60.0; // fake delta time
      }
      if *app.scancode_status.get(&30u32).unwrap_or(&false)  {
        // A
        app.camera_location -= right * 20.0/60.0;
      }
      if *app.scancode_status.get(&32u32).unwrap_or(&false)  {
        // D
        app.camera_location += right * 20.0/60.0;
      }
      if *app.scancode_status.get(&42u32).unwrap_or(&false) {
        // LSHIFT
        app.camera_location += up * 20.0/60.0;
      }
      if *app.scancode_status.get(&29u32).unwrap_or(&false) {
        app.camera_location -= up * 20.0/60.0;
      }
      if *app.scancode_status.get(&57421u32).unwrap_or(&false) {
        // arrow right
        app.camera_yaw += 1.0/60.0;
      }
      if *app.scancode_status.get(&57419u32).unwrap_or(&false) {
        app.camera_yaw -= 1.0/60.0;
      }

      // TODO: the following is under redraw requested in https://github.com/BVE-Reborn/rend3/blob/trunk/examples/scene-viewer/src/lib.rs#L572
      // technically, we could also invert the view rotation (remember this is not the cams matrix, but the _view_ matrix, so how do you transform
      // the world to get to the screen (i.e. 0, 0). Hence we also need to invert the camera_location. Inverting the rotation isn't a deal though,
      // as we can just control the input angles.
      let view = Mat4::from_euler(glam::EulerRot::XYZ, (-0.5 - app.camera_pitch) * PI, 0.0 /* roll */ * PI,  app.camera_yaw * PI);
      let view = view * Mat4::from_translation((-app.camera_location).into());

      // Set camera's location
      renderer.set_camera_data(rend3::types::Camera {
        projection: rend3::types::CameraProjection::Perspective { vfov: 90.0, near: 0.1 },
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
        rend3::types::SampleCount::One,
        glam::Vec4::ZERO,
        glam::Vec4::new(0.10, 0.05, 0.10, 1.0), // Nice scene-referred purple
      );

      // Dispatch a render using the built up rendergraph!
      graph.execute(&renderer, &mut eval_output);

      // Present the frame
      frame.present();
    }
    winit::event::Event::WindowEvent {
      event: winit::event::WindowEvent::KeyboardInput {
        input: winit::event::KeyboardInput { scancode, state, ..},
        ..
      },
      ..
    } => {
      if scancode != 17 && (scancode < 30 || scancode > 32) && scancode != 29 && scancode != 42 && scancode != 57419 && scancode != 57421 {
        dbg!(scancode);
      }

      app.scancode_status.insert(scancode, match state {
        winit::event::ElementState::Pressed => true,
        winit::event::ElementState::Released => false,
      });
    }
    // Other events we don't care about
    _ => {}
  });
}

fn add_wmo_groups<'a, W>(wmos: W, textures: HashMap<String, BlpImage>, renderer: &Arc<Renderer>, object_list: &mut Vec<ObjectHandle>)
  where W: IntoIterator<Item=(&'a Affine3A, &'a Vec<(MeshWithLod, Vec<Material> /* per lod */)>)> {
  for (transform, wmo_groups) in wmos {
    for (lod_mesh, materials) in wmo_groups {
      // One "lod" has it's own material here, but technically it's a wmo group batch.
      for (i, material) in materials.iter().enumerate() {
        let mesh = Rend3BackendConverter::create_mesh_from_ir_lod(lod_mesh, i).unwrap();
        let mesh_handle = renderer.add_mesh(mesh);

        // TODO: concept work for textures
        let blp_opt = match &material.albedo {
          AlbedoType::TextureWithName(tex_name) => textures.get(tex_name),
          _ => None
        };

        let mapped_tex = blp_opt.as_ref().map(|tex| renderer.add_texture_2d(create_texture_rgba8(tex, 0)));
        let material = Rend3BackendConverter::create_material_from_ir(material, mapped_tex);
        let material_handle = renderer.add_material(material);

        // Combine the mesh and the material with a location to give an object.
        let object = create_object(*transform, mesh_handle, material_handle);
        let _object_handle = renderer.add_object(object);
        object_list.push(_object_handle);
      }
    }
  }
}

fn add_placed_doodads(placed_doodads: Vec<(Affine3A, Rc<(Mesh, Material, Option<BlpImage>)>)>, renderer: &Arc<Renderer>, object_list: &mut Vec<ObjectHandle>) {
  for (transform, rc) in placed_doodads {
    let (_mesh, _material, blp_opt) = rc.deref();
    // Create mesh and calculate smooth normals based on vertices
    let mesh = Rend3BackendConverter::create_mesh_from_ir(_mesh).unwrap();
    let mesh_handle = renderer.add_mesh(mesh);

    // TODO: concept work for textures
    let mapped_tex = blp_opt.as_ref().map(|tex| renderer.add_texture_2d(create_texture_rgba8(tex, 0)));
    let material = Rend3BackendConverter::create_material_from_ir(&_material, mapped_tex);
    let material_handle = renderer.add_material(material);

    // Combine the mesh and the material with a location to give an object.
    let object = create_object(transform, mesh_handle, material_handle);

    // Creating an object will hold onto both the mesh and the material
    // even if they are deleted.
    //
    // We need to keep the object handle alive.
    let _object_handle = renderer.add_object(object);
    object_list.push(_object_handle);
  }
}
