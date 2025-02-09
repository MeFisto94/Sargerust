use crate::rendering::asset_graph::nodes::adt_node::IRObject;
use crate::rendering::rend3_backend::IRTextureReference;
use rend3::types::Texture2DHandle;
use std::sync::{Arc, RwLock};

pub struct TerrainTextureLayer {
    pub texture_path: String,
    pub alpha_map: Option<Vec<u8>>,
}

// TODO: this belongs in a different folder then, obviously.
#[derive(Debug)]
pub struct TerrainTextureLayerRend3 {
    pub base_texture_ref: Arc<IRTextureReference>,
    pub alpha_map_ref: Option<RwLock<IRObject<Vec<u8>, Texture2DHandle>>>,
}

impl TerrainTextureLayerRend3 {
    pub fn new(
        base_texture_ref: Arc<IRTextureReference>,
        alpha_map_ref: Option<RwLock<IRObject<Vec<u8>, Texture2DHandle>>>,
    ) -> Self {
        Self {
            base_texture_ref,
            alpha_map_ref,
        }
    }
}
