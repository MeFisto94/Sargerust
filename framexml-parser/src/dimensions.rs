use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SizeType {
    #[serde(rename = "$value", default)]
    pub dimensions: Vec<Dimensions>,
    #[serde(rename = "@x")]
    pub x: Option<i32>,
    #[serde(rename = "@y")]
    pub y: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub enum Dimensions {
    AbsDimension {
        #[serde(rename = "@x")]
        x: u32,
        #[serde(rename = "@y")]
        y: u32,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct ResizeBounds {
    #[serde(rename = "minResize")]
    pub min_resize: Option<Dimensions>,
    #[serde(rename = "maxResize")]
    pub max_resize: Option<Dimensions>,
}

#[derive(Deserialize, Debug)]
pub struct Inset {
    #[serde(rename = "AbsInset")]
    pub abs_inset: Option<AbsInset>,
    #[serde(rename = "RelInset")]
    pub rel_inset: Option<RelInset>,

    pub left: Option<i32>,
    pub right: Option<i32>,
    pub top: Option<i32>,
    pub bottom: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct AbsInset {
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub top: Option<i32>,
    pub bottom: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct RelInset {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Deserialize, Debug)]
pub struct InnerValue {
    #[serde(rename = "@val")]
    pub val: i32,
}

#[derive(Deserialize, Debug)]
pub struct Value {
    #[serde(rename = "AbsValue")]
    pub abs_value: Option<InnerValue>,
    #[serde(rename = "RelValue")]
    pub rel_value: Option<InnerValue>,
    #[serde(rename = "@val")]
    pub val: Option<i32>,
}
