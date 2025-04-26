use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Attributes {
    // TODO: if we'd add an enum that only contains "Attribute", would this help, i.e. are we currently parsing every tag as Attribute?
    #[serde(rename = "$value")]
    pub elements: Vec<AttributeType>,
}

#[derive(Deserialize, Debug)]
pub struct AttributeType {
    #[serde(rename = "@name")]
    pub name: Option<String>,
    #[serde(rename = "@type", default)]
    pub attr_type: AttributeTypeType,
    #[serde(rename = "@value")]
    pub value: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum AttributeTypeType {
    #[default]
    String,
    Number,
    Boolean,
    Nil,
}
