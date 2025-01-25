use crate::rendering::common::types::{Mesh, VertexBuffers};
use glam::Vec3;
use log::warn;

pub enum MeshMerger {}

impl MeshMerger {
    /// Merge multiple meshes by combining their index buffers while taking the vertex buffer from the first.
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

    /// Merge multiple meshes by combining the vertex buffers and counting up the index buffers.
    /// Note that the intended use-case is to merge multiple material-separated meshes into one to build colliders.
    /// It is not guaranteed to be useful/reliable outside of that.
    pub fn merge_meshes_vertices_only(input_meshes: &[Mesh]) -> Mesh {
        let mut current_index = 0u32;

        let mut merged_mesh = Mesh {
            vertex_buffers: VertexBuffers::default(),
            index_buffer: Vec::with_capacity(input_meshes.iter().map(|m| m.index_buffer.len()).sum()),
        };

        if input_meshes.is_empty() {
            warn!("Merging 0 meshes");
            return merged_mesh;
        }

        for mesh in input_meshes {
            merged_mesh
                .vertex_buffers
                .position_buffer
                .extend_from_slice(&mesh.vertex_buffers.position_buffer);
            merged_mesh
                .vertex_buffers
                .normals_buffer
                .extend_from_slice(&mesh.vertex_buffers.normals_buffer);
            merged_mesh
                .vertex_buffers
                .tangents_buffer
                .extend_from_slice(&mesh.vertex_buffers.tangents_buffer);
            merged_mesh
                .vertex_buffers
                .texcoord_buffer_0
                .extend_from_slice(&mesh.vertex_buffers.texcoord_buffer_0);
            merged_mesh
                .vertex_buffers
                .texcoord_buffer_1
                .extend_from_slice(&mesh.vertex_buffers.texcoord_buffer_1);
            merged_mesh
                .vertex_buffers
                .vertex_color_0
                .extend_from_slice(&mesh.vertex_buffers.vertex_color_0);

            for &index in &mesh.index_buffer {
                merged_mesh.index_buffer.push(index + current_index);
            }

            current_index += mesh.index_buffer.iter().max().expect("Empty index buffer") + 1;
        }

        merged_mesh
    }

    // TODO: MeshUtils rather than MeshMerger?
    pub fn mesh_scale_position(mesh: &mut Mesh, scale: Vec3) {
        for pos in &mut mesh.vertex_buffers.position_buffer {
            *pos *= scale;
        }
    }
}
