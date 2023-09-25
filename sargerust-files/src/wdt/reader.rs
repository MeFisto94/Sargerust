use std::io::ErrorKind::UnexpectedEof;
use std::io::Read;

use crate::common::types::{IffChunk, MVerChunk};
use crate::ParserError;
use crate::wdt::types::{MainChunk, MODFChunk, MPHDChunk, MWMOChunk, WDTAsset};

pub struct WDTReader {
}

impl WDTReader {
  pub fn parse_asset<R: Read>(rdr: &mut R) -> Result<WDTAsset, ParserError> {
    // TODO: We don't necessarily have MVER as the first chunk, we don't need to depend on that.
    let version_hdr = IffChunk::read_next_chunk(rdr)?;
    if !version_hdr.magic_str().eq("MVER") {
      return Err(ParserError::InvalidMagicValue { magic: version_hdr.magic });
    }

    let mver = version_hdr.parse::<MVerChunk>()?;
    if mver.version != 18 {
      return Err(ParserError::FormatError { reason: "Unknown MVER Version: 18" });
    }

    let mut chunk_list = Vec::<IffChunk>::new();
    let mut chunk_res = IffChunk::read_next_chunk(rdr);
    while chunk_res.is_ok() {
      chunk_list.push(chunk_res.unwrap());
      chunk_res = IffChunk::read_next_chunk(rdr);
    }

    // weird error handling because when EoF, we get that inside a parser error.
    let err = chunk_res.unwrap_err();
    if let ParserError::IOError(inner_error) = err {
      if inner_error.kind() != UnexpectedEof {
        return Err(ParserError::IOError(inner_error));
      }
    } else {
      return Err(err);
    }

    // No real error, only an EOF
    let mphd= chunk_list.iter()
      .find(|chunk| chunk.magic_str().eq("MPHD"))
      .expect("Missing mandatory MPHD chunk")
      .parse::<MPHDChunk>()?;

    let main_chunk = chunk_list.iter()
      .find(|chunk| chunk.magic_str().eq("MAIN"))
      .expect("Missing mandatory MAIN chunk");

    if main_chunk.size != 32768 {
      return Err(ParserError::FormatError { reason: "Invalid Main Chunk size" });
    }
    let main = main_chunk.parse::<MainChunk>()?;

    // Apparently the following chunks are only there for non-terrain worlds?
    let mwmo_chunk = chunk_list.iter()
      .find(|chunk| chunk.magic_str().eq("MWMO"))
      .expect("Missing mandatory MWMO chunk");
    if mwmo_chunk.size > 0x100 {
      return Err(ParserError::FormatError { reason: "Invalid MWMO Chunk size" });
    }

    let mwmo = MWMOChunk::from(mwmo_chunk);
    dbg!(mwmo);

    let modf = chunk_list.iter()
      .find(|chunk| chunk.magic_str().eq("MODF"))
      .expect("Missing mandatory MODF chunk")
      .parse::<MODFChunk>()?;
    dbg!(modf);

    Ok(WDTAsset {
      mphd,
      main
    })
  }
}

