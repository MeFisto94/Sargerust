use crate::rendering::common::types::{AlbedoType, Material, Mesh, TransparencyType, VertexBuffers};
use glam::{Vec2, Vec3, Vec4};
use image_blp::BlpImage;
use itertools::Itertools;
use sargerust_files::m2::types::{M2Asset, M2SkinProfile};

pub struct M2Importer {}

impl M2Importer {
    pub fn create_mesh(asset: &M2Asset, skin: &M2SkinProfile) -> Mesh {
        let mut verts = Vec::<Vec3>::with_capacity(skin.vertices.len());

        // TODO: does every m2 have UVs?
        let mut uvs = Vec::<Vec2>::with_capacity(skin.vertices.len());

        for v in &skin.vertices {
            let vert = &asset.vertices[*v as usize];
            verts.push(Vec3::new(vert.pos.x, vert.pos.y, vert.pos.z));
            uvs.push(Vec2::new(vert.tex_coords[0].x, vert.tex_coords[0].y));
            // TODO: multiple UVs
        }

        let mut indices = Vec::<u32>::with_capacity(skin.indices.len());
        for &i in &skin.indices {
            indices.push(i as u32);
        }

        Mesh {
            index_buffer: indices,
            vertex_buffers: VertexBuffers {
                position_buffer: verts,
                normals_buffer: vec![],
                tangents_buffer: vec![],
                texcoord_buffer_0: uvs,
                texcoord_buffer_1: vec![],
                vertex_color_0: vec![],
            },
        }
    }

    pub fn create_lodable_mesh_base(asset: &M2Asset) -> VertexBuffers {
        let verts = asset
            .vertices
            .iter()
            .map(|v| Vec3::new(v.pos.x, v.pos.y, v.pos.z))
            .collect_vec();

        let uvs = asset
            .vertices
            .iter()
            .map(|v| Vec2::new(v.tex_coords[0].x, v.tex_coords[0].y))
            .collect_vec();

        VertexBuffers {
            position_buffer: verts,
            normals_buffer: vec![],
            tangents_buffer: vec![],
            texcoord_buffer_0: uvs,
            texcoord_buffer_1: vec![],
            vertex_color_0: vec![],
        }
    }

    pub fn create_lodable_mesh_lod(skin: &M2SkinProfile) -> Vec<u32> {
        // the indices are local to the values in skin.vertices, so we need to translate the index buffer
        skin.indices
            .iter()
            .map(|&idx| skin.vertices[idx as usize] as u32)
            .collect_vec()
    }

    pub fn create_material(blp_opt: &Option<BlpImage> /* TODO */) -> Material {
        Material {
            albedo: match blp_opt {
                Some(texture_handle) => AlbedoType::Texture, /*(TODO)*/
                None => AlbedoType::Value(Vec4::new(1.0, 0.0, 0.5, 1.0)),
            },
            is_unlit: true,
            transparency: TransparencyType::Cutout { cutout: 0.1 },
        }
    }

    pub fn create_material_texname(tex_name: &Option<String> /* TODO */) -> Material {
        Material {
            albedo: match tex_name {
                Some(texture_handle) => AlbedoType::TextureWithName(texture_handle.clone()),
                None => AlbedoType::Value(Vec4::new(1.0, 0.0, 0.5, 1.0)),
            },
            is_unlit: true,
            transparency: TransparencyType::Cutout { cutout: 0.1 },
        }
    }
}
