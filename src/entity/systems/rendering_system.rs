use crate::entity::components::objects::{TmpLocation, TmpOrientation};
use crate::entity::components::render::Renderable;
use crate::game::application::GameApplication;
use crate::rendering::application::RenderingApplication;
use crate::rendering::common::coordinate_systems::{adt_to_blender_rot, adt_to_blender_unaligned};
use glam::{Mat4, Quat, Vec4};
use rend3::Renderer;
use rend3::types::{MaterialHandle, MeshHandle, Object, ObjectMeshKind};
use rend3_routine::pbr::{AlbedoComponent, PbrMaterial};
use std::sync::{Arc, OnceLock};

// cube_example from rend3.
fn vertex(pos: [f32; 3]) -> glam::Vec3 {
    glam::Vec3::from(pos)
}

fn create_debug_mesh() -> rend3::types::Mesh {
    let vertex_positions = [
        // far side (0.0, 0.0, 1.0)
        vertex([-1.0, -1.0, 1.0]),
        vertex([1.0, -1.0, 1.0]),
        vertex([1.0, 1.0, 1.0]),
        vertex([-1.0, 1.0, 1.0]),
        // near side (0.0, 0.0, -1.0)
        vertex([-1.0, 1.0, -1.0]),
        vertex([1.0, 1.0, -1.0]),
        vertex([1.0, -1.0, -1.0]),
        vertex([-1.0, -1.0, -1.0]),
        // right side (1.0, 0.0, 0.0)
        vertex([1.0, -1.0, -1.0]),
        vertex([1.0, 1.0, -1.0]),
        vertex([1.0, 1.0, 1.0]),
        vertex([1.0, -1.0, 1.0]),
        // left side (-1.0, 0.0, 0.0)
        vertex([-1.0, -1.0, 1.0]),
        vertex([-1.0, 1.0, 1.0]),
        vertex([-1.0, 1.0, -1.0]),
        vertex([-1.0, -1.0, -1.0]),
        // top (0.0, 1.0, 0.0)
        vertex([1.0, 1.0, -1.0]),
        vertex([-1.0, 1.0, -1.0]),
        vertex([-1.0, 1.0, 1.0]),
        vertex([1.0, 1.0, 1.0]),
        // bottom (0.0, -1.0, 0.0)
        vertex([1.0, -1.0, 1.0]),
        vertex([-1.0, -1.0, 1.0]),
        vertex([-1.0, -1.0, -1.0]),
        vertex([1.0, -1.0, -1.0]),
    ];

    let index_data: &[u32] = &[
        0, 1, 2, 2, 3, 0, // far
        4, 5, 6, 6, 7, 4, // near
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // top
        20, 21, 22, 22, 23, 20, // bottom
    ];

    rend3::types::MeshBuilder::new(vertex_positions.to_vec(), rend3::types::Handedness::Left)
        .with_indices(index_data.to_vec())
        .build()
        .unwrap()
}

pub struct RenderingSystem {
    debug_object: OnceLock<(MeshHandle, MaterialHandle)>,
}

impl RenderingSystem {
    pub fn new() -> Self {
        Self {
            debug_object: OnceLock::new(),
        }
    }

    fn debug_object(&self, renderer: &Arc<Renderer>) -> &(MeshHandle, MaterialHandle) {
        self.debug_object.get_or_init(|| {
            let mat = PbrMaterial {
                unlit: true,
                albedo: AlbedoComponent::Value(Vec4::new(1.0, 0.0, 0.0, 1.0)),
                ..Default::default()
            };
            let mesh = create_debug_mesh();

            (
                renderer.add_mesh(mesh).expect("Mesh Creation"),
                renderer.add_material(mat),
            )
        })
    }

    pub fn update(&self, app: &GameApplication) {
        let renderer = app.renderer.get().expect("Renderer not initialized");

        // TODO: Think about the whole hecs threading. We should probably enqueue changes and batch do them in a big write lock?
        //  that way, many threads can perform reading instead of permanently waiting for the one writing thread. And once all
        //  calculations are done, commit things. On the other hand, updates are single threaded currently.
        //  ALSO: hecs claims that mutable access is better. But is it better than not having to lock the world?

        let mut write = app
            .entity_tracker
            .world()
            .write()
            .expect("World Write Lock");

        for (_, (renderable, location, orientation)) in
            write.query_mut::<(&mut Renderable, &TmpLocation, &TmpOrientation)>()
        {
            // Which coordinate system to pick? Obviously server side seems to be ADT, so probably
            // that needs to dominate the entities, so I think only converting for rendering is
            // appropriate.
            let quat: Quat = Quat::from_rotation_z(orientation.0).mul_quat(Quat::from_mat4(&adt_to_blender_rot()));
            let transform: Mat4 = Mat4::from_rotation_translation(quat, adt_to_blender_unaligned(location.0));

            if let Some(handle) = &renderable.handle {
                renderer.set_object_transform(handle, transform);
            } else {
                let dbg = self.debug_object(renderer);

                let object = Object {
                    mesh_kind: ObjectMeshKind::Static(dbg.0.clone()),
                    material: dbg.1.clone(),
                    transform,
                };

                renderable.handle = Some(renderer.add_object(object));
            }
        }
    }
}
