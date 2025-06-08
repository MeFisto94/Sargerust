use framexml_parser::scripts::ScriptItemType;
use log::{error, warn};
use mlua::{Function, Lua};

#[derive(Debug, Default)]
pub struct ScriptManager {
    lua: Lua,
}

impl ScriptManager {
    pub fn execute_script(&self, script: &str, name: &str) {
        if let Err(e) = self.lua.load(script).set_name(name).exec() {
            error!("Failed to execute script: {}", e);
        }
    }

    pub fn execute_script_raw(&self, script: &[u8], name: &str) {
        let chunk = self.lua.load(script).set_name(name);
        if let Err(e) = chunk.exec() {
            error!("Failed to execute script: {}", e);
        }
    }

    pub fn parse_function(&self, script: &str) -> Function {
        self.lua.load(script).into_function().unwrap()
    }
}

#[derive(Debug)]
pub enum ScriptContent {
    FunctionReference(String),
    Text(String),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum ScriptEvent {
    OnLoad,
}

pub fn convert_script_item(item: ScriptItemType) -> Result<ScriptContent, ()> {
    if item.content.is_some() && item.function.is_some() {
        warn!(
            "Script Item has both: Content and Function Reference (to {}). Choosing Content.",
            item.function.as_ref().unwrap()
        );
    }

    if let Some(content) = item.content {
        return Ok(ScriptContent::Text(content));
    }

    if let Some(function) = item.function {
        return Ok(ScriptContent::FunctionReference(function));
    }

    error!("Empty Script Item");
    Err(())
}
