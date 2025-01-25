use std::ops::Deref;

use image_blp::BlpImage;
use image_blp::convert::blp_to_image;

pub mod application;
pub mod asset_graph;
pub mod common;
pub mod importer;
pub mod loader;
pub mod rend3_backend;

fn create_texture_rgba8(blp: &BlpImage, mipmap_level: usize) -> rend3::types::Texture {
    let image = blp_to_image(blp, mipmap_level).expect("decode");
    let image_dims = glam::UVec2::new(image.width(), image.height());
    let image_data = image.into_rgba8();

    rend3::types::Texture {
        label: None,
        data: image_data.into_raw(),
        format: rend3::types::TextureFormat::Rgba8UnormSrgb,
        size: image_dims,
        mip_count: rend3::types::MipmapCount::ONE,
        mip_source: rend3::types::MipmapSource::Uploaded,
    }
}
