use crate::entity::components::rendering::{Renderable, RenderableSource};
use crate::entity::components::units::UnitDisplayId;
use crate::game::application::GameApplication;
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::load_dbc;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::m2_generator::M2Generator;
use crate::rendering::asset_graph::nodes::adt_node::{IRTexture, M2Node};
use crate::rendering::asset_graph::resolver::Resolver;
use clap::builder::TypedValueParser;
use hecs::Without;
use itertools::Itertools;
use log::{info, warn};
use sargerust_files::m2::types::M2TextureType;
use std::clone;
use std::fmt::format;
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use wow_dbc::wrath_tables::char_sections::{CharSections, CharSectionsRow};
use wow_dbc::wrath_tables::creature_display_info::CreatureDisplayInfo;
use wow_dbc::wrath_tables::creature_display_info_extra::{CreatureDisplayInfoExtra, CreatureDisplayInfoExtraRow};
use wow_dbc::wrath_tables::creature_model_data::CreatureModelData;
use wow_dbc::{DbcTable, Indexable};

pub struct DisplayIdResolverSystem {
    creature_display_info: CreatureDisplayInfo,
    creature_display_info_extra: CreatureDisplayInfoExtra,
    creature_model_data: CreatureModelData,
    char_sections: CharSections,
    m2_resolver: Resolver<M2Generator, M2Node>,
    tex_resolver: Resolver<M2Generator, RwLock<Option<IRTexture>>>,
}

impl DisplayIdResolverSystem {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            creature_display_info: load_dbc(&mpq_loader, "DBFilesClient\\CreatureDisplayInfo.dbc"),
            creature_display_info_extra: load_dbc(&mpq_loader, "DBFilesClient\\CreatureDisplayInfoExtra.dbc"),
            creature_model_data: load_dbc(&mpq_loader, "DBFilesClient\\CreatureModelData.dbc"),
            char_sections: load_dbc(&mpq_loader, "DBFilesClient\\CharSections.dbc"),
            m2_resolver: Resolver::new(M2Generator::new(mpq_loader.clone())),
            tex_resolver: Resolver::new(M2Generator::new(mpq_loader.clone())),
        }
    }

    pub fn update(&self, app: &GameApplication) {
        let mut write = app
            .entity_tracker
            .world()
            .write()
            .expect("World Lock poisoned");

        let mut new_renderables = vec![];

        for (entity, display_id) in write.query_mut::<Without<&UnitDisplayId, &Renderable>>() {
            // Freshly added entities
            let Some(creature_display_info) = self.creature_display_info.get(display_id.0) else {
                warn!("No CreatureDisplayInfo for DisplayId {}", display_id.0);
                continue;
            };

            let creature_display_info_extra_opt = self
                .creature_display_info_extra
                .get(creature_display_info.extended_display_info_id);

            let Some(creature_model_data) = self
                .creature_model_data
                .get(creature_display_info.model_id.id)
            else {
                warn!(
                    "No CreatureModelData for ModelId {}",
                    creature_display_info.model_id.id
                );
                continue;
            };

            const BASE_SECTION_FACE: i32 = 1;
            const BASE_SECTION_FACIAL_HAIR: i32 = 2;
            const BASE_SECTION_UNDERWEAR: i32 = 4;

            // fix name: currently it ends with .mdx, but we need .m2
            let name = creature_model_data
                .model_name
                .to_lowercase()
                .replace(".mdx", ".m2")
                .replace(".mdl", ".m2");

            let base_path = name.split_at(name.rfind('\\').expect("No \\ in name")).0;

            let result = self.m2_resolver.resolve(name.clone());

            for reference in &result.tex_reference {
                let resolve = self.tex_resolver.resolve(reference.reference_str.clone());
                *reference.reference.write().expect("Write Lock") = Some(resolve);
            }

            let resolved_dynamic_textures = result
                .dynamic_tex_references
                .iter()
                .filter_map(|reference| {
                    let tex_file_name = match reference.texture_type {
                        M2TextureType::TexComponentMonster1 => &creature_display_info.texture_variation[0],
                        M2TextureType::TexComponentMonster2 => &creature_display_info.texture_variation[1],
                        M2TextureType::TexComponentMonster3 => &creature_display_info.texture_variation[2],
                        M2TextureType::TexComponentSkin => {
                            let tex_opt = creature_display_info_extra_opt
                                .and_then(|cdie| get_base_skin_section(cdie, &self.char_sections))
                                .map(|section| &section.texture_name[0]);

                            if let Some(tex) = tex_opt {
                                tex
                            } else {
                                warn!("Failed to load texture type {:?}", reference.texture_type);
                                return None;
                            }
                        }
                        M2TextureType::TexComponentCharacterHair => {
                            let tex_opt = creature_display_info_extra_opt
                                .and_then(|cdie| get_hair_section(cdie, &self.char_sections))
                                .map(|section| &section.texture_name[0]);

                            if let Some(tex) = tex_opt {
                                tex
                            } else {
                                warn!("Failed to load texture type {:?}", reference.texture_type);
                                return None;
                            }
                        }
                        _ => {
                            warn!("Not supported texture type {:?}", reference.texture_type);
                            return None;
                        }
                    };

                    // TODO: Is this just a hack and we should instead do this string format only for the monster tex types?
                    let tex_name = if tex_file_name.ends_with(".blp") {
                        tex_file_name.to_string()
                    } else {
                        format!("{}\\{}.blp", base_path, tex_file_name)
                    };

                    Some((
                        reference.texture_type,
                        reference.texture_flags,
                        self.tex_resolver.resolve(tex_name.clone()),
                    ))
                })
                .collect_vec();

            new_renderables.push((entity, (result, resolved_dynamic_textures)));
        }

        for (entity, (arc, dynamic_textures)) in new_renderables {
            write
                .insert_one(
                    entity,
                    Renderable {
                        handles: None,
                        source: RenderableSource::M2(arc, dynamic_textures),
                    },
                )
                .expect("Insert Renderable");
        }
    }
}

