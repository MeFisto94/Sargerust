use crate::rendering::common::types::{Mesh, VertexBuffers};
use log::warn;

pub enum MeshMerger {}

impl MeshMerger {
    /// Mesh multiple merges by combining their index buffers while taking the vertex buffer from the first.
    /// This is useful for e.g. <code>WMOGroupNode#mesh_batches</code>, because those are initially
    /// only separated by index buffer starts/ends, it's just our architecture looses that information.
    pub fn merge_meshes_index_only(input_meshes: &[Mesh]) -> Mesh {
        let mut merged_mesh = Mesh {
            vertex_buffers: VertexBuffers::default(),
            index_buffer: Vec::with_capacity(input_meshes.iter().map(|m| m.index_buffer.len()).sum()),
        };

        if input_meshes.is_empty() {
            warn!("Merging 0 meshes");
            return merged_mesh;
        }

        merged_mesh.vertex_buffers = input_meshes[0].vertex_buffers.clone();

        for mesh in input_meshes {
            merged_mesh
                .index_buffer
                .extend_from_slice(&mesh.index_buffer);
        }

        merged_mesh
    }
}
