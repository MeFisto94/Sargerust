#![allow(non_snake_case)] // we use the exact wording from wowdev.wiki
use crate::ParserError;
use crate::common::reader::Parseable;
use crate::common::types::{C3Vector, CAaBox, IffChunk};
use byteorder::{LittleEndian, ReadBytesExt};
use sargerust_files_derive_parseable::Parse;
use std::ffi::CStr;
use std::io::Read;

// https://wowdev.wiki/WDT

pub struct WDTAsset {
    pub mphd: MPHDChunk,
    pub main: MainChunk,
    pub modf: Option<SMMapObjDef>,
    pub mwmo: Option<MWMOChunk>,
}

#[derive(Debug, Copy, Clone)]
/// Also known as SMMapHeader
pub struct MPHDChunk {
    pub flags: u32,
    pub something: u32,
    pub unused: [u32; 6],
}

impl Parseable<MPHDChunk> for MPHDChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MPHDChunk, ParserError> {
        let flags = rdr.read_u32::<LittleEndian>()?;
        let something = rdr.read_u32::<LittleEndian>()?;

        let mut unused: [u32; 6] = [0; 6];
        for i in 0..6 {
            unused[i] = rdr.read_u32::<LittleEndian>()?;
        }

        Ok(MPHDChunk {
            flags,
            something,
            unused,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MainChunk {
    pub map_area_info: [SMAreaInfo; 64 * 64],
}

#[derive(Debug, Copy, Clone, Default)]
pub struct SMAreaInfo {
    // versioning is unclear, according to https://wowdev.wiki/WDT#MAIN_chunk
    // pub offset: u32,
    // pub size: u32,

    // apparently we either have flags or offset/size combos
    pub flags: u32, // potentially HasADT, potentially Loaded.
    pub asyncId: u32,
}

impl Parseable<MainChunk> for MainChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MainChunk, ParserError> {
        let mut chunk: MainChunk = MainChunk {
            map_area_info: [SMAreaInfo::default(); 4096],
        };

        for x in 0..4096 {
            chunk.map_area_info[x] = SMAreaInfo {
                // offset: rdr.read_u32::<LittleEndian>()?,
                // size: rdr.read_u32::<LittleEndian>()?,
                flags: rdr.read_u32::<LittleEndian>()?,
                asyncId: rdr.read_u32::<LittleEndian>()?,
            };
        }

        Ok(chunk)
    }
}

#[derive(Debug)]
pub struct MWMOChunk {
    pub filename: String,
}

impl From<&IffChunk> for MWMOChunk {
    fn from(value: &IffChunk) -> Self {
        let size = value.size as usize;
        if size == 0 {
            return MWMOChunk {
                filename: String::new(),
            };
        }

        MWMOChunk {
            filename: CStr::from_bytes_with_nul(&value.data)
                .map_err(|_| ParserError::FormatError {
                    reason: "Cannot convert MWMOChunk to valid UTF-8",
                })
                .map(|str| str.to_str().unwrap().into())
                .unwrap(),
        }
    }
}

#[derive(Debug, Copy, Clone, Parse)]
pub struct SMMapObjDef {
    pub nameId: u32,
    pub uniqueId: u32,
    pub pos: C3Vector,
    pub rot: C3Vector,
    pub extents: CAaBox,
    pub flags: u16,
    pub doodadSet: u16,
    pub nameSet: u16,
    /// when in ADT, scale, in WDT potentially padding
    pub scale: u16,
}

impl WDTAsset {
    pub fn has_chunk(&self, chunk_x: u8, chunk_y: u8) -> bool {
        let range = 0..64;
        assert!(range.contains(&chunk_x));
        assert!(range.contains(&chunk_y));

        self.main.map_area_info[64usize * chunk_y as usize + chunk_x as usize].flags != 0
    }
}
