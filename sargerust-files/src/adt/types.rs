// we use the exact wording from wowdev.wiki
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use crate::ParserError;
use crate::common::reader::{GenericStringList, Parseable, read_chunk_array};
use crate::common::types::{C3Vector, CImVector, IffChunk};
use crate::wdt::types::SMMapObjDef;
use bitflags::bitflags;
use sargerust_files_derive_parseable::Parse;
use std::collections::HashMap;
use std::io::{Cursor, Read};
// https://wowdev.wiki/ADT/v18

#[derive(Debug)]
pub struct ADTAsset {
    pub mhdr: MHDRChunk,
    pub mcin: MCINChunk,
    pub mtex: MTEXChunk,
    pub mmdx: MMDXChunk,
    pub mmid: MMIDChunk,
    pub mwmo: MWMOChunk,
    pub mwid: MWIDChunk,
    pub mddf: MDDFChunk,
    pub modf: MODFChunk,
    pub mh2o: Option<MH2OChunk>,
    pub mcnks: Vec<MCNKChunk>,
}

#[derive(Debug, Parse)]
pub struct MHDRChunk {
    pub flags: u32,
    // from here on, offsets into chunks, allegedly the game uses only those as pointers
    // contrary to reading the chunk headers like we do
    pub mcin: u32,
    pub mtex: u32,
    pub mmdx: u32,
    pub mmid: u32,
    pub mwmo: u32,
    pub mwid: u32,
    pub mddf: u32,
    pub modf: u32,
    pub mfbo: u32,
    pub mh2o: u32,
    pub mtxf: u32,
    pub unused_1: u32,
    pub unused_2: u32,
    pub unused_3: u32,
    pub unused_4: u32,
}

#[derive(Debug, Parse)]
pub struct SMChunkInfo {
    pub offset: u32, // _absolute_ offset
    pub size: u32,
    pub flags: u32,
    pub pad: u32, // client RAM use only. Called asyncId there.
}

#[derive(Debug)]
pub struct MCINChunk {
    pub chunk_info: Vec<SMChunkInfo>, // 16 * 16 elements
}

impl Parseable<MCINChunk> for MCINChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MCINChunk, ParserError> {
        Ok(MCINChunk {
            chunk_info: Vec::<SMChunkInfo>::parse(rdr)?,
        })
    }
}

#[derive(Debug)]
pub struct MTEXChunk {
    pub filenames: Vec<String>,
}

impl Parseable<MTEXChunk> for MTEXChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MTEXChunk, ParserError> {
        Ok(MTEXChunk {
            filenames: GenericStringList::parse(rdr)?.string_list,
        })
    }
}

#[derive(Debug)]
pub struct MMDXChunk {
    pub filenames: Vec<String>,
    pub offsets: HashMap<u32, usize>,
}

impl Parseable<MMDXChunk> for MMDXChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MMDXChunk, ParserError> {
        let gsl = GenericStringList::parse(rdr)?;
        Ok(MMDXChunk {
            filenames: gsl.string_list,
            offsets: gsl.offset_to_string_list_offset,
        })
    }
}

#[derive(Debug)]
pub struct MMIDChunk {
    pub mmdx_offsets: Vec<u32>,
}

impl Parseable<MMIDChunk> for MMIDChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MMIDChunk, ParserError> {
        Ok(MMIDChunk {
            mmdx_offsets: read_chunk_array(rdr)?,
        })
    }
}

// as opposed to in WDT, this seems to be an array
#[derive(Debug)]
pub struct MWMOChunk {
    pub filenames: Vec<String>,
    pub offsets: HashMap<u32, usize>,
}

impl Parseable<MWMOChunk> for MWMOChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MWMOChunk, ParserError> {
        let gsl = GenericStringList::parse(rdr)?;
        Ok(MWMOChunk {
            filenames: gsl.string_list,
            offsets: gsl.offset_to_string_list_offset,
        })
    }
}

#[derive(Debug)]
pub struct MWIDChunk {
    pub mwmo_offsets: Vec<u32>,
}

impl Parseable<MWIDChunk> for MWIDChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MWIDChunk, ParserError> {
        Ok(MWIDChunk {
            mwmo_offsets: read_chunk_array(rdr)?,
        })
    }
}

