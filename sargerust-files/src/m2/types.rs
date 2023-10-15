#![allow(non_snake_case)] // we use the exact wording from wowdev.wiki
use crate::common::reader::Parseable;
use crate::common::types::{C2Vector, C3Vector};
use crate::m2::reader::M2Reader;
use crate::ParserError;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Read, Write};

pub const FOURCC_M2HEADER: u32 = u32::from_le_bytes(*b"MD20");

#[cfg(feature = "wotlk")] // >= WOTLK
pub const FOURCC_M2SKIN: u32 = u32::from_le_bytes(*b"SKIN");

#[repr(C, packed)]
#[derive(Debug)]
pub(crate) struct M2Array {
    pub size: u32,
    pub offset: u32, // relative to the chunk (legion+?) or the start of file.
}

impl Parseable<M2Array> for M2Array {
    fn parse<R: Read>(rdr: &mut R) -> Result<M2Array, ParserError> {
        Ok(M2Array {
            size: rdr.read_u32::<LittleEndian>()?,
            offset: rdr.read_u32::<LittleEndian>()?,
        })
    }
}

#[allow(dead_code)] // At some point we will use it, bounding boxes etc.
#[repr(C, packed)]
struct M2Range {
    minimum: u32,
    maximum: u32,
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct Version {
    pub major: u8, // always 1
    pub minor: u8, // classic: [0, 1], tbc: [4, 7], wotlk: 8
}

impl Parseable<Version> for Version {
    fn parse<R: Read>(rdr: &mut R) -> Result<Version, ParserError> {
        let array = rdr.read_u32::<LittleEndian>()?.to_le_bytes();
        Ok(Version {
            minor: array[0],
            major: array[1],
        })
    }
}

#[derive(Debug)]
pub struct M2Asset {
    pub magic: u32,
    pub version: Version,
    pub name: String,
    // TODO: incomplete.
    pub vertices: Vec<M2Vertex>,
    #[cfg(not(feature = "wotlk"))] // <= TBC
    pub skin_profiles: Vec<M2SkinProfile>,
    #[cfg(feature = "wotlk")] // > TBC
    pub num_skin_profiles: u32,
    pub textures: Vec<M2Texture>,
}

impl M2Asset {
    pub fn dump_to_wavefront_obj<W: Write>(&self, w: &mut W, skin: &M2SkinProfile) -> Result<(), ParserError> {
        write!(w, "o {}\n", &self.name)?;
        // g for groups/submeshes.
        for v in &skin.vertices {
            let vert = &self.vertices[*v as usize];
            write!(w, "v {} {} {}\n", vert.pos.x, vert.pos.y, vert.pos.z)?;
            write!(
                w,
                "vn {} {} {}\n",
                vert.normal.x, vert.normal.y, vert.normal.z
            )?;
            write!(w, "vt {} {}\n", vert.tex_coords[0].x, vert.tex_coords[0].y)?;
        }
        for i in skin.indices.chunks_exact(3) {
            // NO: // change winding order from right handed to left handed.
            // indexes are 1-based and here we specify the same vert index for: vert, normal and texcoord.
            write!(w, "f {}/{}/{} ", i[0] + 1, i[0] + 1, i[0] + 1)?;
            write!(w, "{}/{}/{} ", i[1] + 1, i[1] + 1, i[1] + 1)?;
            write!(w, "{}/{}/{}\n", i[2] + 1, i[2] + 1, i[2] + 1)?;
        }

        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct M2Vertex {
    /// friendly reminder that WoW is right handed (Z Up)
    pub pos: C3Vector,
    pub bone_weights: [u8; 4],
    pub bone_indices: [u8; 4],
    pub normal: C3Vector,
    pub tex_coords: [C2Vector; 2],
}

impl Parseable<M2Vertex> for M2Vertex {
    fn parse<R: Read>(rdr: &mut R) -> Result<M2Vertex, ParserError> {
        Ok(M2Vertex {
            pos: C3Vector::parse(rdr)?,
            // We could try to read the arrays directly instead of this weird roundtrip
            bone_weights: rdr.read_u32::<LittleEndian>()?.to_le_bytes(),
            bone_indices: rdr.read_u32::<LittleEndian>()?.to_le_bytes(),
            normal: C3Vector::parse(rdr)?,
            tex_coords: [C2Vector::parse(rdr)?, C2Vector::parse(rdr)?],
        })
    }
}

#[derive(Debug)]
pub struct M2Texture {
    // TODO: better typing for type and flags.
    pub texture_type: u32,
    pub texture_flags: u32,
    pub filename: String,
}

pub(crate) struct M2TextureInternal {
    pub texture_type: u32,
    pub texture_flags: u32,
    pub filename: M2Array,
}

impl Parseable<M2TextureInternal> for M2TextureInternal {
    fn parse<R: Read>(rdr: &mut R) -> Result<M2TextureInternal, ParserError> {
        Ok(M2TextureInternal {
            texture_type: rdr.read_u32::<LittleEndian>()?,
            texture_flags: rdr.read_u32::<LittleEndian>()?,
            filename: M2Reader::read_array(rdr)?,
        })
    }
}

#[derive(Debug)]
pub struct M2SkinProfile {
    #[cfg(feature = "wotlk")] // >= WOTLK
    pub magic: u32, // on tbc, this is just inside the main m2 file.
    pub vertices: Vec<u16>,
    pub indices: Vec<u16>,
    // TODO: implement
    // pub bones: Vec<[u8; 4]>,
    pub submeshes: Vec<M2SkinSection>,
    // pub batches: Vec<M2Batch>,
    pub boneCountMax: u32,
}

#[derive(Debug)]
pub struct M2SkinSection {
    pub skinSectionId: u16,  // Mesh part ID
    pub Level: u16, // (level << 16) is added (|ed) to startTriangle and alike to avoid having to increase those fields to uint32s.
    pub vertexStart: u16, // Starting vertex number.
    pub vertexCount: u16, // Number of vertices.
    pub indexStart: u16, // Starting triangle index (that's 3* the number of triangles drawn so far).
    pub indexCount: u16, // Number of triangle indices.
    pub boneCount: u16, // Number of elements in the bone lookup table. Max seems to be 256 in Wrath. Shall be ≠ 0.
    pub boneComboIndex: u16, // Starting index in the bone lookup table
    // <= 4
    // from <=BC documentation: Highest number of bones needed at one time in this Submesh --Tinyn (wowdev.org)
    // In 2.x this is the amount of of bones up the parent-chain affecting the submesh --NaK
    // Highest number of bones referenced by a vertex of this submesh. 3.3.5a and suspectedly all other client revisions. -- Skarn
    pub boneInfluences: u16,
    pub centerBoneIndex: u16,
    pub centerPosition: C3Vector, // Average position of all the vertices in the sub mesh.
    #[cfg(any(feature = "wotlk", feature = "tbc"))] // >= TBC
    pub sortCenterPosition: C3Vector, // The center of the box when an axis aligned box is built around the vertices in the submesh.
    #[cfg(any(feature = "wotlk", feature = "tbc"))] // >= TBC
    pub sortRadius: f32,
}

impl Parseable<M2SkinSection> for M2SkinSection {
    fn parse<R: Read>(rdr: &mut R) -> Result<M2SkinSection, ParserError> {
        Ok(M2SkinSection {
      skinSectionId: rdr.read_u16::<LittleEndian>()?,
      Level: rdr.read_u16::<LittleEndian>()?,
      vertexStart: rdr.read_u16::<LittleEndian>()?,
      vertexCount: rdr.read_u16::<LittleEndian>()?,
      indexStart: rdr.read_u16::<LittleEndian>()?,
      indexCount: rdr.read_u16::<LittleEndian>()?,
      boneCount: rdr.read_u16::<LittleEndian>()?,
      boneComboIndex: rdr.read_u16::<LittleEndian>()?,
      boneInfluences: rdr.read_u16::<LittleEndian>()?,
      centerBoneIndex: rdr.read_u16::<LittleEndian>()?,
      centerPosition: C3Vector::parse(rdr)?,
      #[cfg(any(feature = "wotlk", feature = "tbc"))] // >= TBC
      sortCenterPosition: C3Vector::parse(rdr)?,
      #[cfg(any(feature = "wotlk", feature = "tbc"))] // >= TBC
      sortRadius: rdr.read_f32::<LittleEndian>()?,
    })
    }
}
