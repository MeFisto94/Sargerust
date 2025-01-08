use std::collections::HashMap;
use std::ffi::CString;
use std::io::ErrorKind::UnexpectedEof;
use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};

use crate::ParserError;
use crate::common::types::{C2Vector, C3Vector, C4Quaternion, CAaBox, CArgb, CImVector, IffChunk};

pub(crate) trait Parseable<T> {
    fn parse<R: Read>(rdr: &mut R) -> Result<T, ParserError>;
}

impl Parseable<C3Vector> for C3Vector {
    fn parse<R: Read>(rdr: &mut R) -> Result<C3Vector, ParserError> {
        Ok(C3Vector {
            x: rdr.read_f32::<LittleEndian>()?,
            y: rdr.read_f32::<LittleEndian>()?,
            z: rdr.read_f32::<LittleEndian>()?,
        })
    }
}

impl Parseable<C2Vector> for C2Vector {
    fn parse<R: Read>(rdr: &mut R) -> Result<C2Vector, ParserError> {
        Ok(C2Vector {
            x: rdr.read_f32::<LittleEndian>()?,
            y: rdr.read_f32::<LittleEndian>()?,
        })
    }
}

impl Parseable<CAaBox> for CAaBox {
    fn parse<R: Read>(rdr: &mut R) -> Result<CAaBox, ParserError> {
        Ok(CAaBox {
            min: C3Vector::parse(rdr)?,
            max: C3Vector::parse(rdr)?,
        })
    }
}

impl From<u32> for CImVector {
    fn from(value: u32) -> Self {
        let bytes = value.to_le_bytes();
        CImVector {
            b: bytes[0],
            g: bytes[1],
            r: bytes[2],
            a: bytes[3],
        }
    }
}

impl From<CImVector> for u32 {
    fn from(value: CImVector) -> Self {
        u32::from_le_bytes([value.b, value.g, value.r, value.a])
    }
}

impl Parseable<CImVector> for CImVector {
    fn parse<R: Read>(rdr: &mut R) -> Result<CImVector, ParserError> {
        Ok(CImVector::from(rdr.read_u32::<LittleEndian>()?))
    }
}

impl From<u32> for CArgb {
    fn from(value: u32) -> Self {
        let bytes = value.to_le_bytes();
        CArgb {
            r: bytes[0],
            g: bytes[1],
            b: bytes[2],
            a: bytes[3],
        }
    }
}

impl From<CArgb> for u32 {
    fn from(value: CArgb) -> Self {
        u32::from_le_bytes([value.r, value.g, value.b, value.a])
    }
}

impl Parseable<CArgb> for CArgb {
    fn parse<R: Read>(rdr: &mut R) -> Result<CArgb, ParserError> {
        Ok(CArgb::from(rdr.read_u32::<LittleEndian>()?))
    }
}

impl Parseable<C4Quaternion> for C4Quaternion {
    fn parse<R: Read>(rdr: &mut R) -> Result<C4Quaternion, ParserError> {
        Ok(C4Quaternion {
            x: rdr.read_f32::<LittleEndian>()?,
            y: rdr.read_f32::<LittleEndian>()?,
            z: rdr.read_f32::<LittleEndian>()?,
            w: rdr.read_f32::<LittleEndian>()?,
        })
    }
}

impl Parseable<u8> for u8 {
    fn parse<R: Read>(rdr: &mut R) -> Result<u8, ParserError> {
        Ok(rdr.read_u8()?)
    }
}

impl Parseable<i8> for i8 {
    fn parse<R: Read>(rdr: &mut R) -> Result<i8, ParserError> {
        Ok(rdr.read_i8()?)
    }
}

impl Parseable<u16> for u16 {
    fn parse<R: Read>(rdr: &mut R) -> Result<u16, ParserError> {
        Ok(rdr.read_u16::<LittleEndian>()?)
    }
}

impl Parseable<i16> for i16 {
    fn parse<R: Read>(rdr: &mut R) -> Result<i16, ParserError> {
        Ok(rdr.read_i16::<LittleEndian>()?)
    }
}

