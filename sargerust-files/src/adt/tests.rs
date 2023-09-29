use std::fs::File;
use std::io::BufReader;
use crate::adt::reader::ADTReader;

# [test]
fn parse_gm_island() -> Result<(), anyhow::Error> {
  let test_data = std::env::current_dir()?.join("test-data");
  let wmo = "World_Maps_Kalimdor_Kalimdor_0_0.adt";
  let mut file = BufReader::new(File::open(test_data.join(wmo))?);
  let asset = ADTReader::parse_asset(&mut file)?;
  Ok(())
}
