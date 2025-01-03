// For now, we do read everything (since it has to be sequential), but we may not expose it in a meaningful data structure yet.
#![allow(unused_variables)]
// we use the exact wording from wowdev.wiki
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
use crate::ParserError;
use crate::common::reader::Parseable;
use crate::common::types::CAaBox;
use crate::m2::types::{
    FOURCC_M2HEADER, FOURCC_M2SKIN, M2Array, M2Asset, M2SkinProfile, M2Texture, M2TextureInternal, M2Vertex, Version,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::ffi::CString;
use std::io::{Read, Seek, SeekFrom};

pub struct M2Reader {}

impl M2Reader {
    pub(crate) fn read_array<R: Read>(rdr: &mut R) -> Result<M2Array, ParserError> {
        M2Array::parse(rdr)
    }

    fn read_global_flags<R: Read>(rdr: &mut R) -> Result<(), ParserError> {
        let flags = rdr.read_u32::<LittleEndian>()?;
        // flag_tilt_x, flag_tilt_y, unk1
        // if TBC: flag_use_texture_combiner_combos, unk2
        Ok(())
    }

    pub fn parse_asset<R: Read + Seek>(rdr: &mut R) -> Result<M2Asset, ParserError> {
        let magic = rdr.read_u32::<LittleEndian>()?;
        if magic != FOURCC_M2HEADER {
            return Err(ParserError::InvalidMagicValue { magic });
        }

        let version: Version = Version::parse(rdr)?;
        if version.major != 1 {
            return Err(ParserError::FormatError {
                reason: "M2Version.major MUST BE 1",
            });
        }

        #[cfg(feature = "wotlk")]
        if version.minor != 8 {
            return Err(ParserError::FormatError {
                reason: "M2Version.minor MUST BE 8 for WotLK",
            });
        }

        #[cfg(not(feature = "wotlk"))]
        panic!("Implement M2 Version checking for other versions");

        let name_array = M2Reader::read_array(rdr)?;
        _ = M2Reader::read_global_flags(rdr)?;
        let global_loops = M2Reader::read_array(rdr)?;
        let sequences = M2Reader::read_array(rdr)?;
        let sequenceIdxHashById = M2Reader::read_array(rdr)?;
        #[cfg(not(feature = "wotlk"))] // <= TBC
        let playable_animation_lookup = M2Reader::read_array(rdr)?;
        let bones = M2Reader::read_array(rdr)?;
        let boneIndicesById = M2Reader::read_array(rdr)?;
        let vertices = M2Reader::read_array(rdr)?;
        #[cfg(not(feature = "wotlk"))] // <= TBC
        let skin_profiles = M2Reader::read_array(rdr)?;
        #[cfg(feature = "wotlk")] // > TBC
        let num_skin_profiles = rdr.read_u32::<LittleEndian>()?; // Skin Profiles are now in .skin files
        let colors = M2Reader::read_array(rdr)?;
        let textures = M2Reader::read_array(rdr)?;
        let texture_weights = M2Reader::read_array(rdr)?;
        #[cfg(not(feature = "wotlk"))] // <= TBC
        let texture_flipbooks = M2Reader::read_array(rdr)?;
        let texture_transforms = M2Reader::read_array(rdr)?;
        let textureIndicesById = M2Reader::read_array(rdr)?;
        let materials = M2Reader::read_array(rdr)?;
        let boneCombos = M2Reader::read_array(rdr)?;
        let textureCombos = M2Reader::read_array(rdr)?;
        let textureCoordCombos = M2Reader::read_array(rdr)?;
        let textureWeightCombos = M2Reader::read_array(rdr)?;
        let textureTransformCombos = M2Reader::read_array(rdr)?;
        let bounding_box = CAaBox::parse(rdr)?;
        let bounding_sphere_radius = rdr.read_f32::<LittleEndian>()?;
        let collision_box = CAaBox::parse(rdr)?;
        let collision_sphere_radius = rdr.read_f32::<LittleEndian>()?;
        let collisionIndices = M2Reader::read_array(rdr)?;
        let collisionPositions = M2Reader::read_array(rdr)?;
        let collisionFaceNormals = M2Reader::read_array(rdr)?;
        let attachments = M2Reader::read_array(rdr)?;
        let attachmentIndicesById = M2Reader::read_array(rdr)?;
        let events = M2Reader::read_array(rdr)?;
        let lights = M2Reader::read_array(rdr)?;
        let cameras = M2Reader::read_array(rdr)?;
        let cameraIndicesById = M2Reader::read_array(rdr)?;
        let ribbon_emitters = M2Reader::read_array(rdr)?;
        let particle_emitters = M2Reader::read_array(rdr)?;

        // #[cfg(any(feature = "wotlk", feature = "tbc"))] // >= TBC
        // if flag_use_texture_combine_combos {
        //   let textureCombinerCombos = M2Reader::read_array(rdr)?;
        // }

        // Start resolving arrays
        let name = M2Reader::resolve_array_string(rdr, &name_array)?;
        let verts: Vec<M2Vertex> = M2Reader::resolve_array(rdr, &vertices)?;

        let texs: Vec<M2TextureInternal> = M2Reader::resolve_array(rdr, &textures)?;
        let textures: Vec<M2Texture> = texs
            .iter()
            .map(|tex| M2Texture {
                texture_type: tex.texture_type,
                texture_flags: tex.texture_flags,
                filename: M2Reader::resolve_array_string(rdr, &tex.filename).unwrap(),
            })
            .collect();

        Ok(M2Asset {
      magic,
      version,
      name,
      vertices: verts,
      #[cfg(feature = "wotlk")] // > TBC
      num_skin_profiles,
      textures
    })
    }

    pub fn parse_skin_profile<R: std::io::Read + std::io::Seek>(rdr: &mut R) -> Result<M2SkinProfile, ParserError> {
        let magic = rdr.read_u32::<LittleEndian>()?;

        #[cfg(feature = "wotlk")] // > TBC
        if magic != FOURCC_M2SKIN {
            return Err(ParserError::InvalidMagicValue { magic });
        }

        let vertices = M2Array::parse(rdr)?;
        let indices = M2Array::parse(rdr)?;
        M2Array::parse(rdr)?; // bones
        let submeshes = M2Array::parse(rdr)?;
        M2Array::parse(rdr)?; // batches
        let boneCountMax = rdr.read_u32::<LittleEndian>()?;

        Ok(M2SkinProfile {
            magic,
            vertices: M2Reader::resolve_array(rdr, &vertices)?,
            indices: M2Reader::resolve_array(rdr, &indices)?,
            submeshes: M2Reader::resolve_array(rdr, &submeshes)?,
            boneCountMax,
        })
    }

    fn resolve_array<T: Parseable<T>, R: Read + Seek>(rdr: &mut R, array: &M2Array) -> Result<Vec<T>, ParserError> {
        let size = array.size as usize;
        if size > 0 {
            rdr.seek(SeekFrom::Start(array.offset as u64))?;
        }

        let mut list: Vec<T> = Vec::with_capacity(size);
        for _ in 0..size {
            list.push(T::parse(rdr)?);
        }

        Ok(list)
    }

    pub(crate) fn resolve_array_string<R: Read + Seek>(rdr: &mut R, array: &M2Array) -> Result<String, ParserError> {
        let size = array.size as usize;
        if size == 0 {
            return Ok(String::new());
        }

        let mut buf: Vec<u8> = vec![0; size];
        rdr.seek(SeekFrom::Start(array.offset as u64))?;
        rdr.read_exact(&mut buf)?;

        return CString::from_vec_with_nul(buf)
            .map_err(|_| ParserError::FormatError {
                reason: "Cannot convert M2Array<char> to valid UTF-8",
            })
            .map(|str| str.into_string().unwrap());
    }
}
