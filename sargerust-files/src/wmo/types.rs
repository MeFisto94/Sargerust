// we use the exact wording from wowdev.wiki
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::fs::read;
use std::io::ErrorKind::UnexpectedEof;
use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use num_enum::FromPrimitive;
use sargerust_files_derive_parseable::Parse;

use crate::common::reader::{GenericStringList, Parseable, read_chunk_array, read_cstring};
use crate::common::types::{C2Vector, C3Vector, C4Quaternion, CAaBox, CArgb, CImVector, MVerChunk};
use crate::ParserError;

// https://wowdev.wiki/WMO

#[derive(Debug)]
pub struct WMORootAsset {
  pub mver: MVerChunk,
  pub mohd: MOHDChunk,
  pub motx: MOTXChunk,
  pub momt: MOMTChunk,
  pub mogn: MOGNChunk,
  pub mogi: MOGIChunk,
  pub mosb: Option<MOSBChunk>,
// TODO: Portal chunks. MOPV; MOPT, MOPR.
  pub molt: MOLTChunk,
  pub mods: MODSChunk,
  pub modn: MODNChunk,
  pub modd: MODDChunk,
  pub mfog: MFOGChunk,
}

#[derive(Debug, Copy, Clone)]
/// Also known as SMOHeader
pub struct MOHDChunk {
  pub nTextures: u32,
  pub nGroups: u32,
  pub nPortals: u32,
  pub nLights: u32,
  pub nDoodadNames: u32,
  pub nDoodadDefs: u32,
  pub nDoodadSets: u32,
  pub ambColor: CArgb,
  pub wmoID: u32, // foreign_key<uint32_t, &WMOAreaTableRec::m_WMOID>
  pub bounding_box: CAaBox,
  pub flags: u16,
  pub numLod: u16, // could be as of legion only?? otherwise it's padding and zeroed anyway.
}

impl Parseable<MOHDChunk> for MOHDChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOHDChunk, ParserError> {
    Ok(MOHDChunk {
      nTextures: rdr.read_u32::<LittleEndian>()?,
      nGroups: rdr.read_u32::<LittleEndian>()?,
      nPortals: rdr.read_u32::<LittleEndian>()?,
      nLights: rdr.read_u32::<LittleEndian>()?,
      nDoodadNames: rdr.read_u32::<LittleEndian>()?,
      nDoodadDefs: rdr.read_u32::<LittleEndian>()?,
      nDoodadSets: rdr.read_u32::<LittleEndian>()?,
      ambColor: CArgb::parse(rdr)?,
      wmoID: rdr.read_u32::<LittleEndian>()?,
      bounding_box: CAaBox::parse(rdr)?,
      flags: rdr.read_u16::<LittleEndian>()?,
      numLod: rdr.read_u16::<LittleEndian>()?
    })
  }
}

#[derive(Debug)]
pub struct MOTXChunk {
  pub textureNameList: Vec<String>
}

impl Parseable<MOTXChunk> for MOTXChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOTXChunk, ParserError> {
    Ok(MOTXChunk {
      textureNameList: GenericStringList::parse(rdr)?.stringList
    })
  }
}

#[derive(Debug)]
pub struct SMOMaterial {
  pub flags: u32,
  pub shader: u32,
  pub blendMode: u32,
  pub texture_1: u32, // MOTX index.
  pub sidnColor: CImVector,
  pub frameSidnColor: CImVector,
  pub texture_2: u32,
  pub diffColor: CImVector,
  pub ground_type: u32, // foreign_keyⁱ<uint32_t, &TerrainTypeRec::m_ID>
  pub texture_3: u32,
  pub color_2: u32,
  pub flags_2: u32,
  pub runTimeData: [u32; 4] // nulled upon loading.
}

