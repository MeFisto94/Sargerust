use crate::rendering::asset_graph::nodes::adt_node::M2Node;
use rend3::types::ObjectHandle;
use std::sync::Arc;

#[derive(Default, Debug, Clone)]
pub enum RenderableSource {
    #[default]
    DebugCube,
    M2(Arc<M2Node>),
}
#[derive(Default, Debug, Clone)]
pub struct Renderable {
    pub handle: Option<ObjectHandle>,
    pub source: RenderableSource,
}
