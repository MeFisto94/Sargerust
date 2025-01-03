use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use image_blp::BlpImage;
use image_blp::parser::parse_blp_with_externals;
use log::{error, warn};

pub struct BLPLoader {}

impl BLPLoader {
    pub fn load_blp_from_ldr(mpq_loader: &MPQLoader, file_name: &str) -> Option<BlpImage> {
        // TODO: The blp crate has bad error handling, as it doesn't mix with anyhow::Error.
        // furthermore, the built in error types stem from nom, that we don't have as dependency.

        // load_blp uses the fs to load mip maps next to it.
        // we don't want to extract blps into temporary files, though, so we use the other API
        // and there, we either don't support BLP0 Mipmaps or we properly implement the callback at some time

        let owned_file = mpq_loader.load_raw_owned(file_name);
        if owned_file.is_none() {
            warn!("Could not load BLP {}", file_name);
            return None;
        }

        let root_input = owned_file.unwrap();
        let image = parse_blp_with_externals(&root_input, |_i| {
            // This could also be no_mipmaps from the image-blp parser crate.
            panic!("Loading of BLP Mip Maps is unsupported. File {}", file_name)
        });

        if image.is_err() {
            error!(
                "Parsing of the BLP {file_name} failed: {}",
                image.unwrap_err()
            );
            return None;
        }
        Some(image.unwrap().1)
    }
}
