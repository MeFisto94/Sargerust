use crate::entity::components::rendering::{Renderable, RenderableSource};
use crate::entity::components::units::UnitDisplayId;
use crate::game::application::GameApplication;
use crate::io::mpq::load_dbc;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::m2_generator::M2Generator;
use crate::rendering::asset_graph::nodes::adt_node::M2Node;
use crate::rendering::asset_graph::resolver::Resolver;
use crate::rendering::rend3_backend::IRTexture;
use hecs::Without;
use itertools::{Itertools, enumerate};
use log::{info, warn};
use sargerust_files::m2::types::M2TextureType;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use wow_dbc::wrath_tables::char_hair_geosets::{CharHairGeosets, CharHairGeosetsRow};
use wow_dbc::wrath_tables::char_sections::{CharSections, CharSectionsRow};
use wow_dbc::wrath_tables::character_facial_hair_styles::CharacterFacialHairStyles;
use wow_dbc::wrath_tables::creature_display_info::CreatureDisplayInfo;
use wow_dbc::wrath_tables::creature_display_info_extra::{CreatureDisplayInfoExtra, CreatureDisplayInfoExtraRow};
use wow_dbc::wrath_tables::creature_model_data::CreatureModelData;
use wow_dbc::wrath_tables::item_display_info::ItemDisplayInfo;
use wow_dbc::{DbcTable, Indexable};

pub struct DisplayIdResolverSystem {
    creature_display_info: CreatureDisplayInfo,
    creature_display_info_extra: CreatureDisplayInfoExtra,
    creature_model_data: CreatureModelData,
    char_sections: CharSections,
    char_hair_geosets: CharHairGeosets,
    char_facial_hair_styles: CharacterFacialHairStyles,
    item_display_info: ItemDisplayInfo,
    m2_resolver: Resolver<M2Generator, M2Node>,
    tex_resolver: Resolver<M2Generator, RwLock<Option<IRTexture>>>,
}

const BASE_SECTION_BASE_SKIN: i32 = 0;
const BASE_SECTION_FACE: i32 = 1;
const BASE_SECTION_FACIAL_HAIR: i32 = 2;
const BASE_SECTION_HAIR: i32 = 3;
const BASE_SECTION_UNDERWEAR: i32 = 4;

impl DisplayIdResolverSystem {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            creature_display_info: load_dbc(&mpq_loader, "DBFilesClient\\CreatureDisplayInfo.dbc"),
            creature_display_info_extra: load_dbc(&mpq_loader, "DBFilesClient\\CreatureDisplayInfoExtra.dbc"),
            creature_model_data: load_dbc(&mpq_loader, "DBFilesClient\\CreatureModelData.dbc"),
            char_sections: load_dbc(&mpq_loader, "DBFilesClient\\CharSections.dbc"),
            char_hair_geosets: load_dbc(&mpq_loader, "DBFilesClient\\CharHairGeosets.dbc"),
            char_facial_hair_styles: load_dbc(&mpq_loader, "DBFilesClient\\CharacterFacialHairStyles.dbc"),
            item_display_info: load_dbc(&mpq_loader, "DBFilesClient\\ItemDisplayInfo.dbc"),
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

            let base_skin_cdie_opt =
                creature_display_info_extra_opt.and_then(|cdie| get_base_skin_section(cdie, &self.char_sections));

            let hair_cdie_opt = creature_display_info_extra_opt
                .and_then(|cdie| get_hair_section(cdie, &self.char_hair_geosets, &self.char_sections));

            let face_cdie_opt =
                creature_display_info_extra_opt.and_then(|cdie| get_face_section(cdie, &self.char_sections));

            let facial_hair_cdie_opt = creature_display_info_extra_opt
                .and_then(|cdie| get_facial_hair_section(cdie, &self.char_sections, &self.char_facial_hair_styles));

            let underwear_cdie_opt =
                creature_display_info_extra_opt.and_then(|cdie| get_underwear_section(cdie, &self.char_sections));

            let items_opt = creature_display_info_extra_opt.map(|cdie| {
                cdie.n_p_c_item_display
                    .map(|id| self.item_display_info.get(id))
            });

