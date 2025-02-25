use crate::rendering::common::coordinate_systems::GRID_SIZE;
use crate::rendering::common::special_types::TerrainTextureLayer;
use crate::rendering::common::types::{Mesh, VertexBuffers};
use anyhow::Error;
use glam::Vec3;
use itertools::Itertools;
use log::warn;
use sargerust_files::adt::types::{
    MCALSubChunk, MCNKChunk, MCNKChunkHeader, MCNKHeaderFlags, MCNREntry, MTEXChunk, SMLayer, SMLayerFlags,
};
use sargerust_files::common::types::CImVector;
use sargerust_files::wdt::types::{MPHDChunk, MPHDFlags};

pub struct ADTImporter {}

fn calculate_normal(entry: &MCNREntry) -> Vec3 {
    // TODO: Comment from Nieriel suggests a special re-calculation of z from x and y:
    //  float Z = sqrt(1 - (X / 127)² - (Y / 127)²), Z >= 0)

    Vec3::new(
        entry.normal_x as f32 / 127.0f32,
        entry.normal_y as f32 / 127.0f32,
        entry.normal_z as f32 / 127.0f32,
    )
    .normalize()

    // Also, for some reason, the normals appear odd: For the shader, y seems to be up, not z.
}

/// Convert a 4-bit value to an 8-bit value by expanding the bits.
#[inline(always)]
fn expand_bits(data: u8) -> u8 {
    (data * 0x10) | (data & 0xF)
}

#[inline(always)]
fn unpack_2048_bytes(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(4096);
    assert_eq!(data.len(), 2048);

    for byte in data {
        result.push(expand_bits(byte & 0x0F));
        result.push(expand_bits(byte >> 4));
    }
    result
}

/// Transform game file structs into terrain texture layers that can be rendered. Ideally, this
/// would return unfailably, but the game files or our parsing don't seem to align.
fn transform_terrain_layer(
    layer: &SMLayer,
    mtex: &MTEXChunk,
    mcal: &MCALSubChunk,
    mphd: &MPHDChunk,
    mcnk: &MCNKChunkHeader,
) -> Option<TerrainTextureLayer> {
    let file_name = mtex
        .filenames
        .get(layer.textureId as usize)
        .map(|tex| tex.to_string());

    if file_name.is_none() {
        warn!("Texture ID {} not found in MTEX chunk.", layer.textureId);
        return None;
    }

    let texture_path = file_name.unwrap();
    let offset = layer.offset_in_mcal as usize;
    let alpha_map_buf: Vec<u8>;

    if layer.flags.contains(SMLayerFlags::ALPHA_MAP_COMPRESSED) {
        warn!("Alpha map compression not supported."); // TODO
        return None;
    } else if !mphd.flags.contains(MPHDFlags::ADT_HAS_BIG_ALPHA) {
        if mcnk.flags.contains(MCNKHeaderFlags::DO_NOT_FIX_ALPHA_MAP) {
            warn!("Alpha map fixing not supported."); // TODO
            return None;
        }

        if mcal.len() - offset < 2048 {
            warn!(
                "Texture ID {} has an alpha map that is too short. ({} Bytes instead of 2048)",
                layer.textureId,
                mcal.len() - offset
            );
            return None;
        }

        alpha_map_buf = unpack_2048_bytes(&mcal[layer.offset_in_mcal as usize..layer.offset_in_mcal as usize + 2048]);
    } else {
        if mcal.len() - offset < 4096 {
            warn!(
                "Texture ID {} has an alpha map that is too short. ({} Bytes instead of 4096)",
                layer.textureId,
                mcal.len() - offset
            );
            return None;
        }

        alpha_map_buf = mcal[offset..offset + 4096].to_vec();
    }

    Some(TerrainTextureLayer {
        texture_path,
        alpha_map: Some(alpha_map_buf),
    })
}

