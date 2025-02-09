use crate::rendering::asset_graph::nodes::adt_node::{IRTexture, M2Node};
use rend3::types::ObjectHandle;
use sargerust_files::m2::types::{M2TextureFlags, M2TextureType};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

#[derive(Default, Debug, Clone)]
pub enum RenderableSource {
    #[default]
    DebugCube,
    M2(
        Arc<M2Node>,
        Vec<(
            M2TextureType,
            M2TextureFlags,
            Arc<RwLock<Option<IRTexture>>>,
        )>,
        HashSet<u16>,
    ),
}
#[derive(Default, Debug, Clone)]
pub struct Renderable {
    // Order: We can't do Vec<Option> for partial loading, because we can't properly distinguish subparts of a single M2
    // then, also it should only be a few meshes for things already in memory anyway. So it's actually better this way.
    pub handles: Option<Vec<ObjectHandle>>,
    pub source: RenderableSource,
}
