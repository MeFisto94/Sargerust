use glam::{Vec2, Vec3, Vec4};
use std::fmt::{Debug, Formatter};

#[derive(Clone)]
pub struct Mesh {
    pub vertex_buffers: VertexBuffers,
    pub index_buffer: Vec<u32>,
}

impl Debug for Mesh {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ vertex_buffers: {:?}, ", self.vertex_buffers)?;
        write!(f, "index_buffer: [{}] }}", self.index_buffer.len())
    }
}

#[derive(Clone)]
pub struct VertexBuffers {
    pub position_buffer: Vec<Vec3>,
    pub normals_buffer: Vec<Vec3>,
    pub tangents_buffer: Vec<Vec3>,
    pub texcoord_buffer_0: Vec<Vec2>,
    pub texcoord_buffer_1: Vec<Vec2>,
    pub vertex_color_0: Vec<[u8; 4]>,
}

impl Debug for VertexBuffers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ position_buffer: [{}], ", self.position_buffer.len())?;
        write!(f, "normals_buffer: [{}], ", self.normals_buffer.len())?;
        write!(f, "tangents_buffer: [{}], ", self.tangents_buffer.len())?;
        write!(f, "texcoord_buffer_0: [{}], ", self.texcoord_buffer_0.len())?;
        write!(f, "texcoord_buffer_1: [{}], ", self.texcoord_buffer_1.len())?;
        write!(f, "vertex_color_0: [{}] }}", self.vertex_color_0.len())
    }
}

impl Default for VertexBuffers {
    fn default() -> Self {
        VertexBuffers {
            position_buffer: vec![],
            normals_buffer: vec![],
            tangents_buffer: vec![],
            texcoord_buffer_0: vec![],
            texcoord_buffer_1: vec![],
            vertex_color_0: vec![],
        }
    }
}

// TODO: How would we model LODDABLE Meshes? One vertex buffer, multiple index buffers, Importers can support that
#[derive(Clone, Debug)]
pub struct MeshWithLod {
    pub vertex_buffers: VertexBuffers,
    pub index_buffers: Vec<Vec<u32>>,
}

/// Note: The structs in here are very much driven by the current backend/use-case and as such may change
/// quite often. This is especially true for the material, that has a complex structure.
#[derive(Clone, Debug)]
pub struct Material {
    pub albedo: AlbedoType,
    pub transparency: TransparencyType,
}

#[derive(Clone, Debug)]
pub enum AlbedoType {
    Value(Vec4),
    Texture, // TODO
    TextureWithName(String),
}

#[derive(Clone, Copy, Debug)]
pub enum TransparencyType {
    /// Alpha is completely ignored.
    Opaque,
    /// Pixels with alpha less than `cutout` is discorded.
    Cutout { cutout: f32 },
}
