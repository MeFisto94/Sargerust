use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::common::coordinate_systems;
use glam::{Vec3, Vec4};
use itertools::Itertools;
use log::trace;
use std::sync::Arc;
use wow_dbc::wrath_tables::light::{Light, LightRow};
use wow_dbc::wrath_tables::light_float_band::{LightFloatBand, LightFloatBandRow};
use wow_dbc::wrath_tables::light_int_band::{LightIntBand, LightIntBandRow};
use wow_dbc::wrath_tables::light_params::LightParams;
use wow_dbc::wrath_tables::light_skybox::{LightSkybox, LightSkyboxKey};
use wow_dbc::{DbcTable, Indexable};

/// Responsible to load the skybox, ambience light and others for a given map zone.
pub struct MapLightSettingsProvider {
    light: Light,
    light_skybox: LightSkybox,
    light_params: LightParams,
    light_int_band: LightIntBand,
    light_float_band: LightFloatBand,
}

impl MapLightSettingsProvider {
    fn load_dbc<T: DbcTable>(mpq_loader: &MPQLoader, name: &str) -> T {
        let buf = mpq_loader
            .load_raw_owned(name)
            .unwrap_or_else(|| panic!("Failed to load {}", name));
        trace!("Loaded {} ({} bytes)", name, buf.len());
        T::read(&mut buf.as_slice()).unwrap_or_else(|_| panic!("Failed to parse {}", name))
    }

    // TODO: This loads DBC files which is a bit counter intuitive for constructors, but I guess the only way to prevent
    // useless Options. Unless we add a builder...
    pub fn build(mpq_loader: Arc<MPQLoader>) -> Self {
        // This holds ~1MiB of data, technically we could load it on demand (async), because we only need a fragment
        // of the data and that only on map changes (portals, teleports), but we're probably rather memory hungry anyway
        Self {
            light: Self::load_dbc(&mpq_loader, "DBFilesClient\\Light.dbc"),
            light_skybox: Self::load_dbc(&mpq_loader, "DBFilesClient\\LightSkybox.dbc"),
            light_params: Self::load_dbc(&mpq_loader, "DBFilesClient\\LightParams.dbc"),
            light_int_band: Self::load_dbc(&mpq_loader, "DBFilesClient\\LightIntBand.dbc"),
            light_float_band: Self::load_dbc(&mpq_loader, "DBFilesClient\\LightFloatBand.dbc"),
        }
    }