#[derive(Debug, Parse)]
pub struct SMDoodadDef {
    pub nameId: u32, // MMID entry on which model to use
    pub uniqueId: u32,
    pub position: C3Vector, // even that is relative to the corner of a map
    pub rotation: C3Vector, // degrees??
    pub scale: u16,         // 1024 is the default, scale 1.0
    pub flags: u16,
}

#[derive(Debug)]
pub struct MDDFChunk {
    pub doodadDefs: Vec<SMDoodadDef>,
}

impl Parseable<MDDFChunk> for MDDFChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MDDFChunk, ParserError> {
        Ok(MDDFChunk {
            doodadDefs: Vec::<SMDoodadDef>::parse(rdr)?,
        })
    }
}

#[derive(Debug)]
pub struct MODFChunk {
    pub mapObjDefs: Vec<SMMapObjDef>,
}

impl Parseable<MODFChunk> for MODFChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MODFChunk, ParserError> {
        Ok(MODFChunk {
            mapObjDefs: Vec::<SMMapObjDef>::parse(rdr)?,
        })
    }
}

#[cfg(feature = "wotlk")]
#[derive(Debug, Parse)]
pub struct SMLiquidChunk {
    pub offset_instances: u32,  // SMLiquidInstance[layer_count] offset
    pub layer_count: u32,       // 0 if the chunk has no liquids, otherwise > 1, and then the other values become valid
    pub offset_attributes: u32, // points to mh2o_chunk_attributes
}

#[cfg(feature = "wotlk")]
#[derive(Debug, Parse)]
pub struct mh2o_chunk_attributes {
    pub fishable: u64, // 8x8 bit mask. Used for visibility?
    pub deep: u64,     // Fatigue Area
}

#[cfg(feature = "wotlk")]
#[derive(Debug, Parse)]
pub struct SMLiquidInstance {
    pub liquid_type: u16,          // foreign_key<uint16_t, &LiquidTypeRec::m_ID>
    pub liquid_vertex_format: u16, // This is gone after wrath for a database lookup.
    pub min_height_level: f32,
    pub max_height_leve: f32,
    pub x_offset: u8, // [0, 7]
    pub y_offset: u8, // [0, 7]
    pub width: u8,    // [1, 8]
    pub height: u8,   // [1, 8]
    pub offset_exists_bitmap: u32,
    pub offset_vertex_data: u32,
}

#[cfg(feature = "wotlk")]
#[derive(Debug)]
/// https://wowdev.wiki/ADT/v18#MH2O_chunk_(WotLK+) have fun
pub struct MH2OChunk {
    pub chunks: Vec<SMLiquidChunk>, // 16x16 = 256 entries.
}

impl Parseable<MH2OChunk> for MH2OChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MH2OChunk, ParserError> {
        Ok(MH2OChunk {
            chunks: read_chunk_array(rdr)?,
        })
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct MCNKHeaderFlags: u32 {
        const HAS_MCSH = 1 << 0;
        const IMPASS = 1 << 1;
        const LQ_RIVER = 1 << 2;
        const LQ_OCEAN = 1 << 3;
        const LQ_MAGMA = 1 << 4;
        const LQ_SLIME = 1 << 5;
        const HAS_MCCV = 1 << 6;
        const DO_NOT_FIX_ALPHA_MAP = 1 << 14;
        const HIGH_RES_HOLES = 1 << 15;
    }
}

impl Parseable<MCNKHeaderFlags> for MCNKHeaderFlags {
    fn parse<R: Read>(rdr: &mut R) -> Result<MCNKHeaderFlags, ParserError> {
        Ok(MCNKHeaderFlags::from_bits_retain(u32::parse(rdr)?))
    }
}

// 256 individual MCNK chunks, row by row, starting from top-left (northwest).
// The MCNK chunks have a large block of data that starts with a header, and then has sub-chunks of its own.
#[derive(Debug, Parse)]
/// SMChunk
pub struct MCNKChunkHeader {
    pub flags: MCNKHeaderFlags,
    pub IndexX: u32,
    pub IndexY: u32,
    pub nLayers: u32,
    pub nDoodadRefs: u32,
    pub ofsHeight: u32,
    pub ofsNormal: u32,
    pub ofsLayer: u32,
    pub ofsRefs: u32,
    pub ofsAlpha: u32,
    pub sizeAlpha: u32,
    pub ofsShadow: u32,
    pub sizeShadow: u32,
    pub areaId: u32,
    pub nMapObjRefs: u32,
    pub holes_low_res: u16,
    pub unknown_but_used: u16,
    pub ReallyLowQualityTextureingMap: u128, // 8x8 2bit map.
    pub noEffectDoodad: u64,
    pub ofsSndEmitters: u32,
    pub nSndEmitters: u32,
    pub ofsLiquid: u32,
    pub sizeLiquid: u32,
    pub position: C3Vector,
    pub ofsMCCV: u32,
    /// unused until cata.
    pub ofsMCLV: u32,
    pub unused: u32,
}

