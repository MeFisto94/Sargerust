use std::io::ErrorKind::UnexpectedEof;
use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::ParserError;
use crate::common::reader::Parseable;
use crate::common::types::{IffChunk, MVerChunk};
use crate::wmo::types::{
    MFOGChunk, MOBAChunk, MOBNChunk, MOBRChunk, MOCVChunk, MODDChunk, MODNChunk, MODRChunk, MODSChunk, MOGIChunk,
    MOGNChunk, MOGPChunk, MOHDChunk, MOLRChunk, MOLTChunk, MOMTChunk, MONRChunk, MOPYChunk, MOSBChunk, MOTVChunk,
    MOTXChunk, MOVIChunk, MOVTChunk, WMOGroupAsset, WMORootAsset,
};

pub struct WMOReader {}

impl WMOReader {
    pub fn parse_root<R: Read>(rdr: &mut R) -> Result<WMORootAsset, ParserError> {
        // TODO: We don't necessarily have MVER as the first chunk, we don't need to depend on that.
        let version_hdr = IffChunk::read_next_chunk(rdr)?;
        if !version_hdr.magic_str().eq("MVER") {
            return Err(ParserError::InvalidMagicValue {
                magic: version_hdr.magic,
            });
        }

        let mver = version_hdr.parse::<MVerChunk>()?;
        if mver.version != 17 {
            return Err(ParserError::FormatError {
                reason: "Unknown MVER Version",
            });
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

        let mohd = WMOReader::get_mandatory_chunk_by_name::<MOHDChunk>(&chunk_list, "MOHD")?;
        let motx = WMOReader::get_mandatory_chunk_by_name::<MOTXChunk>(&chunk_list, "MOTX")?;

        let momt_chunk = chunk_list
            .iter()
            .find(|chunk| chunk.magic_str().eq("MOMT"))
            .expect("Missing mandatory MOMT chunk");
        if momt_chunk.size % 64 != 0 {
            return Err(ParserError::FormatError {
                reason: "Invalid MOMT Chunk Size",
            });
        }
        let momt = momt_chunk.parse::<MOMTChunk>()?;

        let mogn = WMOReader::get_mandatory_chunk_by_name::<MOGNChunk>(&chunk_list, "MOGN")?;
        let mogi = WMOReader::get_mandatory_chunk_by_name::<MOGIChunk>(&chunk_list, "MOGI")?;

        let mosb = chunk_list
            .iter()
            .find(|chunk| chunk.magic_str().eq("MOSB"))
            .map(|chunk| chunk.parse::<MOSBChunk>())
            .transpose()?;

        // TODO: MOVV; MOVB
        // TODO: Portal chunks. MOPV; MOPT, MOPR.
        let molt = WMOReader::get_mandatory_chunk_by_name::<MOLTChunk>(&chunk_list, "MOLT")?;
        let mods = WMOReader::get_mandatory_chunk_by_name::<MODSChunk>(&chunk_list, "MODS")?;
        let modn = WMOReader::get_mandatory_chunk_by_name::<MODNChunk>(&chunk_list, "MODN")?;
        let modd = WMOReader::get_mandatory_chunk_by_name::<MODDChunk>(&chunk_list, "MODD")?;
        let mfog = WMOReader::get_mandatory_chunk_by_name::<MFOGChunk>(&chunk_list, "MFOG")?;
        // MCVP optional. For inside and outside knowledge. Convex Volume Plane

        Ok(WMORootAsset {
            mver,
            mohd,
            motx,
            momt,
            mogn,
            mogi,
            mosb,
            molt,
            mods,
            modn,
            modd,
            mfog,
        })
    }

    pub fn parse_group<R: Read>(rdr: &mut R) -> Result<WMOGroupAsset, ParserError> {
        // TODO: We don't necessarily have MVER as the first chunk, we don't need to depend on that.
        let version_hdr = IffChunk::read_next_chunk(rdr)?;
        if !version_hdr.magic_str().eq("MVER") {
            return Err(ParserError::InvalidMagicValue {
                magic: version_hdr.magic,
            });
        }

        let mver = version_hdr.parse::<MVerChunk>()?;
        if mver.version != 17 {
            return Err(ParserError::FormatError {
                reason: "Unknown MVER Version",
            });
        }

        let mogp_chunk = IffChunk::read_next_chunk(rdr)?;
        let mogp = mogp_chunk.parse::<MOGPChunk>()?;

        // we need to re-assign the reader to be inside the MOGP Chunk.
        assert_eq!(std::mem::size_of::<MOGPChunk>(), 0x44);
        let mut mogp_reader = Cursor::new(mogp_chunk.data);
        mogp_reader.seek(SeekFrom::Start(0x44))?; // size_of MOGPChunk
        let rdr = &mut mogp_reader; // use shadowing to fake the new reader.

        // This order and type is apparently guaranteed, at least vanilla can't read the files otherwise
        let mopy = IffChunk::read_next_chunk(rdr)?.parse::<MOPYChunk>()?;
        let movi = IffChunk::read_next_chunk(rdr)?.parse::<MOVIChunk>()?;
        let movt = IffChunk::read_next_chunk(rdr)?.parse::<MOVTChunk>()?;
        let monr = IffChunk::read_next_chunk(rdr)?.parse::<MONRChunk>()?;
        let motv = IffChunk::read_next_chunk(rdr)?.parse::<MOTVChunk>()?;
        let moba = IffChunk::read_next_chunk(rdr)?.parse::<MOBAChunk>()?;

        // since here, optional and maybe not in order.
        let mut chunk_list = Vec::<IffChunk>::new();
        let mut chunk_res = IffChunk::read_next_chunk(rdr);
        while chunk_res.is_ok() {
            chunk_list.push(chunk_res.unwrap());
            chunk_res = IffChunk::read_next_chunk(rdr);
        }
        match chunk_res {
            Err(ParserError::IOError(internal)) if internal.kind() == UnexpectedEof => (),
            err => return Err(err.unwrap_err()),
        };

        let molr = WMOReader::get_optional_chunk_by_name::<MOLRChunk>(&chunk_list, "MOLR")?;
        let modr = WMOReader::get_optional_chunk_by_name::<MODRChunk>(&chunk_list, "MODR")?;
        let mobn = WMOReader::get_optional_chunk_by_name::<MOBNChunk>(&chunk_list, "MOBN")?;
        let mobr = WMOReader::get_optional_chunk_by_name::<MOBRChunk>(&chunk_list, "MOBR")?;
        let mocv = WMOReader::get_optional_chunk_by_name::<MOCVChunk>(&chunk_list, "MOCV")?;

        Ok(WMOGroupAsset {
            mver,
            mogp,
            mopy,
            movi,
            movt,
            monr,
            motv,
            moba,
            molr,
            modr,
            mobn,
            mobr,
            mocv,
        })
    }

    fn get_mandatory_chunk_by_name<T: Parseable<T>>(
        chunk_list: &Vec<IffChunk>,
        chunk_magic: &str,
    ) -> Result<T, ParserError> {
        chunk_list
            .iter()
            .find(|chunk| chunk.magic_str().eq(chunk_magic))
            .expect(&format!("Missing mandatory {} chunk", chunk_magic))
            .parse::<T>()
    }

    fn get_optional_chunk_by_name<T: Parseable<T>>(
        chunk_list: &Vec<IffChunk>,
        chunk_magic: &str,
    ) -> Result<Option<T>, ParserError> {
        chunk_list
            .iter()
            .find(|chunk| chunk.magic_str().eq(chunk_magic))
            .map(|chunk| chunk.parse::<T>())
            .transpose()
    }
}