impl Parseable<SMOMaterial> for SMOMaterial {
  fn parse<R: Read>(rdr: &mut R) -> Result<SMOMaterial, ParserError> {
    let mut mat = SMOMaterial {
      flags: rdr.read_u32::<LittleEndian>()?,
      shader: rdr.read_u32::<LittleEndian>()?,
      blendMode: rdr.read_u32::<LittleEndian>()?,
      texture_1: rdr.read_u32::<LittleEndian>()?,
      sidnColor: CImVector::parse(rdr)?,
      frameSidnColor: CImVector::parse(rdr)?,
      texture_2: rdr.read_u32::<LittleEndian>()?,
      diffColor: CImVector::parse(rdr)?,
      ground_type: rdr.read_u32::<LittleEndian>()?,
      texture_3: rdr.read_u32::<LittleEndian>()?,
      color_2: rdr.read_u32::<LittleEndian>()?,
      flags_2: rdr.read_u32::<LittleEndian>()?,
      runTimeData: [0; 4]
    };

    for x in 0..4 {
      mat.runTimeData[x] = rdr.read_u32::<LittleEndian>()?;
    }

    Ok(mat)
  }
}

#[derive(Debug)]
pub struct MOMTChunk {
  pub materialList: Vec<SMOMaterial>
}

impl Parseable<MOMTChunk> for MOMTChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOMTChunk, ParserError> {
    Ok(MOMTChunk { materialList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug)]
pub struct MOGNChunk {
  pub groupNameList: Vec<String>
}

impl Parseable<MOGNChunk> for MOGNChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOGNChunk, ParserError> {
    Ok(MOGNChunk {
      groupNameList: GenericStringList::parse(rdr)?.stringList
    })
  }
}

#[derive(Debug)]
pub struct SMOGroupInfo {
  pub flags: u32,
  pub bounding_box: CAaBox,
  pub nameoffset: i32
}

impl Parseable<SMOGroupInfo> for SMOGroupInfo {
  fn parse<R: Read>(rdr: &mut R) -> Result<SMOGroupInfo, ParserError> {
    Ok(SMOGroupInfo {
      flags: rdr.read_u32::<LittleEndian>()?,
      bounding_box: CAaBox::parse(rdr)?,
      nameoffset: rdr.read_i32::<LittleEndian>()?,
    })
  }
}

#[derive(Debug)]
pub struct MOGIChunk {
  pub groupInfoList: Vec<SMOGroupInfo>
}

impl Parseable<MOGIChunk> for MOGIChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOGIChunk, ParserError> {
    Ok(MOGIChunk{ groupInfoList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug)]
pub struct MOSBChunk {
  pub skyboxName: String
}

impl Parseable<MOSBChunk> for MOSBChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOSBChunk, ParserError> {
    Ok(MOSBChunk{ skyboxName: read_cstring(rdr)?.into_string()? })
  }
}

// Portals


/*
  Light:
  https://wowdev.wiki/WMO#MOLT_chunk

  The entire MOLT and related chunks seem to be unused at least in 3.3.5a.
  Changing light colors and other settings on original WMOs leads to no effect.
  Removing the light leads to no effect either.
  I assume that MOLT rendering is disabled somewhere in the WoW.exe, as it might use the same principle
  as the M2 light emitters which are not properly supported up to WoD.
  However, when you explore the WMOs in 3D editors you can clearly see that MOCV layer is different under those lamps.
  So, I assume they are used for baking MOCV colors and also written to the actual file in case the renderer will ever get updated,
  or just because you can easily import the WMO back and rebake the colors.
*/
#[repr(u8)]
#[derive(FromPrimitive, Debug)]
pub enum SMOLightLightType {
  OMNI_LGT = 0,
  SPOT_LGT = 1,
  DIRECT_LGT = 2,
  AMBIENT_LGT = 3,
  #[default]
  UNKNOWN_LGT
}

#[derive(Debug)]
pub struct SMOLight {
  pub lightType: SMOLightLightType,
  pub useAtten: u8,
  pub padding_1: u8,
  pub padding_2: u8,
  pub color: CImVector,
  pub position: C3Vector,
  pub intensity: f32,
  pub unk1: C2Vector,
  pub unk2: C2Vector,
  pub attenStart: f32,
  pub attenEnd: f32
}

