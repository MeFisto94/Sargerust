use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Ui {
    #[serde(rename = "$value")]
    pub elements: Vec<UiItem>,
}

#[derive(Deserialize, Debug)]
pub enum UiItem {
    // TODO: both Script and Include could carry a script as their $value?/content
    Script {
        #[serde(rename = "@file")]
        file: String,
    },
    Include {
        #[serde(rename = "@file")]
        file: String,
    },
    Frame(LayoutFrameType),

    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct LayoutFrameType {
    #[serde(rename = "Scripts")]
    pub scripts: Option<ScriptsType>,
    #[serde(rename = "Size")]
    pub size: Option<SizeType>,
    #[serde(rename = "Anchors")]
    pub anchors: Option<AnchorsType>,
    #[serde(rename = "Layers")]
    pub layers: Option<LayersType>,
}

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

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum JustifyHType {
    Left,
    #[default] // caution: This default in xsd depends on where it's being used
    Center,
    Right,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum JustifyVType {
    Top,
    #[default] // caution: This default in xsd depends on where it's being used
    Middle,
    Bottom,
}

#[derive(Deserialize, Debug)]
pub enum LayerItems {
    FontString {
        #[serde(rename = "@name")]
        name: String,
        #[serde(rename = "@inherits")]
        inherits: String,
        #[serde(rename = "@justifyH")]
        justify_h: JustifyHType,
        #[serde(rename = "@hidden")]
        hidden: bool,

        // TODO: is this the best type?
        #[serde(rename = "$value")]
        #[serde(default)]
        elements: Vec<LayoutFrameType>,
    },
}

// TODO: Get rid of these list wrappers if we don't need access to attributes on the wrapper list level.
#[derive(Deserialize, Debug)]
pub struct AnchorsType {
    #[serde(rename = "$value")]
    pub elements: Vec<Anchor>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum FramePoint {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

#[derive(Deserialize, Debug)]
pub struct Anchor {
    #[serde(rename = "@point")]
    pub point: FramePoint,
    #[serde(rename = "@relativeTo")]
    pub relative_to: String, // name of another frame
    #[serde(rename = "@relativePoint")]
    pub relative_point: FramePoint,
}

#[derive(Deserialize, Debug)]
pub struct SizeType {
    #[serde(rename = "$value")]
    pub elements: Vec<Dimensions>,
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
pub struct ScriptsType {
    #[serde(rename = "$value")]
    pub elements: Vec<ScriptItem>,
}

#[derive(Deserialize, Debug)]
pub enum ScriptItem {
    OnLoad {
        #[serde(rename = "@function")]
        function: String,
    },
    OnEvent {
        #[serde(rename = "@function")]
        function: String,
    },
    OnUpdate {
        #[serde(rename = "@function")]
        function: String,
    },
    #[serde(other)]
    Unknown,
}
