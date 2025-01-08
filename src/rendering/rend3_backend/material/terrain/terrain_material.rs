use encase::ShaderType;
use rend3::types::{
    Material, RawTexture2DHandle, Sorting, Texture2DHandle, VERTEX_ATTRIBUTE_NORMAL, VERTEX_ATTRIBUTE_POSITION,
    VertexAttributeId,
};
use rend3_routine::pbr::TransparencyType;

pub struct TerrainMaterial {
    pub base_texture: Texture2DHandle,
    // 3 layers with alpha map each
    pub additional_layers: [Option<Texture2DHandle>; 6],
}

#[derive(Debug, Default, Copy, Clone, ShaderType)]
pub struct TerrainShaderMaterial {
    pub material_flag: u32,
}

impl Material for TerrainMaterial {
    type DataType = TerrainShaderMaterial;
    type TextureArrayType = [Option<RawTexture2DHandle>; 7];
    type RequiredAttributeArrayType = [&'static VertexAttributeId; 1];
    type SupportedAttributeArrayType = [&'static VertexAttributeId; 2];

    fn required_attributes() -> Self::RequiredAttributeArrayType {
        [&VERTEX_ATTRIBUTE_POSITION]
    }

    fn supported_attributes() -> Self::SupportedAttributeArrayType {
        [&VERTEX_ATTRIBUTE_POSITION, &VERTEX_ATTRIBUTE_NORMAL]
    }

    fn key(&self) -> u64 {
        TransparencyType::Opaque as u64
    }

    fn sorting(&self) -> Sorting {
        Sorting::OPAQUE
    }

    fn to_textures(&self) -> Self::TextureArrayType {
        [
            Some(self.base_texture.get_raw()),
            self.additional_layers
                .get(0)
                .and_then(|handle_opt| handle_opt.as_ref().map(|handle| handle.get_raw())),
            self.additional_layers
                .get(1)
                .and_then(|handle_opt| handle_opt.as_ref().map(|handle| handle.get_raw())),
            self.additional_layers
                .get(2)
                .and_then(|handle_opt| handle_opt.as_ref().map(|handle| handle.get_raw())),
            self.additional_layers
                .get(3)
                .and_then(|handle_opt| handle_opt.as_ref().map(|handle| handle.get_raw())),
            self.additional_layers
                .get(4)
                .and_then(|handle_opt| handle_opt.as_ref().map(|handle| handle.get_raw())),
            self.additional_layers
                .get(5)
                .and_then(|handle_opt| handle_opt.as_ref().map(|handle| handle.get_raw())),
        ]
    }

    fn to_data(&self) -> Self::DataType {
        TerrainShaderMaterial { material_flag: 0 }
    }
}
