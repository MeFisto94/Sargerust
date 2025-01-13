use crate::entity::components::render::{Renderable, RenderableSource};
use crate::entity::components::units::UnitDisplayId;
use crate::game::application::GameApplication;
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::m2_generator::M2Generator;
use crate::rendering::asset_graph::nodes::adt_node::{IRTexture, M2Node};
use crate::rendering::asset_graph::resolver::Resolver;
use hecs::Without;
use log::{info, warn};
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

            info!("Got a {}", &name);
            let result = self.m2_resolver.resolve(name);

            for reference in &result.tex_reference {
                let resolve = self.tex_resolver.resolve(reference.reference_str.clone());
                *reference.reference.write().expect("Write Lock") = Some(resolve);
            }

            warn!(
                "Result: Tex {:?}, Vertex {}, material: {:?}",
                result.tex_reference,
                result
                    .mesh
                    .read()
                    .unwrap()
                    .data
                    .vertex_buffers
                    .position_buffer
                    .len(),
                result.material.read().unwrap().data
            );
            new_renderables.push((entity, result));
        }

        for (entity, arc) in new_renderables {
            write
                .insert_one(
                    entity,
                    Renderable {
                        handle: None,
                        source: RenderableSource::M2(arc),
                    },
                )
                .expect("Insert Renderable");
        }
    }
}
