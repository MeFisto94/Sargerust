use glam::{Vec2, Vec3};
use itertools::Itertools;

use sargerust_files::wmo::types::WMOGroupAsset;

use crate::rendering::common::types::VertexBuffers;

pub struct WMOGroupImporter {
}

impl WMOGroupImporter {
    // The start and end slices are batches of a bigger buffer, as such we export them as LoddableMeshes here
    // so they can share their vertex buffer at least. Also note that slicing distinct meshes didn't work, because
    // somehow indices have been exceeding the vertices between start and end vertex.
    pub fn create_lodable_mesh_base(asset: &WMOGroupAsset) -> VertexBuffers {
        /* [start_vertex..last_vertex + 1]: NOTE: Currently, the vertex buffer slicing is disabled,
         as there seem to be indices that exceed the vertex buffer range, failing validation */
        let position_buffer = asset.movt.vertexList.iter().map(|v| Vec3::new(v.x, v.y, v.z)).collect();
        let normals_buffer = asset.monr.normalList.iter().map(|v| Vec3::new(v.x, v.y, v.z)).collect();
        let uv = asset.motv.textureVertexList.iter().map(|v| Vec2::new(v.x, v.y)).collect();

        VertexBuffers {
            position_buffer,
            normals_buffer,
            tangents_buffer: vec![],
            texcoord_buffer_0: uv,
            texcoord_buffer_1: vec![],
            vertex_color_0: vec![]
        }
    }

    pub fn create_lodable_mesh_lod(asset: &WMOGroupAsset, start_index: usize, index_count: usize) -> Vec<u32> {
        asset.movi.indices[start_index..start_index + index_count].iter().map(|&i| i as u32).collect_vec()
    }
}