use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use glam::{Affine3A, Vec3, Vec3A};
use image_blp::convert::blp_to_image;
use image_blp::BlpImage;
use rend3::types::{MaterialHandle, MeshHandle, Object, ObjectHandle};
use rend3::Renderer;

use crate::rendering::common::coordinate_systems;
use crate::rendering::common::highlevel_types::PlacedDoodad;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod, TransparencyType};
use crate::rendering::rend3_backend::Rend3BackendConverter;

pub mod application;
pub mod asset_graph;
pub mod common;
pub mod importer;
pub mod loader;
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
        mip_source: rend3::types::MipmapSource::Uploaded,
    }
}

fn create_object(transform: Affine3A, mesh_handle: MeshHandle, material_handle: MaterialHandle) -> Object {
    rend3::types::Object {
        mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
        material: material_handle,
        transform: transform.into(),
    }
}

pub fn add_terrain_chunks(
    terrain_chunk: &Vec<(Vec3, Mesh)>,
    renderer: &Arc<Renderer>,
    object_list: &mut Vec<ObjectHandle>,
) {
    for (position, _mesh) in terrain_chunk {
        let mesh = Rend3BackendConverter::create_mesh_from_ir(_mesh).unwrap();

        let mesh_handle = renderer
            .add_mesh(mesh)
            .expect("Creating the mesh is successful");

        // TODO: here, the renderer defines the material for the terrain, but why? It should be stored outside of terrain_chunk maybe because it applies for every ADT tile at least? Perspectively...
        let _material = Material {
            is_unlit: true,
            albedo: AlbedoType::Vertex { srgb: true },
            transparency: TransparencyType::Opaque,
        };
        let material = Rend3BackendConverter::create_material_from_ir(&_material, None);
        let material_handle = renderer.add_material(material);

        // TODO: per definition, IR should be in blender-space, so we need to transform the translation at the very least. Or rather directly return "tt"
        // Don't ask me where flipping the z and the heightmap values comes from.
        // Actually, I think I flipped everything there is now, for consistency with ADT and where it should belong (i.e. 16k, 16k; not negative area)
        let tt = coordinate_systems::adt_to_blender_transform(Vec3A::new(position.x, position.y, position.z));
        let object = Object {
            mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
            material: material_handle,
            // I think mat * translation rotates our translation and as such is basically always wrong. It can't have ever rotated things as a side effect?
            transform: tt,
        };

        let _object_handle = renderer.add_object(object);
        object_list.push(_object_handle);
    }
}

pub fn add_wmo_groups<'a, W>(
    wmos: W,
    textures: &HashMap<String, BlpImage>,
    renderer: &Arc<Renderer>,
    object_list: &mut Vec<ObjectHandle>,
) where
    W: IntoIterator<
        Item = (
            &'a Affine3A,
            &'a Vec<(MeshWithLod, Vec<Material> /* per lod */)>,
        ),
    >,
{
    for (transform, wmo_groups) in wmos {
        for (lod_mesh, materials) in wmo_groups {
            // One "lod" has it's own material here, but technically it's a wmo group batch.
            for (i, material) in materials.iter().enumerate() {
                let mesh = Rend3BackendConverter::create_mesh_from_ir_lod(lod_mesh, i).unwrap();
                let mesh_handle = renderer
                    .add_mesh(mesh)
                    .expect("Creating the mesh is successful");

                // TODO: concept work for textures
                let blp_opt = match &material.albedo {
                    AlbedoType::TextureWithName(tex_name) => textures.get(tex_name),
                    _ => None,
                };

                let mapped_tex = blp_opt.as_ref().map(|tex| {
                    renderer
                        .add_texture_2d(create_texture_rgba8(tex, 0))
                        .expect("Creating the texture is successful")
                });
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

pub fn add_placed_doodads(
    placed_doodads: &Vec<PlacedDoodad>,
    renderer: &Arc<Renderer>,
    object_list: &mut Vec<ObjectHandle>,
) {
    for dad in placed_doodads {
        let m2 = dad.m2.deref();
        // Create mesh and calculate smooth normals based on vertices
        let mesh = Rend3BackendConverter::create_mesh_from_ir(&m2.mesh).unwrap();
        let mesh_handle = renderer.add_mesh(mesh).expect("Mesh creation successful");

        // TODO: concept work for textures
        let mapped_tex = m2.blp_opt.as_ref().map(|tex| {
            renderer
                .add_texture_2d(create_texture_rgba8(tex, 0))
                .expect("Texture creation successful")
        });
        let material = Rend3BackendConverter::create_material_from_ir(&m2.material, mapped_tex);
        let material_handle = renderer.add_material(material);

        // Combine the mesh and the material with a location to give an object.
        let object = create_object(dad.transform, mesh_handle, material_handle);

        // Creating an object will hold onto both the mesh and the material
        // even if they are deleted.
        //
        // We need to keep the object handle alive.
        let _object_handle = renderer.add_object(object);
        object_list.push(_object_handle);
    }
}
