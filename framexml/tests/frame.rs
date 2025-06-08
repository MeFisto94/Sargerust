use framexml_parser::scripts::{ScriptItem, ScriptItemType, ScriptsType};
use framexml_parser::typedefs::FrameType;
use framexml_parser::typedefs::UiItem::Frame;

#[test]
fn on_load() {
    let frame = Frame(FrameType {
        name: Some("TestFrame".to_string()),
        scripts: Some(ScriptsType {
            elements: vec![ScriptItem::OnLoad(ScriptItemType {
                content: Some("print(\"Hello World!\");".to_string()),
                function: None,
            })],
        }),
        size: None,
        anchors: None,
        layers: None,
        frames: None,
        title_region: None,
        backdrop: None,
        resize_bounds: None,
        hit_rect_insets: None,
        attributes: None,
    });
}
