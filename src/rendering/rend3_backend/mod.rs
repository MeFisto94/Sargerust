use crate::rendering::asset_graph::nodes::adt_node::{IRObject, IRObjectReference};
use crate::rendering::common::types::TransparencyType::Cutout;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod, VertexBuffers};
use crate::rendering::importer::m2_importer::ModelMaterial;
use crate::rendering::rend3_backend::material::units::units_material::{UnitsAlbedo, UnitsMaterial};
use image_blp::convert::blp_to_image;
use image_blp::{BlpDxtn, BlpImage, DxtnFormat};
use log::error;
use rend3::types::{MaterialHandle, MeshHandle, Texture, Texture2DHandle};
use std::num::NonZeroU32;

pub mod gpu_loaders;
pub mod material;

pub type IRMaterial = IRObject<Material, MaterialHandle>;
pub type IRM2Material = IRObject<ModelMaterial, MaterialHandle>;
pub type IRMesh = IRObject<Mesh, MeshHandle>;
// TODO: Why are textures failable? Depending on the context that may not be a good idea. As is the file location for these.
// Textures are failable
pub type IRTextureReference = IRObjectReference<Option<IRTexture>>;
pub type IRTexture = IRObject<BlpImage, Texture2DHandle>;

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

    pub fn create_material_from_ir(material: &Material, texture_handle: Option<Texture2DHandle>) -> UnitsMaterial {
        if texture_handle.is_none() {
            // TODO: fail-safe somehow setting the material type differently.
            if let AlbedoType::Texture = material.albedo {
                error!("Material requires the presence of a texture");
            }
            if let AlbedoType::TextureWithName(name) = &material.albedo {
                error!("Material requires the presence of texture {name}");
            }
        }

        let ret = UnitsMaterial {
            albedo: match material.albedo {
                AlbedoType::Value(rgba) => UnitsAlbedo::Unicolor(rgba),
                _ => UnitsAlbedo::Textures([texture_handle, None, None]),
            },
            alpha_cutout: match material.transparency {
                Cutout { cutout } => Some(cutout),
                _ => None,
            },
            ..UnitsMaterial::default()
        };
        ret
    }

    pub fn create_texture_from_ir(texture: &BlpImage, label: Option<&str>, mipmap_level: usize) -> Texture {
        let image = blp_to_image(texture, mipmap_level).expect("decode");
        let image_dims = glam::UVec2::new(image.width(), image.height());
        let image_data = image.into_rgba8();

        Texture {
            label: label.map(|s| s.to_string()),
            data: image_data.into_raw(),
            format: rend3::types::TextureFormat::Rgba8UnormSrgb,
            size: image_dims,
            mip_count: rend3::types::MipmapCount::ONE,
            mip_source: rend3::types::MipmapSource::Uploaded,
        }
    }

    /// Upload this image as block compressed texture already containing mipmaps. This assumes a DXT format, otherwise
    /// use the more generic create_texture_from_ir.
    pub fn create_texture_from_ir_dxtn(texture: &BlpDxtn, label: Option<&str>, image_dims: (u32, u32)) -> Texture {
        let mipmap_count = NonZeroU32::try_from(texture.images.len() as u32).expect("Non empty texture...");

        let format = match texture.format {
            DxtnFormat::Dxt1 => rend3::types::TextureFormat::Bc1RgbaUnormSrgb,
            DxtnFormat::Dxt3 => rend3::types::TextureFormat::Bc2RgbaUnormSrgb,
            DxtnFormat::Dxt5 => rend3::types::TextureFormat::Bc3RgbaUnormSrgb,
        };

        let px_per_byte = match texture.format {
            DxtnFormat::Dxt1 => 2,
            DxtnFormat::Dxt3 | DxtnFormat::Dxt5 => 1,
        };

        // CMap seems to be the "color map" (i.e. a lookup table / palette), which should only be relevant for DIRECT,
        // which shouldn't be part of Dxtn, I hope. On the other hand dxtn is part of the direct module...

        // Due to some broken mip map layers (https://wowdev.wiki/BLP#Compressed_textures), we would have to re-allocate,
        // so we overshoot a bit (last levels are always 8/16 bytes)
        let mut buf = Vec::with_capacity(
            texture
                .images
                .iter()
                .map(|img| img.content.iter().len())
                .sum::<usize>()
                + 8 * texture.images.len(),
        );

        for (mip, image) in texture.images.iter().enumerate() {
            let real_size = ((image_dims.0 >> mip).max(1), (image_dims.1 >> mip).max(1));
            let physical_size = (
                ((real_size.0 + 4 - 1) / 4) * 4,
                ((real_size.1 + 4 - 1) / 4) * 4,
            );
            let actual_size = image.content.len() as u32;
            let physical_accumulated_size = physical_size.0 * physical_size.1 / px_per_byte;

            buf.extend(&image.content);
            if physical_accumulated_size > actual_size {
                buf.extend(std::iter::repeat(0).take((physical_accumulated_size - actual_size) as usize));
            }
        }

        Texture {
            label: label.map(|s| s.to_string()),
            data: buf,
            format,
            size: glam::UVec2::new(image_dims.0, image_dims.1),
            mip_count: rend3::types::MipmapCount::Specific(mipmap_count),
            mip_source: rend3::types::MipmapSource::Uploaded,
        }
    }
}
