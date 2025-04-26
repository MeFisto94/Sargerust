use crate::dimensions::{Inset, Value};
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
    Button(FrameType),
    Include {
        #[serde(rename = "@file")]
        file: String,
    },
    Frame(FrameType),
    ModelFFX(()),
    #[serde(other)]
    Unknown,
}

// Macros to work around #[serde(flatten)] not working
macro_rules! layout_frame_type {
    ($vis:vis struct $name:ident { $($fields:tt)* }) => {
    #[derive(Deserialize, Debug)]
    $vis struct $name {
        // LayoutFrameType Start
        #[serde(rename = "@name")]
        pub name: Option<String>,
        #[serde(rename = "Scripts")]
        pub scripts: Option<crate::scripts::ScriptsType>,
        #[serde(rename = "Size")]
        pub size: Option<crate::dimensions::SizeType>,
        #[serde(rename = "Anchors")]
        pub anchors: Option<crate::anchor::AnchorsType>,
        #[serde(rename = "Layers")]
        pub layers: Option<crate::layers::LayersType>,
        // LayoutFrameType End
        $($fields)*
    }
    };
}

macro_rules! frame_type {
    ($vis:vis struct $name:ident { $($fields:tt)* }) => {
    layout_frame_type!(
    $vis struct $name {
    // FrameType Start
    #[serde(rename = "Frames")]
    pub frames: Option<Frames>,

    // TODO: Is this the right type?
    #[serde(rename = "TitleRegion")]
    pub title_region: Option<LayoutFrameType>,

    #[serde(rename = "Backdrop")]
    pub backdrop: Option<BackdropType>,

    #[serde(rename = "ResizeBounds")]
    pub resize_bounds: Option<crate::dimensions::ResizeBounds>,

    #[serde(rename = "HitRectInsets")]
    pub hit_rect_insets: Option<Inset>,

    #[serde(rename = "Attributes")]
    pub attributes: Option<crate::attributes::Attributes>,
    // FrameType End
    $($fields)*
    });
    };
}

layout_frame_type!(
    pub struct LayoutFrameType {}
);

frame_type!(
    pub struct FrameType {}
);

frame_type!(
    pub struct ButtonType {
        #[serde(rename = "@text")]
        pub text: Option<String>,
        #[serde(rename = "@registerForClicks")]
        pub register_for_clicks: Option<String>,
        #[serde(rename = "@motionScriptsWhileDisabled", default = "bool_false")]
        pub motion_scripts_while_disabled: bool,
        // TODO: 						<xs:element name="NormalTexture" type="ui:TextureType"/>
        // 						<xs:element name="PushedTexture" type="ui:TextureType"/>
        // 						<xs:element name="DisabledTexture" type="ui:TextureType"/>
        // 						<xs:element name="HighlightTexture" type="ui:TextureType"/>
        // 						<xs:element name="ButtonText" type="FontStringType"/>
        // 						<xs:element name="NormalFont" type="ButtonStyleType"/>
        // 						<xs:element name="HighlightFont" type="ButtonStyleType"/>
        // 						<xs:element name="DisabledFont" type="ButtonStyleType"/>
        // 						<xs:element name="NormalColor" type="ColorType"/>
        // 						<xs:element name="HighlightColor" type="ColorType"/>
        // 						<xs:element name="DisabledColor" type="ColorType"/>
        // 						<xs:element name="PushedTextOffset" type="Dimension"/>
    }
);

#[derive(Deserialize, Debug)]
pub struct Frames {
    // TODO: if we'd add an enum that only contains "Frame", would this help, i.e. are we currently parsing every tag as Frame?
    #[serde(rename = "$value")]
    pub elements: Vec<FrameType>,
}

#[derive(Deserialize, Debug)]
pub struct BackdropType {
    #[serde(rename = "BackgroundInsets")]
    pub background_insets: Option<Inset>,
    #[serde(rename = "TileSize")]
    pub tile_size: Option<Value>,
    #[serde(rename = "EdgeSize")]
    pub edge_size: Option<Value>,
    #[serde(rename = "Color")]
    pub color: Option<ColorType>,
    #[serde(rename = "BorderColor")]
    pub border_color: Option<ColorType>,
}

#[derive(Deserialize, Debug)]
pub struct ColorType {
    #[serde(rename = "@r")]
    pub r: f32,
    #[serde(rename = "@g")]
    pub g: f32,
    #[serde(rename = "@b")]
    pub b: f32,
    #[serde(rename = "@a", default = "float_one")]
    pub a: f32,
}

#[inline(always)]
const fn float_one() -> f32 {
    1.0
}

#[inline(always)]
const fn bool_false() -> bool {
    false
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

layout_frame_type!(
    pub struct FontStringType {
        #[serde(rename = "@font")]
        pub font: Option<String>,

        #[serde(rename = "@text")]
        pub text: Option<String>,

        #[serde(rename = "@justifyH", default)]
        pub justify_h: JustifyHType,

        #[serde(rename = "@justifyV", default)]
        pub justify_v: JustifyVType,

        // TODO: is this the right type?
        #[serde(rename = "$value")]
        #[serde(default)]
        pub elements: Vec<FontStringType>,
    }
);

layout_frame_type!(
    pub struct TextureType {
        #[serde(rename = "@file")]
        pub file: String,
    }
);

#[derive(Deserialize, Debug)]
pub enum LayerItems {
    FontString(FontStringType),
    Texture(TextureType),
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
