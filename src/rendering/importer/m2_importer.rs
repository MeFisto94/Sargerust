use crate::rendering::common::types::{Mesh, VertexBuffers};
use glam::{Vec2, Vec3};
use itertools::Itertools;
use sargerust_files::m2::types::{M2Asset, M2Batch, M2SkinProfile, M2SkinSection, M2Texture};

pub struct M2Importer {}

#[derive(Debug)]
pub struct M2Material {
    pub textures: Vec<M2Texture>,
}

impl M2Importer {
    pub fn create_mesh(asset: &M2Asset, skin: &M2SkinProfile, sub_mesh: &M2SkinSection) -> Mesh {
        let mut verts = Vec::<Vec3>::with_capacity(sub_mesh.vertexCount as usize);
        let mut normals = Vec::<Vec3>::with_capacity(sub_mesh.vertexCount as usize);
        // TODO: does every m2 have UVs?
        let mut uv0 = Vec::<Vec2>::with_capacity(sub_mesh.vertexCount as usize);
        let mut uv1 = Vec::<Vec2>::with_capacity(sub_mesh.vertexCount as usize);

        // I guess sub_mesh.vertex_start() would only help to slice skin.vertices, but we go through the look up table
        // anyway, so I don't see the benefit.
        for &v in &skin.vertices {
            let vert = &asset.vertices[v as usize];
            verts.push(Vec3::new(vert.pos.x, vert.pos.y, vert.pos.z));
            uv0.push(Vec2::new(vert.tex_coords[0].x, vert.tex_coords[0].y));
            uv1.push(Vec2::new(vert.tex_coords[1].x, vert.tex_coords[1].y));
            normals.push(Vec3::new(vert.normal.x, vert.normal.y, vert.normal.z));
        }

        let mut indices = Vec::<u32>::with_capacity(sub_mesh.indexCount as usize);
        let source_indices = skin
            .indices
            .iter()
            .skip(sub_mesh.index_start())
            .take(sub_mesh.indexCount as usize);

        for &i in source_indices {
            indices.push(i as u32); // TODO: Do we also have to add level here?
        }

        Mesh {
            index_buffer: indices,
            vertex_buffers: VertexBuffers {
                position_buffer: verts,
                normals_buffer: vec![],
                tangents_buffer: vec![],
                texcoord_buffer_0: uv0,
                texcoord_buffer_1: uv1,
                vertex_color_0: vec![],
            },
        }
    }

    #[deprecated(note = "This is not using the skin profiles")]
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

    pub fn create_m2_material(m2: &M2Asset, batch: &M2Batch) -> M2Material {
        let tex_ids = &m2.textureCombos
            [batch.textureComboIndex as usize..batch.textureComboIndex as usize + batch.textureCount as usize];
        let tex_uv_ids = &m2.textureCoordCombos[batch.textureCoordComboIndex as usize
            ..batch.textureCoordComboIndex as usize + batch.textureCount as usize];

        // TODO: to use this, we need M2Track, which is rather an animation topic. Also it did panic in the past.
        // let tex_weight_combos = &m2.textureWeightCombos[batch.textureWeightComboIndex as usize
        //     ..batch.textureWeightComboIndex as usize + batch.textureCount as usize];

        let mut textures = Vec::with_capacity(batch.textureCount as usize);

        for id in tex_ids {
            textures.push(m2.textures[*id as usize].clone());
        }

        M2Material { textures }
    }
}
