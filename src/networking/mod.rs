use wow_srp::wrath_header::ClientDecrypterHalf;

pub mod application;
pub mod auth;
pub mod movement_tracker;
pub mod utils;
pub mod world;

pub fn skip_encrypted<R: std::io::Read>(
    mut r: R,
    d: &mut ClientDecrypterHalf,
) -> Result<u16, wow_world_messages::errors::ExpectedOpcodeError> {
    let mut header = [0_u8; 4];
    r.read_exact(&mut header)?;
    d.decrypt(&mut header);

    let (body_size, opcode) = if header[0] & 0x80 != 0 {
        let size = u32::from_be_bytes([0x00, header[0] & 0x7F, header[1], header[2]]).saturating_sub(3);

        let mut last_byte = [0_u8; 1];
        r.read_exact(&mut last_byte)?;
        d.decrypt(&mut last_byte);
        let opcode = u16::from_le_bytes([header[3], last_byte[0]]);
        (size, opcode)
    } else {
        let size = u16::from_be_bytes([header[0], header[1]])
            .saturating_sub(2)
            .into();
        let opcode = u16::from_le_bytes([header[2], header[3]]);
        (size, opcode)
    };

    let mut buf = vec![0; body_size as usize];
    r.read_exact(&mut buf)?;
    Ok(opcode)
}
