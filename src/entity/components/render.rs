use crate::rendering::asset_graph::nodes::adt_node::{IRTexture, M2Node};
use rend3::types::ObjectHandle;
use std::sync::{Arc, RwLock};

#[derive(Default, Debug, Clone)]
pub enum RenderableSource {
    #[default]
    DebugCube,
    M2(Arc<M2Node>, Vec<Arc<RwLock<Option<IRTexture>>>>),
}
#[derive(Default, Debug, Clone)]
pub struct Renderable {
    pub handle: Option<ObjectHandle>,
    pub source: RenderableSource,
}
