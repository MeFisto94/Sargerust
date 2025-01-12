use rend3::types::ObjectHandle;

#[derive(Default, Debug, Clone)]
pub struct Renderable {
    pub handle: Option<ObjectHandle>,
}