impl ADTImporter {
    pub fn create_mesh(
        mcnk: &MCNKChunk,
        low_res: bool,
        mtex: &MTEXChunk,
        mphd: &MPHDChunk,
    ) -> Result<(Vec3, Mesh, Vec<TerrainTextureLayer>), Error> {
        let mut index_buffer = Vec::<u32>::new();
        let mut position_buffer = Vec::new();
        let mut vertex_color_0 = Vec::new();
        let mut normals_buffer = Vec::new();

        let mcvt = mcnk.get_mcvt()?.unwrap();
        let mcnr = mcnk.get_mcnr()?;
        let mcly_opt = mcnk.get_mcly()?;
        let mcal_opt = mcnk.get_mcal()?;

        let texture_references = mcly_opt
            .and_then(|mcly| {
                // TODO: We may need to rewrite this completely into an iterator again, because we only need MCAL if we have more than one layer?
                mcal_opt.map(|mcal| {
                    mcly.iter()
                        .flat_map(|layer| transform_terrain_layer(layer, mtex, &mcal, mphd, &mcnk.header))
                        .collect_vec()
                })
            })
            .unwrap_or(vec![]);

        let use_vertex_color: bool = true; // In theory with this flag we can turn it off for debug purposes.
        let mccv = mcnk.get_mccv(&mcnk.header)?.filter(|_| use_vertex_color); // smchunk flag has_mccv.

        // Here we're in ADT Terrain space, that is +x -> north, +y -> west. Thus rows grow in -x, columns go to -y.
        // index of 9x9: 17 * row + column
        // index of high detail 8x8: 17 * row + column + 9
        for row in 0..9 {
            for column in 0..9 {
                let low = MCNKChunk::get_index_low(row, column);
                let height = mcvt[low as usize];

                position_buffer.push(Vec3::new(
                    -GRID_SIZE * row as f32,
                    -GRID_SIZE * column as f32,
                    height,
                ));

                // TODO: Can we avoid filling a vertex color buffer entirely, if not supported by the chunk?
                let color = mccv
                    .as_ref()
                    .map(|x| x[low as usize])
                    .unwrap_or(CImVector::from(0xFFFFFFFFu32));
                vertex_color_0.push([color.r, color.g, color.b, color.a]); // TODO: Where is the format defined?

                if let Some(normal) = mcnr.as_ref().map(|x| calculate_normal(&x[low as usize])) {
                    normals_buffer.push(Self::fixup_normal(normal));
                }
            }

            if row == 8 {
                continue;
            }

            for column in 0..8 {
                let high = MCNKChunk::get_index_high(row, column);
                let height = mcvt[high as usize];

                position_buffer.push(Vec3::new(
                    -GRID_SIZE * (row as f32 + 0.5),
                    -GRID_SIZE * (column as f32 + 0.5),
                    height,
                ));

                // TODO: Can we avoid filling a vertex color buffer entirely, if not supported by the chunk?
                let color = mccv
                    .as_ref()
                    .map(|x| x[high as usize])
                    .unwrap_or(CImVector::from(0xFFFFFFFFu32));
                vertex_color_0.push([color.r, color.g, color.b, color.a]); // TODO: Where is the format defined?

                if let Some(normal) = mcnr.as_ref().map(|x| calculate_normal(&x[high as usize])) {
                    normals_buffer.push(Self::fixup_normal(normal));
                }
            }
        }

        // build the index buffer, this is probably the most difficult part.
        // TODO: technically, this could be multiple index buffers and swapping them
        // TODO: apparently this is exactly the wrong index buffer winding order. Fix it here instead of the lazy way further down.

        if low_res {
            for row in 0..8 {
                // last row won't work.
                for column in 0..8 {
                    // tri 1
                    index_buffer.push(MCNKChunk::get_index_low(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);

                    // tri 2
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);
                }
            }
        } else {
            for row in 0..8 {
                // last row won't work.
                for column in 0..8 {
                    // W
                    index_buffer.push(MCNKChunk::get_index_low(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);

                    // N
                    index_buffer.push(MCNKChunk::get_index_low(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);

                    // E
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);

                    // S
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column + 1) as u32);
                }
            }
        }

        // TODO: as outlined above, just fix the statements instead.
        assert_eq!(index_buffer.len() % 3, 0);
        for chunk in index_buffer.chunks_exact_mut(3) {
            chunk.swap(1, 2);
        }

        let mesh = Mesh {
            vertex_buffers: VertexBuffers {
                position_buffer,
                vertex_color_0,
                normals_buffer,
                ..VertexBuffers::default()
            },
            index_buffer,
        };
        let pos = Vec3::new(
            mcnk.header.position.x,
            mcnk.header.position.y,
            mcnk.header.position.z,
        );

        Ok((pos, mesh, texture_references))
    }

    #[inline]
    fn fixup_normal(normal: Vec3) -> Vec3 {
        Vec3::new(normal.x, normal.z, normal.y)
    }
}
