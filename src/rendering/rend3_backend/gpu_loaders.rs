use crate::rendering::rend3_backend::{IRMaterial, IRMesh, IRTexture, Rend3BackendConverter};
use rend3::Renderer;
use rend3::types::{MaterialHandle, MeshHandle, Texture2DHandle};
use std::ops::DerefMut;
use std::sync::{Arc, RwLock};

pub fn gpu_load_mesh(renderer: &Arc<Renderer>, mesh: &RwLock<IRMesh>) -> MeshHandle {
    {
        if let Some(handle) = mesh.read().expect("Mesh Read Lock").handle.as_ref() {
            return handle.clone();
        }
    }

    let mut mesh_lock = mesh.write().expect("Mesh Write Lock");
    let render_mesh = Rend3BackendConverter::create_mesh_from_ir(&mesh_lock.data).expect("Mesh building successful");
    let mesh_handle = renderer
        .add_mesh(render_mesh)
        .expect("Mesh creation successful");
    mesh_lock.deref_mut().handle = Some(mesh_handle.clone());
    mesh_handle
}

pub fn gpu_load_material(
    renderer: &Arc<Renderer>,
    material: &RwLock<IRMaterial>,
    texture_handle: Option<Texture2DHandle>,
) -> MaterialHandle {
    {
        if let Some(handle) = material.read().expect("Material Read Lock").handle.as_ref() {
            return handle.clone();
        }
    }
    let mut material_lock = material.write().expect("Material Write Lock");
    let render_mat = Rend3BackendConverter::create_material_from_ir(&material_lock.data, texture_handle);
    let material_handle = renderer.add_material(render_mat);
    material_lock.deref_mut().handle = Some(material_handle.clone());
    material_handle
}

pub fn gpu_load_texture(
    renderer: &Arc<Renderer>,
    texture_reference: &RwLock<Option<Arc<RwLock<Option<IRTexture>>>>>,
) -> Option<Texture2DHandle> {
    {
        let tex_arc = texture_reference.read().expect("Texture Read Lock");
        if let Some(opt_handle) = tex_arc.as_ref() {
            {
                let tex_lock = opt_handle.read().expect("Texture Read Lock 2");
                if let Some(tex_handle) = tex_lock.as_ref() {
                    if let Some(handle) = tex_handle.handle.as_ref() {
                        return Some(handle.clone());
                    } // else: texture not added to the GPU yet - continue with the write lock
                } else {
                    // texture loading error?
                    return None;
                }
            }
        } else {
            // else: texture (reference?) not loaded yet.
            // TODO: the caller should prevent calling in that case and unwrap the lock? The caller should at least distinguish between texture not loaded (grey diffuse color) and texture loading error (pink!)
            return None;
        }
    }

    let tex_wlock = texture_reference.write().expect("Texture Write Lock");
    let mut tex_iwlock = tex_wlock
        .as_ref()
        .expect("unreachable!")
        .as_ref()
        .write()
        .expect("Texture internal write lock");

    let tex = tex_iwlock.as_mut().expect("unreachable!");
    // TODO: What do we do with the mipmap level? From 0 to tex.data.image_count() as u8 - 1
    let texture = Rend3BackendConverter::create_texture_from_ir(&tex.data, 0);
    let texture_handle = renderer
        .add_texture_2d(texture)
        .expect("Texture creation successful");
    tex.handle = Some(texture_handle.clone());
    Some(texture_handle)
}
