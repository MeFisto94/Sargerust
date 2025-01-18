use rust_embed::RustEmbed;

pub mod terrain;
pub mod units;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/shaders/src"]
pub struct SargerustShaderSources;
