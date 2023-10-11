use anyhow::Error;
use glam::{Vec3};
use sargerust_files::adt::types::{MCCVSubChunk, MCNKChunk};
use sargerust_files::common::types::{CImVector};
use crate::rendering::common::coordinate_systems::GRID_SIZE;
use crate::rendering::common::types::{Mesh, VertexBuffers};

pub struct ADTImporter {
}

impl ADTImporter {
    pub fn create_mesh(mcnk: &MCNKChunk) -> Result<(Vec3, Mesh), Error> {
        let mut index_buffer = Vec::<u32>::new();
        let mut position_buffer = Vec::new();
        let mut vertex_color_0 = Vec::new();
        let mcvt = mcnk.get_mcvt()?.unwrap();
        let use_vertex_color: bool = true; // In theory with this flag we can turn it off for debug purposes.
        //let mccv_opt = mcnk.get_mccv()?.filter(|_| use_vertex_color); // smchunk flag has_mccv.
        let mccv_opt: Option<MCCVSubChunk> = None; // TODO: Fixme

        // Here we're in ADT Terrain space, that is +x -> north, +y -> west. Thus rows grow in -x, columns go to -y.
        // index of 9x9: 17 * row + column
        // index of high detail 8x8: 17 * row + column + 9
        for row in 0..9 {
            for column in 0..9 {
                let low = MCNKChunk::get_index_low(row, column);
                let height = mcvt[low as usize];
                // TODO: implement MCCV the _rust_ way (can't unwrap in a loop)
                // let color = if &mccv_opt.is_some() { (mccv_opt.unwrap()[low as usize]) } else { CImVector::from(0x0000FFFFu32) };

                position_buffer.push(Vec3::new(-GRID_SIZE * row as f32, -GRID_SIZE * column as f32, height));
                let color = CImVector::from(0x0000FFFFu32);
                vertex_color_0.push([color.r, color.g, color.b, color.a]); // TODO: Where is the format defined?
            }

            if row == 8 {
                continue;
            }

            for column in 0..8 {
                let high = MCNKChunk::get_index_high(row, column);
                let height = mcvt[high as usize];

                // see above
                // let color = if use_vertex_color { (&mccv_opt.unwrap()[high as usize]).clone() } else { CImVector::from(0xFF0000FFu32) };

                position_buffer.push(Vec3::new(-GRID_SIZE * (row as f32 + 0.5), -GRID_SIZE * (column as f32 + 0.5), height));
                let color = CImVector::from(0xFF0000FFu32);
                vertex_color_0.push([color.r, color.g, color.b, color.a]); // TODO: Where is the format defined?
            }
        }

        // build the index buffer, this is probably the most difficult part.
        let low_res = false;
        // TODO: technically, this could be multiple index buffers and swapping them
        // TODO: apparently this is exactly the wrong index buffer winding order. FIx it here insstead of the lazy way further down.

        if low_res {
            for row in 0..8 { // last row won't work.
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
            for row in 0..8 { // last row won't work.
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
                ..VertexBuffers::default()
            },
            index_buffer
        };
        let pos = Vec3::new(mcnk.header.position.x, mcnk.header.position.y, mcnk.header.position.z);
        Ok((pos, mesh))
    }
}