impl Parseable<SMOLight> for SMOLight {
  fn parse<R: Read>(rdr: &mut R) -> Result<SMOLight, ParserError> {
    Ok(SMOLight {
      lightType: rdr.read_u8()?.into(),
      useAtten: rdr.read_u8()?,
      padding_1: rdr.read_u8()?,
      padding_2: rdr.read_u8()?,
      color: CImVector::parse(rdr)?,
      position: C3Vector::parse(rdr)?,
      intensity: rdr.read_f32::<LittleEndian>()?,
      unk1: C2Vector::parse(rdr)?,
      unk2: C2Vector::parse(rdr)?,
      attenStart: rdr.read_f32::<LittleEndian>()?,
      attenEnd: rdr.read_f32::<LittleEndian>()?,
    })
  }
}

#[derive(Debug)]
pub struct MOLTChunk {
  pub lightList: Vec<SMOLight>
}

impl Parseable<MOLTChunk> for MOLTChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOLTChunk, ParserError> {
    Ok(MOLTChunk { lightList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug)]
pub struct SMODoodadSet {
  pub name: String,
  pub startIndex: u32,
  pub count: u32,
}

impl Parseable<SMODoodadSet> for SMODoodadSet {
  fn parse<R: Read>(rdr: &mut R) -> Result<SMODoodadSet, ParserError> {
    let mut name_buf = Vec::from([0u8; 0x14]);
    rdr.read_exact(&mut name_buf)?;
    let startIndex = rdr.read_u32::<LittleEndian>()?;
    let count = rdr.read_u32::<LittleEndian>()?;
    let _padding = rdr.read_u32::<LittleEndian>()?;

    Ok(SMODoodadSet {
      name: String::from_utf8(name_buf)?.trim_end_matches(char::from(0)).into(),
      startIndex,
      count
    })
  }
}

#[derive(Debug)]
pub struct MODSChunk {
  pub doodadSetList: Vec<SMODoodadSet>
}

impl Parseable<MODSChunk> for MODSChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MODSChunk, ParserError> {
    Ok(MODSChunk { doodadSetList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug)]
pub struct MODNChunk {
  pub doodadNameList: Vec<String>
}

impl Parseable<MODNChunk> for MODNChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MODNChunk, ParserError> {
    Ok(MODNChunk { doodadNameList: GenericStringList::parse(rdr)?.stringList })
  }
}

#[derive(Debug)]
/// https://wowdev.wiki/WMO#MODD_chunk
pub struct SMODoodadDef {
  pub nameIndex: u32, // actually, u24
  pub flags: u8,
  pub position: C3Vector,
  pub orientation: C4Quaternion,
  pub scale: f32,

