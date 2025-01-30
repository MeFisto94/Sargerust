use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use log::trace;
use wow_dbc::DbcTable;

pub mod loader;

pub fn load_dbc<T: DbcTable>(mpq_loader: &MPQLoader, name: &str) -> T {
    let buf = mpq_loader
        .load_raw_owned(name)
        .unwrap_or_else(|| panic!("Failed to load {}", name));
    trace!("Loaded {} ({} bytes)", name, buf.len());
    T::read(&mut buf.as_slice()).unwrap_or_else(|_| panic!("Failed to parse {}", name))
}
