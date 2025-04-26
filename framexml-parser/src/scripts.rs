use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ScriptsType {
    #[serde(rename = "$value")]
    pub elements: Vec<ScriptItem>,
}

#[derive(Deserialize, Debug)]
pub struct ScriptItemType {
    #[serde(rename = "@function")]
    pub function: Option<String>,
    #[serde(rename = "$text")]
    pub content: Option<String>,
}

#[derive(Deserialize, Debug)]
pub enum ScriptItem {
    OnLoad(ScriptItemType),
    OnEvent(ScriptItemType),
    OnUpdate(ScriptItemType),
    OnClick(ScriptItemType),
    OnDoubleClick(ScriptItemType),
    OnEnable(ScriptItemType),
    OnDisable(ScriptItemType),
    OnShow(ScriptItemType),
    OnHide(ScriptItemType),
    OnKeyDown(ScriptItemType),
    #[serde(other)]
    Unknown,
}
