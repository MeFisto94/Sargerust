use crate::entity::components::rendering::{Renderable, RenderableSource};
use crate::entity::components::units::UnitDisplayId;
use crate::game::application::GameApplication;
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::m2_generator::M2Generator;
use crate::rendering::asset_graph::nodes::adt_node::{IRTexture, M2Node};
use crate::rendering::asset_graph::resolver::Resolver;
use hecs::Without;
use itertools::Itertools;
use log::warn;
use sargerust_files::m2::types::M2TextureType;
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use wow_dbc::wrath_tables::creature_display_info::CreatureDisplayInfo;
use wow_dbc::wrath_tables::creature_model_data::CreatureModelData;
use wow_dbc::{DbcTable, Indexable};

pub struct DisplayIdResolverSystem {
    creature_display_info: CreatureDisplayInfo,
    creature_model_data: CreatureModelData,
    m2_resolver: Resolver<M2Generator, M2Node>,
    tex_resolver: Resolver<M2Generator, RwLock<Option<IRTexture>>>,
}

impl DisplayIdResolverSystem {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        let cdi_buf = mpq_loader
            .load_raw_owned("DBFilesClient\\CreatureDisplayInfo.dbc")
            .expect("Failed to load CreatureDisplayInfo.dbc");

        let cmi_buf = mpq_loader
            .load_raw_owned("DBFilesClient\\CreatureModelData.dbc")
            .expect("Failed to load CreatureModelData.dbc");

        let creature_display_info =
            CreatureDisplayInfo::read(&mut Cursor::new(cdi_buf)).expect("Failed to parse Creature Display Info");

        let creature_model_data =
            CreatureModelData::read(&mut Cursor::new(cmi_buf)).expect("Failed to parse Creature Model Info");

        Self {
            creature_display_info,
            creature_model_data,
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
                        _ => {
                            warn!("Not supported texture type {:?}", reference.texture_type);
                            return None;
                        }
                    };

                    let tex_name = format!("{}\\{}.blp", base_path, tex_file_name);

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
