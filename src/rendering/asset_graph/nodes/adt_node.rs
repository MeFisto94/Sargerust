use crate::rendering::common::special_types::TerrainTextureLayerRend3;
use crate::rendering::common::types::{Material, Mesh};
use glam::{Affine3A, Mat4, Vec3A};
use image_blp::BlpImage;
use rend3::types::{MaterialHandle, MeshHandle, ObjectHandle, Texture2DHandle};
use sargerust_files::m2::types::M2Texture;
use sargerust_files::wdt::types::SMMapObjDef;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct ADTNode {
    pub doodads: Vec<Arc<DoodadReference>>,
    pub terrain: Vec<TerrainTile>,
    pub wmos: Vec<Arc<WMOReference>>,
}

#[derive(Debug)]
pub struct TerrainTile {
    pub position: Vec3A,
    pub mesh: RwLock<IRMesh>,
    pub object_handle: RwLock<Option<ObjectHandle>>,
    pub texture_layers: Vec<TerrainTextureLayerRend3>,
}

// TODO: commons.rs in nodes?
#[derive(Debug)]
pub struct DoodadReference {
    pub transform: Mat4,
    pub reference: NodeReference<M2Node>,
    // TODO: maybe we should have separate structs, graph/mapmanager and renderer side?
    pub renderer_object_handle: tokio::sync::RwLock<Option<ObjectHandle>>,
    pub renderer_has_texture: AtomicBool,
    pub renderer_is_complete: AtomicBool, // This is redundant with renderer_object_handle.is_some, but lock-free
}

impl DoodadReference {
    pub fn new(transform: Mat4, reference: String) -> Self {
        Self {
            transform,
            reference: NodeReference::new(reference),
            renderer_is_complete: AtomicBool::new(false),
            renderer_has_texture: AtomicBool::new(false),
            renderer_object_handle: tokio::sync::RwLock::new(None),
        }
    }
}

#[derive(Debug)]
pub struct M2Node {
    // the vec is immutable after creation, just the tex_reference#reference needs RwLocking
    pub tex_reference: Vec<Arc<IRTextureReference>>,
    pub mesh: RwLock<IRMesh>,
    pub material: RwLock<IRMaterial>,
    // TODO: RWLock inside IRMaterial#handle instead? As no-one should modify the material contents
    //  and whenever a node has resolved it's reference, it has to be existent/loaded?
}

#[derive(Debug)]
pub struct WMOReference {
    pub map_obj_def: SMMapObjDef,
    pub transform: Affine3A,
    pub reference: NodeReference<WMONode>,
    // TODO: This type is a clear sign that we should decouple the asset graph from tracking what has been loaded.
    pub obj_handles: RwLock<Vec<RwLock<Vec<ObjectHandle>>>>,
}

impl WMOReference {
    pub fn new(map_obj_def: SMMapObjDef, transform: Affine3A, reference: String) -> Self {
        Self {
            map_obj_def,
            transform,
            reference: NodeReference::new(reference),
            obj_handles: RwLock::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
pub struct WMONode {
    // Arcs are for the async loaders.
    pub doodads: Vec<Arc<DoodadReference>>, // TODO: They have DoodadSets that are referenced in the ADT
    // If this was a dedicated GroupReference struct, it could carry the group name. But currently we don't need the names anyway,
    // they are debug only.
    pub subgroups: Vec<Arc<NodeReference<WMOGroupNode>>>,
    pub materials: Vec<RwLock<IRMaterial>>,
    pub tex_references: Vec<Arc<IRTextureReference>>,
}

#[derive(Debug)]
pub struct WMOGroupNode {
    /// API trickery: One LoD Level is one batch
    pub mesh_batches: Vec<RwLock<IRMesh>>,
    pub material_ids: Vec<u8>,
}

/// DO NOT DERIVE CLONE FOR NODE REFERENCES, it breaks the renderer. As the renderer polls the lock
/// to see if it has been loaded async in the meantime.
#[derive(Debug)]
pub struct NodeReference<T> {
    pub reference_str: String,
    pub reference: RwLock<Option<Arc<T>>>,
}

impl<T> NodeReference<T> {
    pub fn new(reference_str: String) -> Self {
        Self {
            reference_str,
            reference: RwLock::new(None),
        }
    }
}

// TODO: the typedefs belong into rend3_backend, as they leak and wrap rend3 types
pub type IRMaterial = IRObject<Material, MaterialHandle>;
pub type IRMesh = IRObject<Mesh, MeshHandle>;
// Textures are failable
pub type IRTextureReference = IRObjectReference<Option<IRTexture>>;
pub type IRTexture = IRObject<BlpImage, Texture2DHandle>;

// TODO: are IRObjectReferences still needed, considering we have almost similar NodeReference<T>?
#[derive(Debug)]
pub struct IRObjectReference<T> {
    pub reference_str: String,
    pub reference: RwLock<Option<Arc<RwLock<T>>>>, // the inner RwLock is to mutate the IRObject, most notably it's handle. Could be put into handle, but then perspectively data needs to be mutable as well.
}

#[derive(Debug)]
pub struct IRObject<T, U> {
    // with hollowing, we would need to make this an Option<T>, but for now it is more
    // convenient not to have to do this.
    pub data: T,
    pub handle: Option<U>,
}

impl From<Mesh> for IRObject<Mesh, MeshHandle> {
    fn from(value: Mesh) -> Self {
        Self {
            data: value,
            handle: None,
        }
    }
}

impl From<Material> for IRObject<Material, MaterialHandle> {
    fn from(value: Material) -> Self {
        Self {
            data: value,
            handle: None,
        }
    }
}

impl From<M2Texture> for IRTextureReference {
    fn from(value: M2Texture) -> Self {
        Self {
            reference: RwLock::new(None),
            reference_str: value.filename,
        }
    }
}

impl From<String> for IRTextureReference {
    fn from(value: String) -> Self {
        Self {
            reference: RwLock::new(None),
            reference_str: value,
        }
    }
}
