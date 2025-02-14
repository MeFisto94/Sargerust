use encase::ShaderType;
use rend3::types::{
    Material, RawTexture2DHandle, Sorting, Texture2DHandle, VERTEX_ATTRIBUTE_NORMAL, VERTEX_ATTRIBUTE_POSITION,
    VERTEX_ATTRIBUTE_TEXTURE_COORDINATES_0, VertexAttributeId,
};
use rend3_routine::pbr::TransparencyType;

#[derive(Debug, Clone, Default)]
pub struct UnitsMaterial {
    pub texture_layers: [Option<Texture2DHandle>; 3],
}

#[derive(Debug, Default, Copy, Clone, ShaderType)]
pub struct UnitsShaderMaterial {
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
        TransparencyType::Opaque as u64
    }

    fn sorting(&self) -> Sorting {
        Sorting::OPAQUE
    }

    fn to_textures(&self) -> Self::TextureArrayType {
        [
            self.texture_layers[0]
                .as_ref()
                .as_ref()
                .map(|handle| handle.get_raw()),
            self.texture_layers[1]
                .as_ref()
                .map(|handle| handle.get_raw()),
            self.texture_layers[2]
                .as_ref()
                .map(|handle| handle.get_raw()),
        ]
    }

    fn to_data(&self) -> Self::DataType {
        UnitsShaderMaterial { material_flag: 0 }
    }
}
