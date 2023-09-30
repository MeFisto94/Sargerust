use std::io::Cursor;
use mpq::Archive;

pub fn read_mpq_file_into_owned(archive: &mut Archive, file_name: &str) -> Result<Vec<u8>, std::io::Error> {
    let file = archive.open_file(file_name)?;
    let mut buf: Vec<u8> = vec![0; file.size() as usize];
    file.read(archive, &mut buf)?;
    Ok(buf)
}

pub fn read_mpq_file_into_cursor(archive: &mut Archive, file_name: &str) -> Result<Cursor<Vec<u8>>, std::io::Error> {
    read_mpq_file_into_owned(archive, file_name).map(|buf| Cursor::new(buf))
}