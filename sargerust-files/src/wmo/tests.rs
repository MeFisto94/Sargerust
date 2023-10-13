
use std::fs::File;
use std::io::BufReader;

use crate::wmo::reader::WMOReader;

#[test]
fn parse_root() -> Result<(), anyhow::Error> {
    let test_data = std::env::current_dir()?.join("test-data");
    let mut file = BufReader::new(File::open(
        test_data.join("World_wmo_Dungeon_AZ_Subway_Subway.wmo"),
    )?);
    let asset = WMOReader::parse_root(&mut file)?;

    Ok(())
}

#[test]
fn parse_group() -> Result<(), anyhow::Error> {
    let test_data = std::env::current_dir()?.join("test-data");
    let mut file = BufReader::new(File::open(
        test_data.join("World_wmo_Dungeon_AZ_Subway_Subway_000.wmo"),
    )?);
    let group_asset = WMOReader::parse_group(&mut file)?;

    Ok(())
}
