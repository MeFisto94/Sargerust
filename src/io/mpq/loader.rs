use std::cmp::Ordering;
use std::fs;
use std::io::Cursor;
use std::ops::DerefMut;
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

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
enum MPQType {
    // Caution: The order of enum variants also influences loading!
    Base,
    Alternate,
    Patch,
    PatchLocalized,
    Tbc,
    Wotlk,
    Common,
    Locale,
    Speech,
    TbcLocale,
    WotlkLocale,
    TbcSpeech,
    WotlkSpeech,
    Unknown,
}

impl MPQLoader {
    pub fn new(data_folder: &str, locale: &str) -> Self {
        // load-order: base>patch-Z>A>9>1>lichking>expansion>common>locale-LOC>speech-LOC>expansion-locale-LOC>
        // According to https://github.com/namreeb/namigator/issues/22#issuecomment-834813195,
        // alternate.MPQ may also be present in "..\Data", but we don't support escaping data_dir.

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
                    if file.file_name() != locale {
                        warn!(
                            "Found a different locale \"{:?}\" but expected {:?}. Skipping.",
                            file.file_name(),
                            locale
                        );
                        return vec![];
                    }

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

    /// Determine the sorting order of two MPQ files based on their type and version
    fn sorting_order(a: &str, b: &str) -> Ordering {
        let type_a = MPQLoader::extract_mpq_type(a);
        let type_b = MPQLoader::extract_mpq_type(b);

        let ord = type_a.partial_cmp(&type_b);

        if ord.is_none() || ord.unwrap() == Ordering::Equal {
            let version_a = MPQLoader::extract_mpq_version(a);
            let version_b = MPQLoader::extract_mpq_version(b);
            Self::sort_by_version(version_a, version_b, type_a, type_b)
        } else if (type_a == MPQType::Patch && type_b == MPQType::PatchLocalized)
            || (type_a == MPQType::PatchLocalized && type_b == MPQType::Patch)
        {
            // Patches are sorted by version: patch version > patchLocalized version > patch > patchLocalized
            let version_a = MPQLoader::extract_mpq_version(a);
            let version_b = MPQLoader::extract_mpq_version(b);

            if version_a.is_some() && version_b.is_none() {
                Ordering::Less
            } else if version_a.is_none() && version_b.is_some() {
                Ordering::Greater
            } else
            /* if (version_a.is_none() && version_b.is_none()) || (version_a.is_some() && version_b.is_some()) */
            {
                ord.unwrap() // No version or both have a version: Patch > PatchLocalized.
            }
        } else {
            ord.unwrap()
        }
    }

    /// Order the versions of two MPQ files of the same type(!). This is usually patch and common files.
    fn sort_by_version(version_a: Option<u8>, version_b: Option<u8>, type_a: MPQType, type_b: MPQType) -> Ordering {
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
            if type_b == MPQType::Common {
                // common has inverted ordering. This branch probably never happens, unless we have common-3
                version_a.unwrap().partial_cmp(&version_b.unwrap()).unwrap()
            } else {
                // for patches z > a > 9 > 1
                version_b.unwrap().partial_cmp(&version_a.unwrap()).unwrap()
            }
        }
    }

    /// Determine the type of MPQ file (required for ordering) based on it's name alone
    #[inline]
    fn extract_mpq_type(file_name: &str) -> MPQType {
        // Would a regex be more useful? Or passing in the locale and doing .contains?
        // This is especially since we're counting "-"s and then still have to do two contains checks for locale/speech
        // On the other hand, this code is only executed on startup, so we don't need to worry about performance at ALL
        let dash_count = file_name.chars().filter(|c| *c == '-').count();

        if file_name.starts_with("expansion") {
            Self::extract_from_dash_count(file_name, dash_count)
        } else if file_name.starts_with("lichking") {
            // This is a bit ugly, but we wanted to re-use that code, so we just remap TBC -> WotLK
            match Self::extract_from_dash_count(file_name, dash_count) {
                MPQType::Tbc => MPQType::Wotlk,
                MPQType::TbcLocale => MPQType::WotlkLocale,
                MPQType::TbcSpeech => MPQType::WotlkSpeech,
                other => other,
            }
        } else if file_name.starts_with("patch") {
            if dash_count == 2 {
                return MPQType::PatchLocalized;
            } else if dash_count == 0 {
                return MPQType::Patch;
            } else if dash_count == 1 {
                // This could be patch-enUS.mpq or patch-3.mpq
                if file_name.chars().count() == 11 {
                    return MPQType::Patch;
                } else if file_name.chars().count() == 14 {
                    return MPQType::PatchLocalized;
                }
            }

            warn!("Could not determine MPQ type for {}", file_name);
            MPQType::Unknown
        } else if file_name.starts_with("common") {
            MPQType::Common
        } else if file_name.starts_with("base") {
            MPQType::Base
        } else if file_name.starts_with("alternate") {
            MPQType::Alternate
        } else if file_name.starts_with("locale") {
            MPQType::Locale
        } else if file_name.starts_with("speech") {
            MPQType::Speech
        } else {
            MPQType::Unknown
        }
    }

