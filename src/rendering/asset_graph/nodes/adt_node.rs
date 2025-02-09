use crate::rendering::common::special_types::TerrainTextureLayerRend3;
use crate::rendering::common::types::{Material, Mesh};
use crate::rendering::importer::m2_importer::M2Material;
use crate::rendering::rend3_backend::{IRM2Material, IRMaterial, IRMesh, IRTextureReference};
use glam::{Affine3A, Mat4, Vec3A};
use rend3::types::{MaterialHandle, MeshHandle, ObjectHandle};
use sargerust_files::m2::types::M2Texture;
use sargerust_files::wdt::types::SMMapObjDef;
use std::hash::{Hash, Hasher};
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
    pub renderer_object_handles: tokio::sync::RwLock<Vec<ObjectHandle>>,
    pub renderer_waiting_for_textures: AtomicBool, // Stage 1: unicolor objects
    pub renderer_is_complete: AtomicBool,          // Stage 2: textures applied
}

impl Hash for DoodadReference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.reference.hash(state);
        for val in self.transform.to_cols_array() {
            (val as u32).hash(state);
        }
    }
}

impl PartialEq for DoodadReference {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference && self.transform == other.transform
    }
}

impl Eq for DoodadReference {}

impl DoodadReference {
    pub fn new(transform: Mat4, reference: String) -> Self {
        Self {
            transform,
            reference: NodeReference::new(reference),
            renderer_is_complete: AtomicBool::new(false),
            renderer_waiting_for_textures: AtomicBool::new(false),
            renderer_object_handles: tokio::sync::RwLock::new(vec![]),
        }
    }
}

#[derive(Debug)]
pub struct M2Node {
    // the vec is immutable after creation, just the tex_reference#reference needs RwLocking
    // TODO: Rename
    // TODO: rustdoc linking could somehow work with M2TextureType#variant.None, but then it doesn't even find the parent atm.
    /// These are the texture references that have been referenced with a [`M2TextureType`] of None.
    pub tex_reference: Vec<Arc<IRTextureReference>>,
    pub dynamic_tex_references: Vec<M2Texture>,

    // Since we have multiple meshes, we need to link the materials to it.
    pub meshes_and_materials: Vec<(RwLock<IRMesh>, RwLock<IRM2Material>, u16)>,
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
    /// According to the wiki, the mesh batches are *not* (as previously noted) LoDs, but rather proper
    /// mesh batches, whereby the meshes with the same material id are batched together to reduce
    /// draw calls.
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

impl<T> Hash for NodeReference<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.reference_str.hash(state);
    }
}

impl<T> PartialEq for NodeReference<T> {
    fn eq(&self, other: &Self) -> bool {
        self.reference_str == other.reference_str
    }
}

impl<T> Eq for NodeReference<T> {}

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

impl From<M2Material> for IRM2Material {
    fn from(value: M2Material) -> Self {
        Self {
            data: value,
            handle: None,
        }
    }
}
