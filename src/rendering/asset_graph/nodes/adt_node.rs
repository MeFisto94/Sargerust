use crate::rendering::common::types::{Material, Mesh, MeshWithLod};
use glam::{Affine3A, Mat4, Vec3A};
use image_blp::BlpImage;
use rend3::types::{MaterialHandle, MeshHandle, Texture2DHandle};
use sargerust_files::m2::types::M2Texture;
use sargerust_files::wdt::types::SMMapObjDef;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct ADTNode {
    pub doodads: Vec<DoodadReference>,
    pub terrain: Vec<(Vec3A, RwLock<IRMesh>)>,
    pub wmos: Vec<WMOReference>,
}

// TODO: commons.rs in nodes?
#[derive(Debug)]
pub struct DoodadReference {
    pub transform: Mat4,
    pub reference: NodeReference<M2Node>,
}

impl DoodadReference {
    pub fn new(transform: Mat4, reference: String) -> Self {
        Self {
            transform,
            reference: NodeReference::new(reference),
        }
    }
}

#[derive(Debug)]
pub struct M2Node {
    // the vec is immutable after creation, just the tex_reference#reference needs RwLocking
    pub tex_reference: Vec<IRTextureReference>,
    pub mesh: RwLock<IRMesh>,
    pub material: RwLock<IRMaterial>, // TODO: RWLock inside IRMaterial#handle instead? As no-one should modify the material contents and whenever a node has resolved it's reference, it has to be existant/loaded?
}

#[derive(Debug)]
pub struct WMOReference {
    pub map_obj_def: SMMapObjDef,
    pub transform: Affine3A,
    pub reference: NodeReference<WMONode>,
}

impl WMOReference {
    pub fn new(map_obj_def: SMMapObjDef, transform: Affine3A, reference: String) -> Self {
        Self {
            map_obj_def,
            transform,
            reference: NodeReference::new(reference),
        }
    }
}

#[derive(Debug)]
pub struct WMONode {
    // TODO: transform of a WMONode, much like with doodads, comes from it's references
    // TODO: dooad references also need to be translated based on the transform of this nodes references transform.
    pub doodads: Vec<DoodadReference>, // TODO: They have DoodadSets that are referenced in the ADT
    // IF this was a dedicated GroupReference, it could carry the group name
    pub subgroups: Vec<NodeReference<WMOGroupNode>>,
    pub materials: Vec<RwLock<IRMaterial>>,
    pub tex_references: Vec<IRTextureReference>,
}

#[derive(Debug)]
pub struct WMOGroupNode {
    /// API trickery: One LoD Level is one batch
    pub mesh_batches: Vec<RwLock<IRMesh>>,
    pub material_ids: Vec<u8>,
}

/// DO NOT DERIVCE CLONE FOR NODE REFERENCES, it breaks the renderer. As the renderer polls the lock
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

type IRMaterialReference = IRObjectReference<IRMaterial>;
pub type IRMaterial = IRObject<Material, MaterialHandle>;
type IRMeshReference = IRObjectReference<IRMesh>;
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
