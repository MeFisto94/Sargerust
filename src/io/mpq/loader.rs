use std::cmp::Ordering;
use std::fs;
use std::io::Cursor;
use std::ops::DerefMut;
use std::path::Path;
use std::sync::RwLock;

use itertools::Itertools;
use log::{trace, warn};

use mpq::Archive;

use crate::io::common::loader::RawAssetLoader;

pub fn read_mpq_file_into_owned(archive: &mut Archive, file_name: &str) -> Result<Vec<u8>, std::io::Error> {
    let file = archive.open_file(file_name)?;
    let mut buf: Vec<u8> = vec![0; file.size() as usize];
    file.read(archive, &mut buf)?;
    Ok(buf)
}

pub fn read_mpq_file_into_cursor(archive: &mut Archive, file_name: &str) -> Result<Cursor<Vec<u8>>, std::io::Error> {
    read_mpq_file_into_owned(archive, file_name).map(Cursor::new)
}

pub struct MPQLoader {
    prioritized_archives: Vec<(String, RwLock<Archive>)>,
    #[allow(unused)]
    // Will become used once MPQLoader is concurrent (because then we construct new readers from the data_folder and the archive name)
    data_folder: String,
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
enum MPQType {
    Base,
    Patch,
    Tbc,
    Wotlk,
    Common,
    Unknown,
}

impl MPQLoader {
    pub fn new(data_folder: &str) -> Self {
        // load-order: base>patch-Z>A>9>1>lichking>expansion>common
        // see also https://github.com/namreeb/namigator/issues/22#issuecomment-833183096 and https://github.com/namreeb/namigator/issues/22#issuecomment-834792971

        let prioritized_archives = fs::read_dir(data_folder)
            .unwrap_or_else(|_| {
                panic!(
                    "MPQLoader: Failed to enumerate data folder: {}",
                    data_folder
                )
            })
            .filter_map(|file| file.ok())
            .flat_map(|file| {
                if file.path().is_dir() {
                    return fs::read_dir(file.path())
                        .unwrap_or_else(|_| {
                            panic!(
                                "MPQLoader: Failed to enumerate data folder: {}",
                                data_folder
                            )
                        })
                        .filter_map(|file| file.ok())
                        .filter(|file| file.path().is_file()) // no further recursion
                        .collect_vec();
                }

                vec![file]
            })
            .map(|entry| {
                (
                    entry
                        .file_name()
                        .into_string()
                        .expect("Failed to convert filename"),
                    entry,
                )
            })
            .filter(|(filename, _)| filename.to_ascii_lowercase().ends_with("mpq"))
            .sorted_by(|a, b| MPQLoader::sorting_order(&a.0, &b.0))
            .map(|(filename, entry)| {
                (
                    filename,
                    RwLock::new(
                        Archive::open(entry.path())
                            .unwrap_or_else(|_| panic!("Failed to load MPQ {}", entry.path().to_str().unwrap())),
                    ),
                )
            })
            .collect_vec();

        MPQLoader {
            prioritized_archives,
            data_folder: data_folder.into(),
        }
    }

    // TODO: understand locales (e.g. deDE) and their order/priority.
    fn sorting_order(a: &String, b: &String) -> Ordering {
        let type_a = MPQLoader::extract_mpq_type(a);
        let type_b = MPQLoader::extract_mpq_type(b);

        let ord = type_a.partial_cmp(&type_b);

        if ord.is_none() || ord.unwrap() == Ordering::Equal {
            let version_a = MPQLoader::extract_mpq_version(a);
            let version_b = MPQLoader::extract_mpq_version(b);

            if version_a.is_some() && version_b.is_none() {
                if type_a == MPQType::Common {
                    // common has inverted ordering
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            } else if version_a.is_none() && version_b.is_some() {
                if type_a == MPQType::Common {
                    // common has inverted ordering
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            } else if version_a.is_none() && version_b.is_none() {
                Ordering::Equal
            } else {
                #[allow(clippy::collapsible_else_if)]
                if type_b == MPQType::Common {
                    // common has inverted ordering. This branch probably never happens, unless we have common-3
                    version_a.unwrap().partial_cmp(&version_b.unwrap()).unwrap()
                } else {
                    // for patches z > a > 9 > 1
                    version_b.unwrap().partial_cmp(&version_a.unwrap()).unwrap()
                }
            }
        } else {
            ord.unwrap()
        }
    }

    fn extract_mpq_type(file_name: &str) -> MPQType {
        if file_name.starts_with("common") {
            MPQType::Common
        } else if file_name.starts_with("expansion") {
            MPQType::Tbc
        } else if file_name.starts_with("lichking") {
            MPQType::Wotlk
        } else if file_name.starts_with("patch") {
            MPQType::Patch
        } else {
            MPQType::Unknown
        }
    }

    fn extract_mpq_version(file_name: &String) -> Option<u8> {
        if file_name[file_name.chars().count() - 6..file_name.chars().count() - 5].eq("-") {
            Some(file_name.as_bytes()[file_name.len() - 5..][0])
        } else {
            None
        }
    }
}

impl RawAssetLoader for MPQLoader {
    fn load_raw(&self, _path: &str) -> &[u8] {
        //&self.load_raw_owned(path)
        todo!()
    }

    fn load_raw_owned(&self, path: &str) -> Option<Vec<u8>> {
        // the very bad API design of the mpq crate currently loads the file as soon as we try to open it.
        let opt = self
            .prioritized_archives
            .iter()
            .map(|(name, archive)| {
                let exists = archive
                    .read()
                    .map(|ar| ar.contains_file(path))
                    .unwrap_or(false);
                (name, archive, exists)
            })
            .find(|(_, _, exists)| *exists)
            .map(|(name, archive, _)| (name, archive));

        if opt.is_none() {
            warn!("Could not locate {}!", path);
        }

        opt.map(|(name, archive_guard)| {
            trace!("Loading {} from {}", path, name);
            let mut guard = archive_guard.write().unwrap();
            let archive = guard.deref_mut();
            let file = archive.open_file(path).unwrap();
            let mut buf: Vec<u8> = vec![0; file.size() as usize];
            file.read(archive, &mut buf)
                .expect("I/O Error. TODO: Error handling");
            buf
        })
    }
}
