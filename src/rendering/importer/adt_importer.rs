use crate::rendering::common::coordinate_systems::GRID_SIZE;
use crate::rendering::common::types::{Mesh, VertexBuffers};
use anyhow::Error;
use glam::Vec3;
use sargerust_files::adt::types::{MCCVSubChunk, MCNKChunk, MCNREntry};
use sargerust_files::common::types::CImVector;

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
}

impl ADTImporter {
    pub fn create_mesh(mcnk: &MCNKChunk, low_res: bool) -> Result<(Vec3, Mesh), Error> {
        let mut index_buffer = Vec::<u32>::new();
        let mut position_buffer = Vec::new();
        let mut vertex_color_0 = Vec::new();
        let mut normals_buffer = Vec::new();
        let mcvt = mcnk.get_mcvt()?.unwrap();
        let mcnr = mcnk.get_mcnr()?;

        let use_vertex_color: bool = true; // In theory with this flag we can turn it off for debug purposes.
        // let mccv = mcnk.get_mccv()?.filter(|_| use_vertex_color); // smchunk flag has_mccv.
        let mccv: Option<MCCVSubChunk> = None; // TODO: For some reason, get_mccv encounters chunks with invalid utf-8??

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
                    normals_buffer.push(normal);
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
                    normals_buffer.push(normal);
                }
            }
        }

        // build the index buffer, this is probably the most difficult part.
        // TODO: technically, this could be multiple index buffers and swapping them
        // TODO: apparently this is exactly the wrong index buffer winding order. FIx it here insstead of the lazy way further down.

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
        Ok((pos, mesh))
    }
}
