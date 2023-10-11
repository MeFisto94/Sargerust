use image_blp::BlpImage;
use log::error;
use rend3::types::{Texture, Texture2DHandle};
use rend3_routine::pbr::{AlbedoComponent, PbrMaterial, Transparency};

use crate::rendering::common::types::TransparencyType::{Blend, Cutout, Opaque};
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod, VertexBuffers};
use crate::rendering::create_texture_rgba8;

pub struct Rend3BackendConverter {}

impl Rend3BackendConverter {
    fn create_mesh_from_ir_internal(
        vertex_buffers: &VertexBuffers,
        indices: &Vec<u32>,
    ) -> Result<rend3::types::Mesh, anyhow::Error> {
        // TODO: introspect the individual buffers, and if they are >0, call .with_foo().
        let mut builder = rend3::types::MeshBuilder::new(
            vertex_buffers.position_buffer.clone(),
            rend3::types::Handedness::Right,
        );
        builder = builder.with_indices(indices.clone());

        if !vertex_buffers.texcoord_buffer_0.is_empty() {
            builder = builder.with_vertex_texture_coordinates_0(vertex_buffers.texcoord_buffer_0.clone());
        }

        if !vertex_buffers.normals_buffer.is_empty() {
            builder = builder.with_vertex_normals(vertex_buffers.normals_buffer.clone());
        }

        if !vertex_buffers.vertex_color_0.is_empty() {
            builder = builder.with_vertex_color_0(vertex_buffers.vertex_color_0.clone());
        }

        Ok(builder.build()?)
    }
    pub fn create_mesh_from_ir(mesh: &Mesh) -> Result<rend3::types::Mesh, anyhow::Error> {
        Rend3BackendConverter::create_mesh_from_ir_internal(&mesh.vertex_buffers, &mesh.index_buffer)
    }
    pub fn create_mesh_from_ir_lod(mesh: &MeshWithLod, lod_level: usize) -> Result<rend3::types::Mesh, anyhow::Error> {
        Rend3BackendConverter::create_mesh_from_ir_internal(&mesh.vertex_buffers, &mesh.index_buffers[lod_level])
    }

    pub fn create_material_from_ir(material: &Material, texture_handle: Option<Texture2DHandle>) -> PbrMaterial {
        if texture_handle.is_none() {
            // TODO: fail-safe somehow setting the material type differently.
            if let AlbedoType::Texture = material.albedo {
                error!("Material requires the presence of a texture");
            }
            if let AlbedoType::TextureWithName(name) = &material.albedo {
                error!("Material requires the presence of texture {name}");
            }
        }

        let ret = PbrMaterial {
            unlit: material.is_unlit,
            albedo: match material.albedo {
                AlbedoType::None => AlbedoComponent::None,
                AlbedoType::Vertex { srgb } => AlbedoComponent::Vertex { srgb },
                AlbedoType::Texture => AlbedoComponent::Texture(texture_handle.unwrap()),
                AlbedoType::TextureWithName(_) => AlbedoComponent::Texture(texture_handle.unwrap()),
                AlbedoType::Value(rgba) => AlbedoComponent::Value(rgba),
                AlbedoType::ValueVertex { value, srgb } => AlbedoComponent::ValueVertex { value, srgb },
            },
            transparency: match material.transparency {
                Cutout { cutout } => Transparency::Cutout { cutout },
                Opaque => Transparency::Opaque,
                Blend => Transparency::Blend,
            },
            ..PbrMaterial::default()
        };
        ret
    }

    pub fn create_texture_from_ir(texture: &BlpImage, mipmap_level: u8) -> Texture {
        create_texture_rgba8(texture, mipmap_level as usize)
    }
}