  // (B,G,R,A) overrides pc_sunColor
  // when A is != 0xff && A < 255, A is a MOLT index and that's used instead the RGB given here, taking distance and intensity into account
  // If A > MOLT count, then MOLT[0] is used
  // If A == 255, the shading direction vector is based on the center of the group and not the sun direction vector, the look-at vector from group bounds center to doodad position
  pub color: CImVector
}

impl Parseable<SMODoodadDef> for SMODoodadDef {
  fn parse<R: Read>(rdr: &mut R) -> Result<SMODoodadDef, ParserError> {
    let nameAndFlags = rdr.read_u32::<LittleEndian>()?;

    Ok(SMODoodadDef {
      nameIndex: nameAndFlags & 0x00FFFFFF,
      flags: ((nameAndFlags & 0xFF000000) >> 24) as u8,
      position: C3Vector::parse(rdr)?,
      orientation: C4Quaternion::parse(rdr)?,
      scale: rdr.read_f32::<LittleEndian>()?,
      color: CImVector::parse(rdr)?
    })
  }
}

#[derive(Debug)]
pub struct MODDChunk {
  pub doodadDefList: Vec<SMODoodadDef>
}

impl Parseable<MODDChunk> for MODDChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MODDChunk, ParserError> {
    Ok(MODDChunk { doodadDefList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug)]
pub struct SMOFog {
  pub flags: u32,
  pub pos: C3Vector,
  pub smaller_radius: f32, // start
  pub larger_radius: f32, // end
  // this is badly mapped, compare https://wowdev.wiki/WMO#MFOG_chunk, but I didn't want to add that complexity.
  pub fog_end: f32,
  pub fog_start_scalar: f32,
  pub fog_color: CImVector,
  pub uwfog_end: f32,
  pub uwfog_start_scalar: f32,
  pub uwfog_color: CImVector,
}

impl Parseable<SMOFog> for SMOFog {
  fn parse<R: Read>(rdr: &mut R) -> Result<SMOFog, ParserError> {
    Ok(SMOFog {
      flags: rdr.read_u32::<LittleEndian>()?,
      pos: C3Vector::parse(rdr)?,
      smaller_radius: rdr.read_f32::<LittleEndian>()?,
      larger_radius: rdr.read_f32::<LittleEndian>()?,
      fog_end: rdr.read_f32::<LittleEndian>()?,
      fog_start_scalar: rdr.read_f32::<LittleEndian>()?,
      fog_color: CImVector::parse(rdr)?,
      uwfog_end: rdr.read_f32::<LittleEndian>()?,
      uwfog_start_scalar: rdr.read_f32::<LittleEndian>()?,
      uwfog_color: CImVector::parse(rdr)?,
    })
  }
}

#[derive(Debug)]
pub struct MFOGChunk {
  pub fogList: Vec<SMOFog>
}

impl Parseable<MFOGChunk> for MFOGChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MFOGChunk, ParserError> {
    Ok(MFOGChunk { fogList: read_chunk_array(rdr)? })
  }
}

// WMO group file

#[derive(Debug)]
pub struct WMOGroupAsset {
  pub mver: MVerChunk,
  pub mogp: MOGPChunk,
  pub mopy: MOPYChunk,
  pub movi: MOVIChunk,
  pub movt: MOVTChunk,
  pub monr: MONRChunk,
  pub motv: MOTVChunk,
  pub moba: MOBAChunk,
  pub molr: Option<MOLRChunk>,
  pub modr: Option<MODRChunk>,
  pub mobn: Option<MOBNChunk>,
  pub mobr: Option<MOBRChunk>,
  pub mocv: Option<MOCVChunk>,
}

#[derive(Debug)]
pub struct MOGPChunk {
  pub groupName: u32, // offset into MOGN
  pub descriptiveGroupName: u32, // offset into MOGN
  pub flags: u32,
  pub boundingBox: CAaBox,
  pub portalStart: u16, // index into MOPR
  pub portalCount: u16,
  pub transBatchCount: u16,
  pub intBatchCount: u16,
  pub extBatchCount: u16,
  pub padding_or_bach_type_d: u16,
  pub fogIds: [u8; 4], // MFOG ids
  pub groupLiquid: u32,
  pub uniqueID: u32, // foreign_keyⁱ<uint32_t, &WMOAreaTableRec::m_WMOGroupID>
  pub flags2: u32, // SMOGroupFlags2
  pub unused: u32, // after Shadow Lands, split groups.
}

impl Parseable<MOGPChunk> for MOGPChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOGPChunk, ParserError> {
    Ok(MOGPChunk {
      groupName: rdr.read_u32::<LittleEndian>()?,
      descriptiveGroupName: rdr.read_u32::<LittleEndian>()?,
      flags: rdr.read_u32::<LittleEndian>()?,
      boundingBox: CAaBox::parse(rdr)?,
      portalStart: rdr.read_u16::<LittleEndian>()?,
      portalCount: rdr.read_u16::<LittleEndian>()?,
      transBatchCount: rdr.read_u16::<LittleEndian>()?,
      intBatchCount: rdr.read_u16::<LittleEndian>()?,
      extBatchCount: rdr.read_u16::<LittleEndian>()?,
      padding_or_bach_type_d: rdr.read_u16::<LittleEndian>()?,
      fogIds: rdr.read_u32::<LittleEndian>()?.to_le_bytes(),
      groupLiquid: rdr.read_u32::<LittleEndian>()?,
      uniqueID: rdr.read_u32::<LittleEndian>()?,
      flags2: rdr.read_u32::<LittleEndian>()?,
      unused: rdr.read_u32::<LittleEndian>()?,
    })
  }
}

