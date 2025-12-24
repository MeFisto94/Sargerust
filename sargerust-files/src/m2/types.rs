#![allow(non_snake_case)] // we use the exact wording from wowdev.wiki
use crate::ParserError;
use crate::common::reader::Parseable;
use crate::common::types::{C2Vector, C3Vector};
use crate::m2::reader::M2Reader;
use bitflags::bitflags;
use byteorder::{LittleEndian, ReadBytesExt};
use sargerust_files_derive_parseable::Parse;
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
    pub materials: Vec<M2Material>,
    /// Aka texture_lookup_table: Is used to lookup from batch to texture index
    pub textureCombos: Vec<u16>,
    pub textureCoordCombos: Vec<u16>,
    /// Aka transparency_lookup_table
    pub textureWeightCombos: Vec<u16>,
    pub textureTransformCombos: Vec<u16>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum M2TextureType {
    /// Texture given in filename
    None,
    /// Skin / Body + Clothes
    TexComponentSkin,
    /// Object Skin (Item, Capes, Item\ObjectComponents\Cape\*.blp)
    TexComponentObjectSkin,
    /// WeaponBlade (not used in the client?)
    TexComponentWeaponBlade,
    /// Weapon Handle
    TexComponentWeaponHandle,
    /// Environment (Obsolete, please remove from source art)
    TexComponentEnvironment,
    /// Character Hair
    TexComponentCharacterHair,
    /// Character Facial Hair (Obsolete, please remove from source art)
    TexComponentCharacterFacialHair,
    /// Skin Extra
    TexComponentSkinExtra,
    /// UI Skin -- Used on invetory art M2s (1) InventoryArtGeometry.m2 and InventoryArtGeometryOld.m2
    TexComponentUISkin,
    /// Character Misc (Tauren Mane), Obsolete, please remove from source art
    TexComponentTaurenMane,
    /// Monster Skin 1 (Skin for Creatures or GameObjects)
    TexComponentMonster1,
    /// Monster Skin 2 (Skin for Creatures or GameObjects)
    TexComponentMonster2,
    /// Monster Skin 3 (Skin for Creatures or GameObjects)
    TexComponentMonster3,
    /// Item Icon (Used on inventory art m2s: ui-button.m2 and forcedbackpackitem.m2)
    TexComponentItemIcon,
    // From here on: CATA.
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct M2TextureFlags: u32 {
        const WRAP_X = 0x1;
        const WRAP_Y = 0x2;
    }
}

#[derive(Debug, Clone)]
pub struct M2Texture {
    // TODO: This could be an enum with filename and without
    pub texture_type: M2TextureType,
    pub texture_flags: M2TextureFlags, // 0x1 wrap x, 0x2 wrap y, 0x3 = wrap x+y (bitflags)
    pub filename: String,              // maximum of 0x108 chars
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct M2MaterialFlag: u16 {
        const UNLIT = 0x1;
        const UNFOGGED = 0x2;
        const TWO_SIDED = 0x4; // No backface culling
        const DEPTH_TEST = 0x8;
        const DEPTH_WRITE = 0x10;
        /// seems to be WoD+?
        const SHADOW_BATCH_RELATED_1 = 0x40;
        /// seems to be WoD+?
        const SHADOW_BATCH_RELATED_2 = 0x80;
        /// (seen in 1 model in Wrath : HFjord_Fog_02.m2)
        const UNK_1 = 0x100;
        /// seems to be WoD+?
        const SHADOW_BATCH_RELATED_3 = 0x200;
        /// seems to be WoD+?
        const UNK_2 = 0x400;
        /// prevent alpha for custom elements. if set, use (fully) opaque or transparent. (litSphere, shadowMonk) (MoP+)
        const PREVENT_ALPHA_CUSTOM_ELEMENT = 0x800;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum M2BlendingMode {
    Opaque,
    AlphaKey,
    Alpha,
    NoAlphaAdditive,
    Additive,
    Modulative,
    Modulative2x,
    BlendAdd,
}

impl TryFrom<u16> for M2BlendingMode {
    type Error = ();

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(M2BlendingMode::Opaque),
            1 => Ok(M2BlendingMode::AlphaKey),
            2 => Ok(M2BlendingMode::Alpha),
            3 => Ok(M2BlendingMode::NoAlphaAdditive),
            4 => Ok(M2BlendingMode::Additive),
            5 => Ok(M2BlendingMode::Modulative),
            6 => Ok(M2BlendingMode::Modulative2x),
            7 => Ok(M2BlendingMode::BlendAdd),
            _ => Err(()),
        }
    }
}