            let resolved_dynamic_textures = result
                .dynamic_tex_references
                .iter()
                .filter_map(|reference| {
                    let tex_file_name = match reference.texture_type {
                        M2TextureType::TexComponentMonster1 => creature_display_info.texture_variation[0].to_string(),
                        M2TextureType::TexComponentMonster2 => creature_display_info.texture_variation[1].to_string(),
                        M2TextureType::TexComponentMonster3 => creature_display_info.texture_variation[2].to_string(),
                        // M2TextureType::TexComponentSkin => {
                        //     let tex_opt = base_skin_cdie_opt.map(|section| &section.texture_name[0]);
                        //
                        //     if let Some(tex) = tex_opt {
                        //         tex.to_string()
                        //     } else {
                        //         warn!("Failed to load texture type {:?}", reference.texture_type);
                        //         return None;
                        //     }
                        // }
                        M2TextureType::TexComponentCharacterHair => {
                            let tex_opt = hair_cdie_opt.map(|section| &section.1.texture_name[0]);

                            if let Some(tex) = tex_opt {
                                tex.to_string()
                            } else {
                                warn!("Failed to load texture type {:?}", reference.texture_type);
                                return None;
                            }
                        }
                        M2TextureType::TexComponentSkin | M2TextureType::TexComponentObjectSkin => {
                            // Use the baked texture for non-players (it's faster and easier).
                            let tex_opt = creature_display_info_extra_opt
                                .map(|cdie| format!("textures\\BakedNpcTextures\\{}", cdie.bake_name));

                            if let Some(tex) = tex_opt {
                                // This is the reason we can't return &String, even if it's later to_stringed and formatted anyway...
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

                    if tex_file_name == "" {
                        return None;
                    }

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

            let mut geoset_whitelist = HashSet::<u16>::new();

            // https://gitlab.com/T1ti/noggit-red/-/blob/56baf6ed038c82bc2345e605ed246cedbaf6a53a/src/noggit/Model.cpp#L884

            // TODO: The following two. We have conflicts with other sections. Also underwear?? Interestingly,
            //  face variation should correlate with facial_hairs geoset[0]
            // if let Some(base_skin_sections) = base_skin_cdie_opt {
            //     // This is tricky because this seems to essentially set "0000" regardless of the variation, where hair starts at 0001
            //     geoset_whitelist.insert((BASE_SECTION_BASE_SKIN * 100 + base_skin_sections.variation_index + 1) as u16);
            // }
            //
            // if let Some(face_sections) = face_cdie_opt {
            //     geoset_whitelist.insert((BASE_SECTION_FACE * 100 + face_sections.variation_index + 1) as u16);
            // }

            if let Some((geosets, _facial_hair_sections)) = facial_hair_cdie_opt {
                geoset_whitelist.insert((100 + geosets[0]) as u16);
                geoset_whitelist.insert((200 + geosets[1]) as u16);
                geoset_whitelist.insert((300 + geosets[2]) as u16);
                geoset_whitelist.insert((1600 + geosets[3]) as u16);
                geoset_whitelist.insert((1700 + geosets[4]) as u16);
            }

            if let Some(hair_sections) = hair_cdie_opt {
                geoset_whitelist.insert(hair_sections.0.geoset_id as u16);
            }

            if let Some(underwear_section) = underwear_cdie_opt {
                geoset_whitelist.insert((BASE_SECTION_UNDERWEAR * 100 + underwear_section.variation_index + 1) as u16);
            }

            if let Some(items) = items_opt {
                for (idx, item_opt) in items.iter().enumerate() {
                    let Some(item) = item_opt else {
                        continue;
                    };

                    for (grp_id, grp) in item.geoset_group.iter().enumerate() {
                        if *grp == 0 {
                            continue;
                        }

                        let geoset_group = item_slot_id_to_geoset_id(idx, grp_id).expect("sane lookup");
                        let geoset = geoset_group * 100 + (*grp as u16) + 1;
                        geoset_whitelist.insert(geoset);
                    }
                }
            }

            // Since the above dbc files can only specify overrides, we need to set defaults. The way we do this is by
            // checking if a specific group within 0..29 (* 100) is already part of the whitelist and if not, take our
            // big match statement to get the default id for them.

            for range in 0..29 {
                if geoset_whitelist
                    .iter()
                    .any(|&set| set >= (range * 100) && set < (range + 1) * 100)
                {
                    continue; // override present
                }

                geoset_whitelist.insert(default_geoset_for(range));
            }

            geoset_whitelist.insert(0); // always want to have the skin, even though we had hair (> 0)

            new_renderables.push((
                entity,
                (result, resolved_dynamic_textures, geoset_whitelist),
            ));
        }

        for (entity, (arc, dynamic_textures, geoset_whitelist)) in new_renderables {
            write
                .insert_one(
                    entity,
                    Renderable {
                        handles: None,
                        source: RenderableSource::M2(arc, dynamic_textures, geoset_whitelist),
                    },
                )
                .expect("Insert Renderable");
        }
    }
}

#[inline]
fn default_geoset_for(category: u16) -> u16 {
    match category {
        0 => 0,     // hair or skin
        1 => 100,   // facial default?
        2 => 200,   // facial default?
        3 => 300,   // facial default?
        4 => 401,   // default gloves
        5 => 501,   // default boots
        6 => 600,   // tail for draenei?
        7 => 702,   // ears (two)
        8 => 801,   // sleeves (no geoset)
        9 => 901,   // LegCuffs (no geoset)
        10 => 1001, // Chest (no geoset)
        11 => 1101, // Pants (no geoset)
        12 => 1201, // Tabard (no geoset)
        13 => 1301, // Trousers (default)
        14 => 1400, // DH/Pandaren female loincloth
        15 => 1501, // Cloak (no geoset)
        16 => 1600, // Nose Earrings
        17 => 1700, // Eyeglows (bloodelf ear size)
        18 => 1801, // Belt (default)
        19 => 1900, // Bone?
        20 => 2001, // feet (default)
        21 => 2101, // head (show head)
        22 => 2201, // Torso: Default
        23 => 2301, // Hands attachments
        24 => 2400, // head attachments
        25 => 2500, // blindfolds
        26 => 2600, // shoulders (no geoset)
        27 => 2701, // Helm (no geoset) was 2701
        28 => 2801, // arm upper (default)
        _ => unreachable!(),
    }
}

#[inline]
fn item_slot_id_to_geoset_id(slot_id: usize, group_id: usize) -> Option<u16> {
    Some(match slot_id {
        // Helm
        0 => match group_id {
            0 => 27,
            1 => 21, // can only be 01
            _ => return None,
        },
        // Shoulder
        1 => match group_id {
            0 => 26, // can only be 01
            _ => return None,
        },
        // Shirt
        2 => match group_id {
            0 => 8,  // can only be 01
            1 => 10, // can only be 01
            _ => return None,
        },
        // Chest
        3 => match group_id {
            0 => 8,  // can only be 01
            1 => 10, // can only be 01
            2 => 13, // can only be 01
            3 => 22, // can only be 01
            4 => 28, // can only be 01
            _ => return None,
        },
        // Waist
        4 => match group_id {
            0 => 18, // can only be 01
            _ => return None,
        },
        // Pants
        5 => match group_id {
            0 => 11, // can only be 01
            1 => 9,  // can only be 01
            2 => 13, // can only be 01
            _ => return None,
        },
        // Boots
        6 => match group_id {
            0 => 5, // can only be 01
            1 => 20,
            _ => return None,
        },
        // Wrists
        7 => return None,
        // Gloves
        8 => match group_id {
            0 => 4,  // can only be 01
            1 => 23, // can only be 01
            _ => return None,
        },
        // Cape
        9 => match group_id {
            0 => 15, // can only be 01
            _ => return None,
        },
        // Tabard
        10 => match group_id {
            0 => 12, // can only be 01
            _ => return None,
        },
        // Weapon
        11 => return None,
        // Weapon
        12 => return None,
        // Shield
        13 => return None,
        // Ammo
        14 => return None,
        _ => return None,
    })
}

fn get_base_skin_section<'a>(
    creature_display_info_extra: &CreatureDisplayInfoExtraRow,
    char_sections: &'a CharSections,
) -> Option<&'a CharSectionsRow> {
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
    char_hair_geosets: &'a CharHairGeosets,
    char_sections: &'a CharSections,
) -> Option<(&'a CharHairGeosetsRow, &'a CharSectionsRow)> {
    let hair_opt = char_hair_geosets
        .rows
        .iter() //
        .find(|geo| {
            geo.sex_id == creature_display_info_extra.display_sex_id
                && geo.race_id == creature_display_info_extra.display_race_id
                && geo.variation_id == creature_display_info_extra.hair_style_id
        });

    let Some(hair) = hair_opt else {
        warn!(
            "Could not find hair style for {}",
            creature_display_info_extra.hair_style_id
        );
        return None;
    };

    let sections = char_sections
        .rows()
        .iter()
        .filter(|section| {
            section.race_id == creature_display_info_extra.display_race_id
                && section.sex_id == creature_display_info_extra.display_sex_id
                && section.base_section == BASE_SECTION_HAIR
                && section.variation_index == hair.variation_id
                && section.color_index == creature_display_info_extra.hair_color_id
        })
        .collect_vec();

    if sections.len() > 1 {
        warn!("CharacterSection was not unique for hair");
        None
    } else if sections.is_empty() {
        warn!("Could not find any matching CharacterSection for hair");
        None
    } else {
        Some((hair, sections[0]))
    }
}

// TODO: we can probably unify those methods besides maybe the hair section?
fn get_face_section<'a>(
    creature_display_info_extra: &CreatureDisplayInfoExtraRow,
    char_sections: &'a CharSections,
) -> Option<&'a CharSectionsRow> {
    let sections = char_sections
        .rows()
        .iter()
        .filter(|section| {
            section.race_id == creature_display_info_extra.display_race_id
                && section.sex_id == creature_display_info_extra.display_sex_id
                && section.base_section == BASE_SECTION_FACE
                && section.variation_index == creature_display_info_extra.face_id
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

fn get_facial_hair_section<'a>(
    creature_display_info_extra: &CreatureDisplayInfoExtraRow,
    char_sections: &'a CharSections,
    char_hair_styles: &'a CharacterFacialHairStyles,
) -> Option<([i32; 5], &'a CharSectionsRow)> {
    let hair_opt = char_hair_styles
        .rows
        .iter() //
        .find(|geo| {
            geo.sex_id == creature_display_info_extra.display_sex_id
                && geo.race_id == creature_display_info_extra.display_race_id
                && geo.variation_id == creature_display_info_extra.facial_hair_id
        });

    let Some(hair) = hair_opt else {
        warn!(
            "Could not find facial hair style for {}",
            creature_display_info_extra.facial_hair_id
        );
        return None;
    };

    let sections = char_sections
        .rows()
        .iter()
        .filter(|section| {
            section.race_id == creature_display_info_extra.display_race_id
                && section.sex_id == creature_display_info_extra.display_sex_id
                && section.base_section == BASE_SECTION_FACIAL_HAIR
                && section.variation_index == hair.variation_id
                && section.color_index == creature_display_info_extra.hair_color_id
        })
        .collect_vec();

    if sections.len() > 1 {
        warn!("CharacterSection was not unique for facial hair");
        None
    } else if sections.is_empty() {
        warn!("Could not find any matching CharacterSection for facial hair");
        None
    } else {
        Some((hair.geoset, sections[0]))
    }
}

fn get_underwear_section<'a>(
    creature_display_info_extra: &CreatureDisplayInfoExtraRow,
    char_sections: &'a CharSections,
) -> Option<&'a CharSectionsRow> {
    let sections = char_sections
        .rows()
        .iter()
        .filter(|section| {
            section.race_id == creature_display_info_extra.display_race_id
                && section.sex_id == creature_display_info_extra.display_sex_id
                && section.base_section == BASE_SECTION_UNDERWEAR
                && section.color_index == creature_display_info_extra.skin_id
        })
        .collect_vec();

    if sections.len() > 1 {
        warn!("CharacterSection was not unique for underwear");
        None
    } else if sections.is_empty() {
        warn!("Could not find any matching CharacterSection for underwear");
        None
    } else {
        Some(sections[0])
    }
}
