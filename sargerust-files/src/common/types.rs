use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::ParserError;
use crate::common::reader::Parseable;

#[derive(Debug, Copy, Clone)]
pub struct C3Vector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct C2Vector {
    pub x: f32,
    pub y: f32,
}

// could also call this CBgra, but we keep consistency with WoWDevWiki
#[derive(Debug, Copy, Clone)]
pub struct CImVector {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

#[derive(Debug, Copy, Clone)]
pub struct CAaBox {
    pub min: C3Vector,
    pub max: C3Vector,
}

#[derive(Debug, Copy, Clone)]
pub struct CArgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Copy, Clone)]
pub struct C4Quaternion {
    /// https://wowdev.wiki/WMO#MODD_chunk
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Debug)]
pub(crate) struct IffChunk {
    pub magic: u32,
    pub size: u32,
    pub data: Vec<u8>,
}

impl IffChunk {
    pub fn magic_str(&self) -> String {
        std::str::from_utf8(&self.magic.to_be_bytes()[..])
            .unwrap_or_else(|_| panic!("Chunk Magic invalid utf8: {:X}", self.magic))
            .to_owned()
    }

    pub fn parse<T: Parseable<T>>(&self) -> Result<T, ParserError> {
        T::parse(&mut Cursor::new(&self.data))
    }

    pub fn read_next_chunk<R: Read>(rdr: &mut R) -> Result<IffChunk, ParserError> {
        let magic = rdr.read_u32::<LittleEndian>()?;
        let size = rdr.read_u32::<LittleEndian>()?;
        let mut data = vec![0; size as usize];
        rdr.read_exact(&mut data)?;

        Ok(IffChunk { magic, size, data })
    }

    pub fn is_magic(&self, magic: &str) -> bool {
        // The reason of doing it this way is that we've discovered some chunks that had an invalid
        // magic (not valid utf-8). This is probably rather a parser issue, but also that way we
        // do u32 comparisons instead of converting everything to strings and comparing those.
        self.magic == u32::from_be_bytes(magic.as_bytes().try_into().unwrap())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MVerChunk {
    pub version: u32,
}

impl Parseable<MVerChunk> for MVerChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MVerChunk, ParserError> {
        Ok(MVerChunk {
            version: rdr.read_u32::<LittleEndian>()?,
        })
    }
}