fn get_base_skin_section<'a>(
    creature_display_info_extra: &CreatureDisplayInfoExtraRow,
    char_sections: &'a CharSections,
) -> Option<&'a CharSectionsRow> {
    const BASE_SECTION_BASE_SKIN: i32 = 0;
    let sections = char_sections
        .rows()
        .iter()
        .filter(|section| {
            section.race_id == creature_display_info_extra.display_race_id
                && section.sex_id == creature_display_info_extra.display_sex_id
                && section.base_section == BASE_SECTION_BASE_SKIN
                && section.color_index == creature_display_info_extra.skin_id
        })
        .collect_vec();

    if sections.len() > 1 {
        warn!("CharacterSection was not unique ");
        None
    } else if sections.is_empty() {
        warn!("Could not find any matching CharacterSection");
        None
    } else {
        Some(sections[0])
    }
}

fn get_hair_section<'a>(
    creature_display_info_extra: &CreatureDisplayInfoExtraRow,
    char_sections: &'a CharSections,
) -> Option<&'a CharSectionsRow> {
    const BASE_SECTION_HAIR: i32 = 3;
    let sections = char_sections
        .rows()
        .iter()
        .filter(|section| {
            section.race_id == creature_display_info_extra.display_race_id
                && section.sex_id == creature_display_info_extra.display_sex_id
                && section.base_section == BASE_SECTION_HAIR
                && section.variation_index == creature_display_info_extra.hair_style_id
                && section.color_index == creature_display_info_extra.hair_color_id
        })
        .collect_vec();

    if sections.len() > 1 {
        warn!("CharacterSection was not unique ");
        None
    } else if sections.is_empty() {
        warn!("Could not find any matching CharacterSection");
        None
    } else {
        Some(sections[0])
    }
}
