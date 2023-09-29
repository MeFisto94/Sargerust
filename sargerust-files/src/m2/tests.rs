  use std::fs::File;
  use std::io::{BufReader, BufWriter};

  use crate::m2::reader::M2Reader;

  # [test]
  fn m2_parsing_and_obj_dumping() -> Result<(), anyhow::Error> {
    let test_data = std::env::current_dir()?.join("test-data");

    let mut file = BufReader::new(File::open(test_data.join("Chair01.m2"))?);
    let asset = M2Reader::parse_asset(&mut file)?;

    let mut skin_file = BufReader::new(File::open(test_data.join("Chair0100.skin"))?);
    let skin = M2Reader::parse_skin_profile(&mut skin_file)?;

    let mut w = BufWriter::new(File::create("./test.obj")?);
    asset.dump_to_wavefront_obj(&mut w, &skin)?;

    Ok(())
  }