    pub fn get_local_settings(settings: &[LightSettings], position: Vec3) -> Option<&LightSettings> {
        settings
            .iter()
            .filter(|setting| !Self::is_global(setting))
            .map(|settings| (settings, (settings.position - position).length_squared()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(settings, _)| settings)
    }

    pub fn get_global_settings(settings: &[LightSettings]) -> Option<&LightSettings> {
        settings.iter().find(|setting| Self::is_global(setting))
    }

    fn is_global(setting: &LightSettings) -> bool {
        // (17066.666, 17066.666, 0.0) is (0, 0, 0) before transformation into world space.
        setting.position.x == 17066.666
            && setting.position.y == 17066.666
            && setting.position.z == 0.0
            && setting.falloff_start == 0.0
            && setting.falloff_end == 0.0
    }

    fn get_light_for_map(&self, map_id: i32) -> impl Iterator<Item = &LightRow> {
        self.light
            .rows()
            .iter()
            .filter(move |row| row.continent_id == map_id.into())
    }

    fn get_int_bands(&self, id: usize) -> [LightBandEntry<i32>; 18] {
        self.light_int_band.rows[id * 18 - 17..id * 18 + 1]
            .iter()
            .map(LightBandEntry::<i32>::new)
            .collect_vec()
            .try_into()
            .expect("length should be 18")
    }

    fn get_float_bands(&self, id: usize) -> [LightBandEntry<f32>; 6] {
        self.light_float_band.rows[id * 6 - 5..id * 6 + 1]
            .iter()
            .map(LightBandEntry::<f32>::new)
            .collect_vec()
            .try_into()
            .expect("length should be 6")
    }

    fn get_light_parameters(&self, light_params_id: i32) -> Option<LightParameters> {
        self.light_params.get(light_params_id).map(|row| {
            let int_bands = self.get_int_bands(light_params_id as usize);
            let float_bands = self.get_float_bands(light_params_id as usize);

            let [
                fog_distance,
                fog_multiplier,
                celestial_glow_through,
                cloud_density,
                unk_float_bands1,
                unk_float_bands2,
            ] = float_bands;

            let [
                diffuse_color,
                ambient_color,
                sky_top_color,
                sky_mid_color,
                sky_mid_to_horizon_color,
                above_horizon_color,
                horizon_color,
                fog_color,
                unk_int_band,
                cloud_sun_color,
                cloud_emissive_color,
                cloud_color_b,
                cloud_color_a2,
                unk_int_band2,
                ocean_shallow_color,
                ocean_deep_color,
                water_shallow_color,
                water_deep_color,
            ] = int_bands;

            LightParameters {
                highlight_sky: row.highlight_sky != 0,
                light_skybox: self.get_skybox(row.light_skybox_id),
                // cloud_type: row.cloud_type,
                glow: row.glow,
                water_shallow_alpha: row.water_shallow_alpha,
                water_deep_alpha: row.water_deep_alpha,
                ocean_shallow_alpha: row.ocean_shallow_alpha,
                ocean_deep_alpha: row.ocean_deep_alpha,
                flags: row.flags,
                fog_distance,
                fog_multiplier,
                celestial_glow_through,
                cloud_density,
                unk_float_bands: [unk_float_bands1, unk_float_bands2],
                diffuse_color,
                ambient_color,
                sky_top_color,
                sky_mid_color,
                sky_mid_to_horizon_color,
                above_horizon_color,
                horizon_color,
                fog_color,
                unk_int_band,
                cloud_sun_color,
                cloud_emissive_color,
                cloud_color_b,
                cloud_color_a2,
                unk_int_band2,
                ocean_shallow_color,
                ocean_deep_color,
                water_shallow_color,
                water_deep_color,
            }
        })
    }

    fn get_skybox(&self, skybox_id: LightSkyboxKey) -> Option<String> {
        self.light_skybox.get(skybox_id).map(|row| row.name.clone())
    }

    pub fn get_settings_for_map(&self, map_id: i32) -> Vec<LightSettings> {
        self.get_light_for_map(map_id)
            .map(|light| {
                let position = coordinate_systems::light_dbc_to_vec(light.game_coords);
                let falloff_start = coordinate_systems::light_dbc_falloff(light.game_falloff_start);
                let falloff_end = coordinate_systems::light_dbc_falloff(light.game_falloff_end);

                LightSettings {
                    position,
                    falloff_start,
                    falloff_end,
                    clear: self.get_light_parameters(light.light_params_id[0]).unwrap(),
                    clear_underwater: self.get_light_parameters(light.light_params_id[1]).unwrap(),
                    storm: self.get_light_parameters(light.light_params_id[2]).unwrap(),
                    storm_underwater: self.get_light_parameters(light.light_params_id[3]).unwrap(),
                    death: self.get_light_parameters(light.light_params_id[4]).unwrap(),
                    wotlk_unk1: self.get_light_parameters(light.light_params_id[5]),
                    wotlk_unk2: self.get_light_parameters(light.light_params_id[6]),
                    wotlk_unk3: self.get_light_parameters(light.light_params_id[7]),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct LightSettings {
    pub position: Vec3,
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub clear: LightParameters,
    pub clear_underwater: LightParameters,
    pub storm: LightParameters,
    pub storm_underwater: LightParameters,
    pub death: LightParameters,
    pub wotlk_unk1: Option<LightParameters>,
    pub wotlk_unk2: Option<LightParameters>,
    pub wotlk_unk3: Option<LightParameters>,
}

#[derive(Debug, Clone)]
pub struct LightBandEntry<T> {
    pub data: Vec<LightBandTuple<T>>,
}

impl<T> LightBandEntry<T> {
    pub fn get_value_for_time(&self, time: i32) -> Option<&T> {
        self.data
            .iter()
            .find(|band| band.time == time)
            .map(|band| &band.data)
    }

    pub fn get_tuple_for_time(&self, time: i32) -> Option<&LightBandTuple<T>> {
        self.data.iter().find(|band| band.time == time)
    }
}

impl LightBandEntry<i32> {
    pub fn new(row: &LightIntBandRow) -> Self {
        Self {
            data: row
                .data
                .iter()
                .zip(row.time.iter())
                .take(row.num as usize)
                .map(|(data, time)| LightBandTuple::<i32> {
                    time: *time,
                    data: *data,
                })
                .collect_vec(),
        }
    }
}

impl LightBandEntry<f32> {
    pub fn new(row: &LightFloatBandRow) -> Self {
        Self {
            data: row
                .data
                .iter()
                .zip(row.time.iter())
                .take(row.num as usize)
                .map(|(data, time)| LightBandTuple::<f32> {
                    time: *time,
                    data: *data,
                })
                .collect_vec(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct LightBandTuple<T> {
    pub time: i32,
    pub data: T,
}

impl LightBandTuple<i32> {
    pub fn as_color(&self) -> Vec4 {
        Vec4::new(
            ((self.data >> 16) & 0xFF) as f32 / 255.0,
            ((self.data >> 8) & 0xFF) as f32 / 255.0,
            (self.data & 0xFF) as f32 / 255.0,
            1.0,
        )
    }
}

#[derive(Debug, Clone)]
pub struct LightParameters {
    pub highlight_sky: bool,
    /// Path to the M2 file.
    pub light_skybox: Option<String>,
    // pub cloud_type: u32, // TODO: where?
    /// This controls how much Fog gets added to everything (!) and is used in some places to make them look extra bright
    pub glow: f32,
    /// Controls how transparent the water is for lakes & rivers.
    pub water_shallow_alpha: f32,
    /// Controls how transparent the water is for lakes & rivers.
    pub water_deep_alpha: f32,
    /// Controls how transparent the water is for the ocean.
    pub ocean_shallow_alpha: f32,
    /// Controls how transparent the water is for the ocean.
    pub ocean_deep_alpha: f32,

    /// There's conflicting notes about what flag means, besides 0x4 (hide sun), 0x8 (hide moon) and 0x10 (hide stars)
    pub flags: i32,

    /// Fog distance multiplied by 36 - distance at which everything will be hidden by the fog
    pub fog_distance: LightBandEntry<f32>,
    /// fog distance * fog multiplier = fog start distance. 0-0,999...
    pub fog_multiplier: LightBandEntry<f32>,
    /// Celestial Glow Through - the brightness of the sun and moon as it shows through cloud cover. Note that this effect only appears when the Sun or Moon is being obscured by clouds. 0-1
    pub celestial_glow_through: LightBandEntry<f32>,
    /// Controls the density of cloud cover in the area. Value range is 0.0 to 1.0.
    pub cloud_density: LightBandEntry<f32>,
    pub unk_float_bands: [LightBandEntry<f32>; 2],

    pub diffuse_color: LightBandEntry<i32>,
    pub ambient_color: LightBandEntry<i32>,
    pub sky_top_color: LightBandEntry<i32>,
    pub sky_mid_color: LightBandEntry<i32>,
    pub sky_mid_to_horizon_color: LightBandEntry<i32>,
    pub above_horizon_color: LightBandEntry<i32>,
    pub horizon_color: LightBandEntry<i32>, // also smog color
    /// Fog / background mountains. Also affects weather effects
    pub fog_color: LightBandEntry<i32>,
    /// Unknown/unused in 3.3.5 ? This value was ported to ShadowOpacity in the new format
    pub unk_int_band: LightBandEntry<i32>,
    /// Sun color + sun halo color, specular lighting, sun rays
    pub cloud_sun_color: LightBandEntry<i32>,
    /// Sun larger halo color  //  cloud color a1 (base)
    pub cloud_emissive_color: LightBandEntry<i32>,
    /// ? // cloud color B (edge)
    pub cloud_color_b: LightBandEntry<i32>,
    /// Cloud color  // cloud color a2 (secondary base)
    pub cloud_color_a2: LightBandEntry<i32>,
    /// Unknown/unused in 3.3.5 ? This value was ported to Cloud Layer 2 Ambient Color in the new format,
    pub unk_int_band2: LightBandEntry<i32>,
    pub ocean_shallow_color: LightBandEntry<i32>,
    pub ocean_deep_color: LightBandEntry<i32>,
    pub water_shallow_color: LightBandEntry<i32>,
    pub water_deep_color: LightBandEntry<i32>,
}