#[derive(Debug)]
pub struct MCNKChunk {
    pub header: MCNKChunkHeader,
    pub(crate) sub_chunks: Vec<u8>,
}

impl Parseable<MCNKChunk> for MCNKChunk {
    fn parse<R: Read>(rdr: &mut R) -> Result<MCNKChunk, ParserError> {
        let header = MCNKChunkHeader::parse(rdr)?;

        // Sadly, the sub-chunks are not laid out consecutively, so we have to read them as one and
        // reinterpret them using the offsets of the header on demand.

        let mut sub_chunks = Vec::<u8>::new();
        rdr.read_to_end(&mut sub_chunks)?;

        Ok(MCNKChunk { header, sub_chunks })
    }
}

impl MCNKChunk {
    pub fn get_mcvt(&self) -> Result<Option<MCVTSubChunk>, ParserError> {
        if self.header.ofsHeight == 0 {
            return Ok(None);
        }

        let mut rdr = Cursor::new(&self.sub_chunks[(self.header.ofsHeight - 136) as usize..]);
        let iff = IffChunk::read_next_chunk(&mut rdr)?;

        if !iff.is_magic("MCVT") {
            return Err(ParserError::InvalidMagicValue { magic: iff.magic });
        }

        Ok(Some(read_chunk_array(&mut Cursor::new(&iff.data))?))
    }

    pub fn get_mccv(&self, mcnk: &MCNKChunkHeader) -> Result<Option<MCCVSubChunk>, ParserError> {
        if self.header.ofsMCCV == 0 || !mcnk.flags.contains(MCNKHeaderFlags::HAS_MCCV) {
            return Ok(None);
        }

        let mut rdr = Cursor::new(&self.sub_chunks[(self.header.ofsMCCV - 136) as usize..]);
        let iff = IffChunk::read_next_chunk(&mut rdr)?;

        if !iff.is_magic("MCCV") {
            return Err(ParserError::InvalidMagicValue { magic: iff.magic });
        }

        Ok(Some(read_chunk_array(&mut Cursor::new(&iff.data))?))
    }

    pub fn get_mcnr(&self) -> Result<Option<MCNRSubChunk>, ParserError> {
        if self.header.ofsNormal == 0 {
            return Ok(None);
        }

        let mut rdr = Cursor::new(&self.sub_chunks[(self.header.ofsNormal - 136) as usize..]);
        let iff = IffChunk::read_next_chunk(&mut rdr)?;

        if !iff.is_magic("MCNR") {
            return Err(ParserError::InvalidMagicValue { magic: iff.magic });
        }

        Ok(Some(read_chunk_array(&mut Cursor::new(&iff.data))?))
    }

    pub fn get_mcly(&self) -> Result<Option<MCLYSubChunk>, ParserError> {
        if self.header.ofsLayer == 0 {
            return Ok(None);
        }

        let mut rdr = Cursor::new(&self.sub_chunks[(self.header.ofsLayer - 136) as usize..]);
        let iff = IffChunk::read_next_chunk(&mut rdr)?;

        if !iff.is_magic("MCLY") {
            return Err(ParserError::InvalidMagicValue { magic: iff.magic });
        }

        Ok(Some(read_chunk_array(&mut Cursor::new(&iff.data))?))
    }

    pub fn get_mcal(&self) -> Result<Option<MCALSubChunk>, ParserError> {
        if self.header.ofsAlpha == 0 {
            return Ok(None);
        }

        let mut rdr = Cursor::new(&self.sub_chunks[(self.header.ofsAlpha - 136) as usize..]);
        let iff = IffChunk::read_next_chunk(&mut rdr)?;

        if !iff.is_magic("MCAL") {
            return Err(ParserError::InvalidMagicValue { magic: iff.magic });
        }

        Ok(Some(iff.data.clone()))
    }

