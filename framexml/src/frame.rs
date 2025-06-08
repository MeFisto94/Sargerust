use crate::scripts::ScriptEvent;
use framexml_parser::scripts::ScriptItem;
use mlua::Function;
use stackmap::StackMap;
use std::collections::HashMap;

pub enum FrameType {
    Frame,
    Button,
}

impl TryFrom<&str> for FrameType {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.eq_ignore_ascii_case("frame") {
            Ok(FrameType::Frame)
        } else if value.eq_ignore_ascii_case("button") {
            Ok(FrameType::Button)
        } else {
            Err(())
        }
    }
}

pub enum FrameAttribute {
    String(String),
    Number(f64),
    Boolean(bool),
    Nil,
}

pub struct Frame {
    pub name: Option<String>,
    pub parent: Option<String>, // TODO: ARCs?
    pub frame_type: FrameType,
    pub inherits: Option<String>,
    pub attributes: HashMap<String, FrameAttribute>,
    pub scripts: Vec<ScriptItem>, // TODO: Probably want rlua types
    event_scripts: StackMap<ScriptEvent, Function, { std::mem::variant_count::<ScriptEvent>() }>,
}

impl Frame {
    pub fn new(
        frame_type: FrameType,
        name: Option<String>,
        parent: Option<String>, /* TODO*/
        inherits: Option<String>,
    ) -> Self {
        Frame {
            name,
            parent,
            frame_type,
            inherits,
            attributes: HashMap::new(),
            scripts: Vec::new(),
            event_scripts: StackMap::new(),
        }
    }

    pub fn on_load(&self) -> Result<(), mlua::Error> {
        let Some(function) = self.event_scripts.get(&ScriptEvent::OnLoad) else {
            return Ok(());
        };

        function.call::<()>(())?;

        Ok(())
    }
}

#[derive(Default)]
pub struct FrameManager {
    frames: Vec<Frame>, // We can't HashMap as the name is not mandatory.
}

impl FrameManager {
    pub fn new() -> Self {
        FrameManager::default()
    }

    pub fn register_frame(&mut self, frame: Frame) -> &Frame {
        self.frames.push(frame);
        self.frames
            .last()
            .expect("Frame manager can't be empty, we just pushed")
    }

    pub(crate) fn nb_frames(&self) -> usize {
        self.frames.len()
    }
}