impl From<M2BlendingMode> for u16 {
    fn from(value: M2BlendingMode) -> Self {
        match value {
            M2BlendingMode::Opaque => 0,
            M2BlendingMode::AlphaKey => 1,
            M2BlendingMode::Alpha => 2,
            M2BlendingMode::NoAlphaAdditive => 3,
            M2BlendingMode::Additive => 4,
            M2BlendingMode::Modulative => 5,
            M2BlendingMode::Modulative2x => 6,
            M2BlendingMode::BlendAdd => 7,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct M2Material {
    pub flags: M2MaterialFlag,
    pub blending_mode: M2BlendingMode,
}

impl Parseable<M2Material> for M2Material {
    fn parse<R: Read>(rdr: &mut R) -> Result<M2Material, ParserError> {
        Ok(M2Material {
            flags: M2MaterialFlag::from_bits_retain(rdr.read_u16::<LittleEndian>()?),
            blending_mode: rdr
                .read_u16::<LittleEndian>()?
                .try_into()
                .map_err(|_| ParserError::FormatError {
                    reason: "Unknown M2Material#blending_mode",
                })?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct M2TextureInternal {
    pub texture_type: u32,
    pub texture_flags: u32,
    pub filename: M2Array,
}

impl TryFrom<u32> for M2TextureType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(M2TextureType::None),
            1 => Ok(M2TextureType::TexComponentSkin),
            2 => Ok(M2TextureType::TexComponentObjectSkin),
            3 => Ok(M2TextureType::TexComponentWeaponBlade),
            4 => Ok(M2TextureType::TexComponentWeaponHandle),
            5 => Ok(M2TextureType::TexComponentEnvironment),
            6 => Ok(M2TextureType::TexComponentCharacterHair),
            7 => Ok(M2TextureType::TexComponentCharacterFacialHair),
            8 => Ok(M2TextureType::TexComponentSkinExtra),
            9 => Ok(M2TextureType::TexComponentUISkin),
            10 => Ok(M2TextureType::TexComponentTaurenMane),
            11 => Ok(M2TextureType::TexComponentMonster1),
            12 => Ok(M2TextureType::TexComponentMonster2),
            13 => Ok(M2TextureType::TexComponentMonster3),
            14 => Ok(M2TextureType::TexComponentItemIcon),
            _ => Err(()),
        }
    }
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
    pub batches: Vec<M2Batch>,
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

impl M2SkinSection {
    pub fn vertex_start(&self) -> usize {
        self.vertexStart as usize | ((self.Level as usize) << 16)
    }

    pub fn index_start(&self) -> usize {
        self.indexStart as usize | ((self.Level as usize) << 16)
    }
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

#[derive(Debug, Parse)]
pub struct M2Batch {
    /// Usually 16 for static textures, and 0 for animated textures. &0x1: materials invert something; &0x2: transform
    /// &0x4: projected texture; &0x10: something batch compatible; &0x20: projected texture?;
    /// &0x40: possibly don't multiply transparency by texture weight transparency to get final transparency value(?)
    pub flags: u8,
    pub priorityPlane: i8,
    pub shader_id: u16,
    // a duplicate entry of a submesh from the list above
    pub skinSectionIndex: u16,
    // See below. New name: flags2 (BfA). 0x2 - projected. 0x8 - EDGF chunk in m2 is mandatory and data from is applied to this mesh
    pub geosetIndex: u16,
    // A Color out of the Colors-Block or -1 if none.
    pub colorIndex: u16,
    // The renderflags used on this texture-unit.
    pub materialIndex: u16,
    // Capped at 7 (see CM2Scene::BeginDraw)
    pub materialLayer: u16,
    // 1 to 4. See below. Also seems to be the number of textures to load, starting at the texture lookup in the next field (0x10).
    pub textureCount: u16,
    // The index in the texture lookup table.
    pub textureComboIndex: u16,
    // The index in the texture mapping lookup table
    pub textureCoordComboIndex: u16,
    // The index into the transparency lookup table
    pub textureWeightComboIndex: u16,
    // The index into the uvanimation lookup table
    pub textureTransformComboIndex: u16,
}