#[derive(Debug, Parse)]
pub struct SMOPoly {
  pub flags: u8, // TODO: kind of important
  pub material_id: u8 // index into MOMT, 0xFF for collision faces.
}

#[derive(Debug)]
pub struct MOPYChunk {
  pub polyList: Vec<SMOPoly>
}

impl Parseable<MOPYChunk> for MOPYChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOPYChunk, ParserError> {
    Ok(MOPYChunk { polyList: Vec::<SMOPoly>::parse(rdr)? })
  }
}

#[derive(Debug)]
pub struct MOVIChunk {
  pub indices: Vec<u16>
}

#[derive(Debug)]
pub struct MOVTChunk {
  pub vertexList: Vec<C3Vector>
}

#[derive(Debug)]
pub struct MONRChunk {
  pub normalList: Vec<C3Vector>
}

#[derive(Debug)]
pub struct MOTVChunk {
  pub textureVertexList: Vec<C2Vector>
}

impl Parseable<MOVIChunk> for MOVIChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOVIChunk, ParserError> {
    Ok(MOVIChunk { indices: read_chunk_array(rdr)? })
  }
}

impl Parseable<MOVTChunk> for MOVTChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOVTChunk, ParserError> {
    Ok(MOVTChunk { vertexList: read_chunk_array(rdr)? })
  }
}

impl Parseable<MONRChunk> for MONRChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MONRChunk, ParserError> {
    Ok(MONRChunk { normalList: read_chunk_array(rdr)? })
  }
}

impl Parseable<MOTVChunk> for MOTVChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOTVChunk, ParserError> {
    Ok(MOTVChunk { textureVertexList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug, Parse)]
pub struct SMOBatch {
  // bounding box for culling.
  pub bx: u16,
  pub by: u16,
  pub bz: u16,
  pub tx: u16,
  pub ty: u16,
  pub tz: u16,
  pub startIndex: u32, // first face index in MOVI
  pub count: u16, // nb of MOVI indices
  pub minIndex: u16, // index of the first vertex in MOVT
  pub maxIndex: u16, // index of the last vertex inclusively.
  pub flag_unknown: u8,
  pub material_id: u8, // Index in MOMT
}

#[derive(Debug)]
pub struct MOBAChunk {
  pub batchList: Vec<SMOBatch>
}

impl Parseable<MOBAChunk> for MOBAChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOBAChunk, ParserError> {
    Ok(MOBAChunk { batchList: Vec::<SMOBatch>::parse(rdr)? })
  }
}

#[derive(Debug)]
pub struct MOLRChunk {
  pub lightRefList: Vec<u16>
}

impl Parseable<MOLRChunk> for MOLRChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOLRChunk, ParserError> {
    Ok(MOLRChunk{ lightRefList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug)]
pub struct MODRChunk {
  pub doodadRefList: Vec<u16>
}

impl Parseable<MODRChunk> for MODRChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MODRChunk, ParserError> {
    Ok(MODRChunk{ doodadRefList: read_chunk_array(rdr)? })
  }
}

#[derive(Debug, Parse)]
pub struct CAaBspNode {
  pub flags: u16,
  pub negChild: i16,
  pub posChild: i16,
  pub nFaces: u16,
  pub faceStart: u32,
  pub planeDist: f32
}

pub type MOBNChunk = CAaBspNode;

#[derive(Debug)]
pub struct MOBRChunk {
  pub nodeFaceIndices: Vec<u16>
}

impl Parseable<MOBRChunk> for MOBRChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOBRChunk, ParserError> {
    Ok(MOBRChunk{ nodeFaceIndices: read_chunk_array(rdr)? })
  }
}

#[derive(Debug)]
pub struct MOCVChunk {
  pub colorVertexList: Vec<CImVector>
}

impl Parseable<MOCVChunk> for MOCVChunk {
  fn parse<R: Read>(rdr: &mut R) -> Result<MOCVChunk, ParserError> {
    Ok(MOCVChunk{ colorVertexList: read_chunk_array(rdr)? })
  }
}
