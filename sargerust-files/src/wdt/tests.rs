  use std::fs::File;
  use std::io::BufReader;

  use crate::wdt::reader::WDTReader;

  # [test]
  fn simple_parse() -> Result<(), anyhow::Error> {
    let test_data = std::env::current_dir()?.join("test-data");

    let mut file = BufReader::new(File::open(test_data.join("World_Maps_DeeprunTram_DeeprunTram.wdt"))?);
    let asset = WDTReader::parse_asset(&mut file)?;

    Ok(())
  }