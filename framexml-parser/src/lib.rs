extern crate core;

use crate::typedefs::Ui;
use quick_xml::de::Deserializer;
use serde::Deserialize;
use std::io::BufRead;

pub mod anchor;
pub mod attributes;
pub mod dimensions;
pub mod layers;
pub mod scripts;
pub mod toc;
pub mod typedefs;

// TODO: Better struct? Error handling
pub fn deserialize_xml<T: BufRead>(read: T) -> Result<Ui, String> {
    let mut deserializer = Deserializer::from_reader(read);
    Ui::deserialize(&mut deserializer).map_err(|e| e.to_string())
}