    /// Extract the MPQ type for expansion (tbc) and lichking files based on if they have dashes
    /// and if they are speech or general locale files
    #[inline]
    fn extract_from_dash_count(file_name: &str, dash_count: usize) -> MPQType {
        if dash_count == 2 {
            if file_name.contains("speech") {
                MPQType::TbcSpeech
            } else if file_name.contains("locale") {
                MPQType::TbcLocale
            } else {
                warn!("Could not determine MPQ type for {}", file_name);
                MPQType::Unknown
            }
        } else if dash_count == 0 {
            MPQType::Tbc
        } else {
            warn!("Could not determine MPQ type for {}", file_name);
            MPQType::Unknown
        }
    }

    /// Extract the UTF-8 Character that is used as versioning (0-9, A-Z) i.e patch-7.mpq would return '7' as u8
    /// Note: This is only valid for patch and common files. Or put differently: Files ending with -x.mpq
    /// As such it also works for localized files (e.g. patch-enUS-2.mpq)
    #[inline]
    fn extract_mpq_version(file_name: &str) -> Option<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::SliceRandom;

    #[test]
    fn patch_version_sorting_order() {
        let mut unsorted = vec!["patch-1.mpq", "patch-a.mpq", "patch-9.mpq", "patch-z.mpq"];
        unsorted.sort_by(|a, b| MPQLoader::sorting_order(a, b));
        let expected = vec!["patch-z.mpq", "patch-a.mpq", "patch-9.mpq", "patch-1.mpq"];
        assert_eq!(unsorted, expected);
    }

    #[test]
    fn common_version_ordering() {
        // technically there's only common.mpq and common-2.mpq, but test the ordering logic anyway
        let mut v = vec!["common-3.mpq", "common.mpq", "common-2.mpq"];
        v.sort_by(|a, b| MPQLoader::sorting_order(a, b));
        let expected = vec!["common.mpq", "common-2.mpq", "common-3.mpq"];
        assert_eq!(v, expected);
    }

    #[test]
    fn type_recognition() {
        let types = vec![
            ("base.mpq", MPQType::Base),
            ("alternate.mpq", MPQType::Alternate),
            ("patch-x.mpq", MPQType::Patch),
            ("patch-enUS-x.mpq", MPQType::PatchLocalized),
            ("patch.mpq", MPQType::Patch),
            ("patch-enUS.mpq", MPQType::PatchLocalized),
            ("expansion.mpq", MPQType::Tbc),
            ("lichking.mpq", MPQType::Wotlk),
            ("common.mpq", MPQType::Common),
            ("locale-enUS.mpq", MPQType::Locale),
            ("speech-enUS.mpq", MPQType::Speech),
            ("expansion-locale-enUS.mpq", MPQType::TbcLocale),
            ("lichking-locale-enUS.mpq", MPQType::WotlkLocale),
            ("expansion-speech-enUS.mpq", MPQType::TbcSpeech),
            ("lichking-speech-enUS.mpq", MPQType::WotlkSpeech),
            ("unknown.mpq", MPQType::Unknown),
        ];

        for (filename, expected_type) in types {
            assert_eq!(
                MPQLoader::extract_mpq_type(filename),
                expected_type,
                "Failed to recognize type for {}",
                filename
            );
        }
    }

    #[test]
    fn ordering_without_locale() {
        let expected = vec![
            "base.mpq",
            "alternate.mpq",
            "patch-3.mpq",
            "patch-2.mpq",
            "patch.mpq",
            "expansion.mpq",
            "lichking.mpq",
            "common.mpq",
            "common-2.mpq",
        ];
        let mut unsorted = expected.clone();
        unsorted.shuffle(&mut rand::rng());
        unsorted.sort_by(|a, b| MPQLoader::sorting_order(a, b));
        assert_eq!(unsorted, expected);
    }

    #[test]
    fn ordering_with_locale() {
        let expected = vec![
            "base.mpq",
            "alternate.mpq",
            "patch-3.mpq",
            "patch-2.mpq",
            "patch-enUS-3.mpq",
            "patch-enUS-2.mpq",
            "patch.mpq",
            "patch-enUS.mpq",
            "expansion.mpq",
            "lichking.mpq",
            "common.mpq",
            "common-2.mpq",
            "locale-enUS.mpq",
            "speech-enUS.mpq",
            "expansion-locale-enUS.mpq",
            "lichking-locale-enUS.mpq",
            "expansion-speech-enUS.mpq",
            "lichking-speech-enUS.mpq",
        ];
        let mut unsorted = expected.clone();
        unsorted.shuffle(&mut rand::rng());

        unsorted.sort_by(|a, b| MPQLoader::sorting_order(a, b));
        assert_eq!(unsorted, expected);
    }
}
