use crate::typedefs::FramePoint;
use serde_derive::Deserialize;

// TODO: setAllPoints -> TopLeft and BottomRight

#[derive(Deserialize, Debug)]
pub struct Anchor {
    #[serde(rename = "@point")]
    pub point: FramePoint,
    #[serde(rename = "@relativeTo")]
    pub relative_to: Option<String>, // name of another frame
    #[serde(rename = "@relativePoint")]
    pub relative_point: Option<FramePoint>,
    // TODO: Some blizz UI (e.g. FrameXML\PVPFrame.xml) has x="", so this'd fail, we may need to resort to string....
    #[serde(rename = "@x")]
    pub x: Option<f32>,
    #[serde(rename = "@y")]
    pub y: Option<f32>,
    #[serde(rename = "$value", default)]
    pub children: Vec<AnchorChild>,
}

#[derive(Deserialize, Debug)]
pub enum AnchorChild {
    Offset, // TODO: parse.
}

// TODO: Get rid of these list wrappers if we don't need access to attributes on the wrapper list level.
#[derive(Deserialize, Debug)]
pub struct AnchorsType {
    #[serde(rename = "$value")]
    pub elements: Vec<Anchor>,
}
