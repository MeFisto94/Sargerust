use std::sync::RwLock;

use glam::{Vec2, Vec3, Vec4};
use itertools::Itertools;
use log::{debug, info, trace};

use sargerust_files::wmo::reader::WMOReader;
use sargerust_files::wmo::types::{WMOGroupAsset, WMORootAsset};

use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::nodes::adt_node::WMOGroupNode;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod, TransparencyType, VertexBuffers};

pub struct WMOGroupImporter {}

impl WMOGroupImporter {
    // The start and end slices are batches of a bigger buffer, as such we export them as LoddableMeshes here
    // so they can share their vertex buffer at least. Also note that slicing distinct meshes didn't work, because
    // somehow indices have been exceeding the vertices between start and end vertex.
    pub fn create_lodable_mesh_base(asset: &WMOGroupAsset) -> VertexBuffers {
        /* [start_vertex..last_vertex + 1]: NOTE: Currently, the vertex buffer slicing is disabled,
        as there seem to be indices that exceed the vertex buffer range, failing validation */
        let position_buffer = asset
            .movt
            .vertexList
            .iter()
            .map(|v| Vec3::new(v.x, v.y, v.z))
            .collect();
        let normals_buffer = asset
            .monr
            .normalList
            .iter()
            .map(|v| Vec3::new(v.x, v.y, v.z))
            .collect();
        let uv = asset
            .motv
            .textureVertexList
            .iter()
            .map(|v| Vec2::new(v.x, v.y))
            .collect();

        VertexBuffers {
            position_buffer,
            normals_buffer,
            tangents_buffer: vec![],
            texcoord_buffer_0: uv,
            texcoord_buffer_1: vec![],
            vertex_color_0: vec![],
        }
    }

    pub fn create_lodable_mesh_lod(asset: &WMOGroupAsset, start_index: usize, index_count: usize) -> Vec<u32> {
        asset.movi.indices[start_index..start_index + index_count]
            .iter()
            .map(|&i| i as u32)
            .collect_vec()
    }

    // MPQLoader: we dynamically load the WMO Groups based upon WMORootAsset. Could change that but this yields error potential.
    // TODO: Still do it, to separate loading/parsing from importing (which is asset -> IR)
    #[deprecated(note = "For some reason this isn't called anywhere anymore which is confusing")]
    pub fn load_wmo_groups(loader: &MPQLoader, wmo: &WMORootAsset, path: &str) -> Vec<(MeshWithLod, Vec<Material>)> {
        // just for debug???
        for group in &wmo.mogi.groupInfoList {
            if group.nameoffset != -1 {
                let offset = wmo.mogn.offset_lookup[&(group.nameoffset as u32)];
                trace!("Loading WMO Group {}", &wmo.mogn.groupNameList[offset]);
            }
        }

        let mut group_list = Vec::new();
        for x in 0..wmo.mohd.nGroups {
            let cursor = &mut std::io::Cursor::new(
                loader
                    .load_raw_owned(&format!("{}_{:0>3}.wmo", path, x))
                    .unwrap(),
            );
            group_list.push(WMOReader::parse_group(cursor).unwrap());
        }

        group_list
            .iter()
            .map(|group| {
                let mesh_base = WMOGroupImporter::create_lodable_mesh_base(group);
                let indices = group
                    .moba
                    .batchList
                    .iter()
                    .map(|batch| {
                        WMOGroupImporter::create_lodable_mesh_lod(
                            group,
                            batch.startIndex as usize,
                            batch.count as usize,
                        )
                    })
                    .collect_vec();

                let materials = group
                    .moba
                    .batchList
                    .iter()
                    .map(|batch| {
                        let first_material = match batch.material_id {
                            0xFF => None,
                            _ => Some(&wmo.momt.materialList[batch.material_id as usize]),
                        };

                        let txname_opt = first_material.map(|mat| {
                            let offset = wmo.motx.offsets[&mat.texture_1];
                            wmo.motx.textureNameList[offset].clone()
                        });

                        let mut mat = Material {
                            albedo: AlbedoType::Value(Vec4::new(0.6, 0.6, 0.6, 1.0)),
                            transparency: TransparencyType::Opaque,
                        };

                        if let Some(_mat) = first_material {
                            mat.albedo = match txname_opt {
                                Some(texture_handle) => AlbedoType::TextureWithName(texture_handle),
                                None => AlbedoType::Value(Vec4::new(
                                    _mat.diffColor.r as f32 / 255.0,
                                    _mat.diffColor.g as f32 / 255.0,
                                    _mat.diffColor.b as f32 / 255.0,
                                    _mat.diffColor.a as f32 / 255.0,
                                )),
                            };

                            mat.transparency = match _mat.blendMode {
                                0 => TransparencyType::Opaque,
                                1 => TransparencyType::Cutout { cutout: 0.5 },
                                _ => {
                                    debug!("Unknown blend mode: {}", _mat.blendMode);
                                    TransparencyType::Opaque
                                }
                            };
                        }

                        mat
                    })
                    .collect_vec();
                (
                    MeshWithLod {
                        vertex_buffers: mesh_base,
                        index_buffers: indices,
                    },
                    materials,
                )
            })
            .collect_vec()
    }

    pub fn load_wmo_group(loader: &MPQLoader, path: &str) -> WMOGroupNode {
        // This duplicates the above, sadly.
        let group = WMOReader::parse_group(&mut std::io::Cursor::new(
            loader.load_raw_owned(path).unwrap(),
        ))
        .unwrap();

        // TODO: Currently we can't slice down the vertex buffer properly anyway. But at some point MeshhWithLod should also work with the asset graph
        let mesh_base = WMOGroupImporter::create_lodable_mesh_base(&group);
        let mut material_ids = Vec::new();
        let mut mesh_batches = Vec::new();

        for batch in &group.moba.batchList {
            let index =
                WMOGroupImporter::create_lodable_mesh_lod(&group, batch.startIndex as usize, batch.count as usize);
            material_ids.push(batch.material_id); // 0xFF is no material.

            mesh_batches.push(RwLock::new(
                Mesh {
                    vertex_buffers: mesh_base.clone(),
                    index_buffer: index,
                }
                .into(),
            ));
        }

        WMOGroupNode {
            mesh_batches,
            material_ids,
        }
    }
}