impl Parseable<u32> for u32 {
    fn parse<R: Read>(rdr: &mut R) -> Result<u32, ParserError> {
        Ok(rdr.read_u32::<LittleEndian>()?)
    }
}

impl Parseable<f32> for f32 {
    fn parse<R: Read>(rdr: &mut R) -> Result<f32, ParserError> {
        Ok(rdr.read_f32::<LittleEndian>()?)
    }
}

impl Parseable<u64> for u64 {
    fn parse<R: Read>(rdr: &mut R) -> Result<u64, ParserError> {
        Ok(rdr.read_u64::<LittleEndian>()?)
    }
}

impl Parseable<u128> for u128 {
    fn parse<R: Read>(rdr: &mut R) -> Result<u128, ParserError> {
        Ok(rdr.read_u128::<LittleEndian>()?)
    }
}

// Helper Type because we have multiple chunks that are merely String Arrays.
#[derive(Debug, Clone)]
pub struct GenericStringList {
    pub string_list: Vec<String>,
    pub offset_to_string_list_offset: HashMap<u32, usize>,
}

impl Parseable<GenericStringList> for GenericStringList {
    fn parse<R: Read>(rdr: &mut R) -> Result<GenericStringList, ParserError> {
        let mut map = HashMap::new();
        let mut list = Vec::new();
        let mut byte_ctr = 0u32;
        loop {
            let cstring_res = read_cstring(rdr);
            if cstring_res.is_ok() {
                let string = cstring_res?.into_string()?;

                // This is a costly way to skip the padding, but we can't properly "peek" with rdr
                // Furthermore, making read_cstring skip leading NUL-bytes makes it less readable and concise.
                // Additionally, thanks to MODDs byte offset, we need to keep track of the relative offsets of every string.
                if !string.is_empty() {
                    map.insert(byte_ctr, list.len());
                    let str_len = (string.chars().count() + 1) as u32;
                    list.push(string);
                    byte_ctr += str_len;
                } else {
                    byte_ctr += 1;
                }
            } else {
                match cstring_res {
                    Err(ParserError::IOError(internal)) if internal.kind() == UnexpectedEof => break,
                    err => return Err(err.unwrap_err()),
                };
            }
        }

        Ok(GenericStringList {
            string_list: list,
            offset_to_string_list_offset: map,
        })
    }
}

pub(crate) fn read_cstring<R: Read>(rdr: &mut R) -> Result<CString, ParserError> {
    let mut buf = Vec::new();
    loop {
        let c = rdr.read_u8()?;
        if c == 0 {
            // SAFETY: We can ensure, that there are no nul-bytes in buf
            return Ok(unsafe { CString::from_vec_unchecked(buf) });
        }
        buf.push(c);
    }
}

pub(crate) fn read_chunk_array<T: Parseable<T>, R: Read>(rdr: &mut R) -> Result<Vec<T>, ParserError> {
    let mut list = Vec::<T>::new();
    let mut element = T::parse(rdr);
    while element.is_ok() {
        list.push(element?);
        element = T::parse(rdr);
    }

    // weird error handling because when EoF, we get that inside a parser error.
    match element {
        Err(ParserError::IOError(internal)) if internal.kind() == UnexpectedEof => (),
        err => return err.map(|_| Vec::with_capacity(0)),
    };
    Ok(list)
}

pub(crate) fn get_mandatory_chunk_by_name<T: Parseable<T>>(
    chunk_list: &Vec<IffChunk>,
    chunk_magic: &str,
) -> Result<T, ParserError> {
    chunk_list
        .iter()
        .find(|chunk| chunk.magic_str().eq(chunk_magic))
        .unwrap_or_else(|| panic!("Missing mandatory {} chunk", chunk_magic))
        .parse::<T>()
}

pub(crate) fn get_optional_chunk_by_name<T: Parseable<T>>(
    chunk_list: &Vec<IffChunk>,
    chunk_magic: &str,
) -> Result<Option<T>, ParserError> {
    chunk_list
        .iter()
        .find(|chunk| chunk.magic_str().eq(chunk_magic))
        .map(|chunk| chunk.parse::<T>())
        .transpose()
}
