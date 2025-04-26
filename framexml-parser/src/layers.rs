use crate::typedefs::LayerItems;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct LayersType {
    #[serde(rename = "$value")]
    pub elements: Vec<Layer>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum DrawLayer {
    #[default] // caution: This default in xsd depends on where it's being used
    Artwork,
    Background,
    Border,
    Highlight,
    Overlay,
}

#[derive(Deserialize, Debug)]
pub struct Layer {
    #[serde(rename = "@level")]
    #[serde(default)]
    pub level: DrawLayer,
    #[serde(rename = "$value")]
    pub elements: Vec<LayerItems>,
}
