use std::io::ErrorKind::UnexpectedEof;
use std::io::Read;

use crate::adt::types::{ADTAsset, MCINChunk, MCNKChunk, MDDFChunk, MH2OChunk, MHDRChunk, MMDXChunk, MMIDChunk, MTEXChunk, MWIDChunk, MWMOChunk};
use crate::common::reader::{get_mandatory_chunk_by_name, Parseable};
use crate::common::types::{IffChunk, MVerChunk};
use crate::ParserError;

pub struct ADTReader {
}

impl ADTReader {
  pub fn parse_asset<R: Read>(rdr: &mut R) -> Result<Box<ADTAsset>, ParserError> {
    // TODO: We don't necessarily have MVER as the first chunk, we don't need to depend on that.
    let version_hdr = IffChunk::read_next_chunk(rdr)?;
    if !version_hdr.magic_str().eq("MVER") {
      return Err(ParserError::InvalidMagicValue { magic: version_hdr.magic });
    }

    let mver = version_hdr.parse::<MVerChunk>()?;
    if mver.version != 18 {
      return Err(ParserError::FormatError { reason: "Unknown MVER Version" });
    }

    let mut chunk_list = Vec::<IffChunk>::new();
    let mut chunk_res = IffChunk::read_next_chunk(rdr);
    while chunk_res.is_ok() {
      chunk_list.push(chunk_res.unwrap());
      chunk_res = IffChunk::read_next_chunk(rdr);
    }

    // weird error handling because when EoF, we get that inside a parser error.
    match chunk_res {
      Err(ParserError::IOError(internal)) if internal.kind() == UnexpectedEof => (),
      err => return Err(err.unwrap_err()),
    };
    // No real error, only an EOF

    for chunk in &chunk_list {
      dbg!(chunk.magic_str());
    }

    let mhdr = get_mandatory_chunk_by_name::<MHDRChunk>(&chunk_list, "MHDR")?;
    let mcin = get_mandatory_chunk_by_name::<MCINChunk>(&chunk_list, "MCIN")?;
    let mtex = get_mandatory_chunk_by_name::<MTEXChunk>(&chunk_list, "MTEX")?;
    let mmdx = get_mandatory_chunk_by_name::<MMDXChunk>(&chunk_list, "MMDX")?;
    let mmid = get_mandatory_chunk_by_name::<MMIDChunk>(&chunk_list, "MMID")?;
    let mwmo = get_mandatory_chunk_by_name::<MWMOChunk>(&chunk_list, "MWMO")?;
    let mwid = get_mandatory_chunk_by_name::<MWIDChunk>(&chunk_list, "MWID")?;
    let mddf = get_mandatory_chunk_by_name::<MDDFChunk>(&chunk_list, "MDDF")?;
    // TODO: Fix MODF
    //let modf = get_mandatory_chunk_by_name::<MODFChunk>(&chunk_list, "MODF")?;
    let mh2o = get_mandatory_chunk_by_name::<MH2OChunk>(&chunk_list, "MH2O")?;
    // TODO: assert all the coming locations comparing to the offsets here.
    let mcnk_err: Result<Vec<MCNKChunk>, _> = chunk_list.iter()
      .filter(|cnk| cnk.magic_str().eq("MCNK"))
      .map(|cnk| cnk.parse())
      .collect();

    let mcnks = mcnk_err?;
    Ok(Box::new(ADTAsset {
      mhdr,
      mcin,
      mtex,
      mmdx,
      mmid,
      mwmo,
      mwid,
      mddf,
      mh2o,
      mcnks,
    }))
  }
}

