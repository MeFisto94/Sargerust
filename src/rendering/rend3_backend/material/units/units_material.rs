use encase::ShaderType;
use rend3::types::{
    Material, RawTexture2DHandle, Sorting, Texture2DHandle, VERTEX_ATTRIBUTE_NORMAL, VERTEX_ATTRIBUTE_POSITION,
    VERTEX_ATTRIBUTE_TEXTURE_COORDINATES_0, VertexAttributeId,
};

#[derive(Debug, Clone)]
pub enum UnitsAlbedo {
    Textures([Option<Texture2DHandle>; 3]),
    Unicolor(glam::Vec4),
}

impl Default for UnitsAlbedo {
    fn default() -> Self {
        UnitsAlbedo::Unicolor(glam::Vec4::new(1.0, 0.0, 0.0, 1.0))
    }
}

#[derive(Debug, Clone, Default)]
pub struct UnitsMaterial {
    pub alpha_cutout: Option<f32>,
    pub albedo: UnitsAlbedo,
}

#[derive(Debug, Default, Copy, Clone, ShaderType)]
pub struct UnitsShaderMaterial {
    pub albedo_unicolor: glam::Vec4,
    pub alpha_cutout: f32,
    pub material_flag: u32,
}

impl Material for UnitsMaterial {
    type DataType = UnitsShaderMaterial;
    type TextureArrayType = [Option<RawTexture2DHandle>; 3];
    type RequiredAttributeArrayType = [&'static VertexAttributeId; 2];
    type SupportedAttributeArrayType = [&'static VertexAttributeId; 3];

    fn required_attributes() -> Self::RequiredAttributeArrayType {
        [
            &VERTEX_ATTRIBUTE_POSITION,
            &VERTEX_ATTRIBUTE_TEXTURE_COORDINATES_0,
        ]
    }

    fn supported_attributes() -> Self::SupportedAttributeArrayType {
        [
            &VERTEX_ATTRIBUTE_POSITION,
            &VERTEX_ATTRIBUTE_TEXTURE_COORDINATES_0,
            &VERTEX_ATTRIBUTE_NORMAL,
        ]
    }

    fn key(&self) -> u64 {
        match self.alpha_cutout {
            None => 0,
            Some(_) => 1,
        }
        // TODO: We could also add a new key to select shadow casting or not
    }

    fn sorting(&self) -> Sorting {
        Sorting::OPAQUE // Cutout doesn't require sorting
    }

    fn to_textures(&self) -> Self::TextureArrayType {
        match &self.albedo {
            UnitsAlbedo::Unicolor(_) => [None, None, None],
            UnitsAlbedo::Textures(textures) => [
                textures[0].as_ref().as_ref().map(|handle| handle.get_raw()),
                textures[1].as_ref().map(|handle| handle.get_raw()),
                textures[2].as_ref().map(|handle| handle.get_raw()),
            ],
        }
    }

    fn to_data(&self) -> Self::DataType {
        UnitsShaderMaterial {
            material_flag: 0,
            alpha_cutout: self.alpha_cutout.unwrap_or(0.0),
            albedo_unicolor: match self.albedo {
                UnitsAlbedo::Unicolor(unicolor) => unicolor,
                // signaling lime green
                UnitsAlbedo::Textures(_) => glam::Vec4::new(0.22, 1.0, 0.0, 1.0),
            },
        }
    }
}
