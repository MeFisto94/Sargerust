use crate::frame::{Frame, FrameManager, FrameType};
use crate::scripts::ScriptManager;
use framexml_parser::toc::TocFile;
use framexml_parser::typedefs::{Ui, UiItem};
use log::{debug, info, trace, warn};

pub struct AddonInfo {
    pub interface_version: Option<usize>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub author: Option<String>,
}

impl From<&TocFile> for AddonInfo {
    fn from(toc: &TocFile) -> Self {
        AddonInfo {
            interface_version: toc.directives.get("Interface").and_then(|s| s.parse().ok()),
            title: toc.directives.get("Title").cloned(), // TODO: some directives can be localized (e.g. Title-frFR)
            version: toc.directives.get("Version").cloned(),
            notes: toc.directives.get("Notes").cloned(),
            author: toc.directives.get("Author").cloned(),
        }
    }
}

pub struct LoadedAddon {
    script_manager: ScriptManager,
    info: AddonInfo,
}

pub struct AddonManager<F>
where
    F: Fn(&str) -> Option<Vec<u8>>,
{
    asset_loader: F,
    frame_manager: FrameManager,
}

impl<F: Fn(&str) -> Option<Vec<u8>>> AddonManager<F> {
    pub fn new(asset_loader: F) -> Self {
        AddonManager {
            asset_loader,
            frame_manager: FrameManager::new(),
        }
    }

    pub fn load_addon(&mut self, path: &str) -> LoadedAddon {
        let script_manager = ScriptManager::default();

        script_manager.execute_script(include_str!("../lua/stubs.lua"), "=builtin-stubs");

        trace!("Loading addon {}", path);

        let toc_path = format!("{}\\{}.toc", path, path);
        let toc_buf_opt = (self.asset_loader)(&toc_path);
        let Some(toc_buf) = toc_buf_opt else {
            warn!(
                "Could not load addon {}, as {} was missing",
                path, &toc_path
            );
            // TODO: Error handling
            panic!("Failed to load file: {}", &toc_path);
        };

        let toc = TocFile::parse_file(toc_buf.as_slice()).unwrap(); // TODO: Error handling

        for file in &toc.files {
            let file_path = format!("{}\\{}", path, file);

            // We at least need to differentiate between Lua and Xml. Luas are just smashed into the (global?) space.
            if file.ends_with(".lua") {
                // TODO: case sensitivity?
                debug!("Loading lua file: {}", file_path);
                let lua_buf_opt = (self.asset_loader)(&file_path);
                let Some(lua_buf) = lua_buf_opt else {
                    warn!("Missing {} of addon {}", file_path, path);
                    continue;
                };

                script_manager.execute_script_raw(&lua_buf, &format!("@{}", file_path));
            } else if file.ends_with(".xml") {
                self.load_xml_file(path, &script_manager, &file_path);
            } else {
                info!("Skipping unknown file: {}", file_path);
            }
        }

        info!(
            "Loaded {} Frames for addon {}",
            self.frame_manager.nb_frames(),
            path
        );

        LoadedAddon {
            info: AddonInfo::from(&toc),
            script_manager,
        }
    }

    fn load_xml_file(&mut self, folder: &str, script_manager: &ScriptManager, file_path: &String) {
        // TODO: Case sensitivity
        debug!("Loading xml file: {}", file_path);

        let frame_xml_buf_opt = (self.asset_loader)(file_path);
        let Some(frame_xml_buf) = frame_xml_buf_opt else {
            warn!("Missing {} of addon {}", file_path, folder);
            return;
        };

        let frame_xml_result = framexml_parser::deserialize_xml(frame_xml_buf.as_slice());
        let Ok(frame_xml) = frame_xml_result else {
            warn!(
                "Failed to parse XML file {}: {}",
                file_path,
                frame_xml_result.unwrap_err()
            );
            return;
        };

        // TODO: This may belong into a struct. "FrameManager"?
        for element in frame_xml.elements {
            match element {
                UiItem::Include { file } => {
                    trace!("Including XML {} for {}", file, file_path);
                    let include_path = format!("{}\\{}", folder, file); // TODO: Includes can be relative, what do we do if file_path is already a subfolder of folder? We'd need to get the parent.
                    self.load_xml_file(folder, script_manager, &include_path);
                }
                UiItem::Script { file, content } => {
                    if let Some(file) = file {
                        trace!("Including Script {} for {}", file, file_path);

                        let lua_script_path = format!("{}\\{}", folder, file);
                        let Some(lua_script_buf) = (self.asset_loader)(&lua_script_path) else {
                            warn!("Missing {} of addon {}", file, folder);
                            continue;
                        };

                        script_manager.execute_script_raw(&lua_script_buf, &format!("@{}", lua_script_path));
                    } else if let Some(content) = content {
                        trace!("Script content for {}", file_path);
                        script_manager.execute_script(&content, &format!("@{}.inline.lua", file_path));
                    }
                }
                UiItem::Frame(xml_frame) => {
                    trace!(
                        "Frame {} for {}",
                        xml_frame.name.as_deref().unwrap_or("Unnamed"),
                        file_path
                    );

                    let frame = Frame::new(FrameType::Frame, xml_frame.name, None, None);
                    self.frame_manager.register_frame(frame);
                }
                _ => {}
            }
        }
    }
}
