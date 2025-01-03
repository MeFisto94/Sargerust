use encase::ShaderType;
use rend3::types::{
    Material, RawTexture2DHandle, Sorting, VERTEX_ATTRIBUTE_NORMAL, VERTEX_ATTRIBUTE_POSITION, VertexAttributeId,
};

#[derive(Default)]
pub struct TerrainMaterial {}

#[derive(Debug, Default, Copy, Clone, ShaderType)]
pub struct TerrainShaderMaterial {
    pub material_flag: u32,
}

impl Material for TerrainMaterial {
    type DataType = TerrainShaderMaterial;
    type TextureArrayType = [Option<RawTexture2DHandle>; 10];
    type RequiredAttributeArrayType = [&'static VertexAttributeId; 1];
    type SupportedAttributeArrayType = [&'static VertexAttributeId; 2];

    fn required_attributes() -> Self::RequiredAttributeArrayType {
        [&VERTEX_ATTRIBUTE_POSITION]
    }

    fn supported_attributes() -> Self::SupportedAttributeArrayType {
        [&VERTEX_ATTRIBUTE_POSITION, &VERTEX_ATTRIBUTE_NORMAL]
    }

    fn key(&self) -> u64 {
        0u64
    }

    fn sorting(&self) -> Sorting {
        Sorting::OPAQUE
    }

    fn to_textures(&self) -> Self::TextureArrayType {
        [None; 10]
    }

    fn to_data(&self) -> Self::DataType {
        TerrainShaderMaterial { material_flag: 0 }
    }
}