    pub fn get_index_low(row: u8, column: u8) -> u8 {
        17 * row + column
    }

    pub fn get_index_high(row: u8, column: u8) -> u8 {
        17 * row + column + 9
    }
}

/// float height[9\*9 + 8\*8]
pub type MCVTSubChunk = Vec<f32>;

#[cfg(feature = "wotlk")]
/// chunk_lighting[9\*9 + 8\*8] bgra, vertex shading, 0x7F is 1.0, alpha irrelevant
pub type MCCVSubChunk = Vec<CImVector>;

// LK and before, this is more simple because it has no padding
#[derive(Debug, Parse)]
/// 127 = 1, -127 = -1, _not_ normalized but almost.
/// Nieriel recommends a recalculation of Z from X and Y, so that the vector is normalized.
pub struct MCNREntry {
    pub normal_x: i8,
    pub normal_z: i8,
    pub normal_y: i8,
}

pub type MCNRSubChunk = Vec<MCNREntry>;

bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct SMLayerFlags: u32 {
        // lowest 3 bits: animation rotation, next 3 bits: animation speed
        const ANIMATION_ENABLED = 1 << 6;
        const OVERBRIGHT = 1 << 7; // used for lava to make it glow
        const USE_ALPHA_MAP = 1 << 8; // should be set for every layer after the first.
        const ALPHA_MAP_COMPRESSED = 1 << 9;
        const USE_CUBE_MAP_REFLECTION = 1 << 10;
    }
}

impl Parseable<SMLayerFlags> for SMLayerFlags {
    fn parse<R: Read>(rdr: &mut R) -> Result<SMLayerFlags, ParserError> {
        let flags = u32::parse(rdr)?;
        Ok(SMLayerFlags::from_bits_retain(flags))
    }
}

#[derive(Debug, Parse)]
pub struct SMLayer {
    pub textureId: u32,
    pub flags: SMLayerFlags,
    pub offset_in_mcal: u32,
    pub effectId: u32, // foreign_key <uint32_t, &GroundEffectTextureRec::m_ID>
}

/// Up to 4 layers, apparently.
pub type MCLYSubChunk = Vec<SMLayer>;

/// uint32_t doodad_refs[header.nDoodadRefs]; // into MDDF
/// uint32_t object_refs[header.nMapObjRefs]; // into MODF
pub type MCRFSubChunk = Vec<u32>;

/// 64x64 bit shadow_map
pub type MCSHSubChunk = Vec<u64>;

/// alpha_map with 64x64 index, 4bit, 8bit or something completely else.
pub type MCALSubChunk = Vec<u8>;

#[cfg(not(feature = "wotlk"))] // <= TBC
compile_error!("MCLQ Sub Chunk: Not implemented");

#[cfg(not(feature = "wotlk"))] // <= TBC
#[derive(Debug, Parse)]
pub struct CWSoundEmitter {
    pub soundPointID: u32,
    pub soundNameID: u32,
    pub pos: C3Vector,
    pub minDistance: f32,
    pub maxDistance: f32,
    pub cutoffDistance: f32,
    pub startTime: u16,
    pub endTime: u16,
    pub mode: u16,
    pub groupSilenceMin: u16,
    pub groupSilenceMax: u16,
    pub playInstancesMin: u16,
    pub playInstancesMax: u16,
    pub loopCountMin: u8,
    pub loopCountMax: u8,
    pub interSoundGapMin: u16,
    pub interSoundGapMax: u16,
}

#[cfg(feature = "wotlk")] // > TBC
/// Apparently this is not well documented/researched
#[derive(Debug, Parse)]
pub struct CWSoundEmitter {
    pub entry_id: u32, // foreign_key<uint32_t, &SoundEntriesAdvancedRec::m_ID>
    pub position: C3Vector,
    pub size: C3Vector,
}

pub type MCSESubChunk = Vec<CWSoundEmitter>;

#[cfg(any(feature = "wotlk", feature = "tbc"))] // >= TBC
#[derive(Debug, Parse)]
pub struct MFBOSubChunk {
    // not implemented yet
}

#[cfg(feature = "wotlk")] // > TBC
/// SMTextureFlags
pub type MXTFSubChunk = u32;
