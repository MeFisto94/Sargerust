use crate::game::application::GameApplication;
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::load_dbc;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::nodes::adt_node::ADTNode;
use crate::settings::CliArgs;
use chrono::{Duration, NaiveDateTime, Utc};
use glam::Vec3;
use kira::sound::PlaybackState;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::{Easing, StartTime, Tween};
use log::{debug, error, trace, warn};
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use wow_dbc::Indexable;
use wow_dbc::wrath_tables::area_table::AreaTable;
use wow_dbc::wrath_tables::sound_entries::{SoundEntries, SoundEntriesKey, SoundEntriesRow};
use wow_dbc::wrath_tables::zone_intro_music_table::{ZoneIntroMusicTable, ZoneIntroMusicTableRow};
use wow_dbc::wrath_tables::zone_music::{ZoneMusic, ZoneMusicRow};

struct CurrentZoneMusic {
    intro_opt: Option<ZoneIntroMusicTableRow>,
    intro_sound_entry: Option<SoundEntriesRow>,
    is_still_playing_intro: bool,
    zone_music: ZoneMusicRow,
    // TODO: Use Day and Night variants
    #[deprecated]
    zone_sound_entry: SoundEntriesRow,
}

impl CurrentZoneMusic {
    pub fn new(
        intro_opt: Option<ZoneIntroMusicTableRow>,
        intro_sound_entry: Option<SoundEntriesRow>,
        is_still_playing_intro: bool,
        zone_music: ZoneMusicRow,
        zone_sound_entry: SoundEntriesRow,
    ) -> Self {
        Self {
            intro_opt,
            intro_sound_entry,
            is_still_playing_intro,
            zone_music,
            zone_sound_entry,
        }
    }

    #[inline]
    pub fn current_entry(&self) -> &SoundEntriesRow {
        if self.is_still_playing_intro {
            if let Some(intro) = &self.intro_sound_entry {
                intro
            } else {
                warn!("is_still_playing_intro is true but intro_sound_entry is None");
                &self.zone_sound_entry
            }
        } else {
            &self.zone_sound_entry
        }
    }

    // The assumption here is that zone intros aren't meant to be played in succession (or only ever have one entry)
    fn pick_next_slice(&self) -> u8 {
        let highest_index = self.highest_index();
        let sum_of_frequencies = self
            .current_entry()
            .freq
            .iter()
            .take((highest_index + 1) as usize)
            .map(|freq| *freq as u32)
            .sum::<u32>();
        let mut rng = rand::rng();
        let roll = rng.random_range(0..sum_of_frequencies);
        let mut cumulative = 0u32;
        for (index, freq) in self
            .current_entry()
            .freq
            .iter()
            .take((highest_index + 1) as usize)
            .enumerate()
        {
            cumulative += *freq as u32; // if someone uses negative frequencies, they get what they deserve
            if roll < cumulative {
                return index as u8;
            }
        }
        highest_index
    }

    /// SAFETY: Overflows if no files are present at all, which would be a broken sound entry
    #[inline]
    fn highest_index(&self) -> u8 {
        self.current_entry()
            .file
            .iter()
            .position(|file| file.is_empty())
            .unwrap_or(10) as u8
            - 1
    }

    pub fn get_next_sound(&self) -> (String, f32) {
        let next_index = self.pick_next_slice();
        (
            format!(
                "{}\\{}",
                self.current_entry().directory_base,
                self.current_entry().file[next_index as usize]
            ),
            self.current_entry().volume_float,
        )
    }

    pub fn is_still_intro(&self) -> bool {
        self.is_still_playing_intro
    }

    pub fn finish_intro(&mut self) {
        // separate method to not have it implicitly in get_next_sound_path _and_ not require &mut.
        self.is_still_playing_intro = false;
    }

    pub fn get_cooldown_duration(&self) -> (Duration, SoundEntriesKey) {
        if self.is_still_playing_intro
            && let Some(intro) = &self.intro_opt
        {
            (
                Duration::minutes(intro.min_delay_minutes as i64),
                intro.sound_id,
            )
        } else {
            let mut rng = rand::rng();
            // TODO: Day and Night support
            let min = self.zone_music.silence_interval_min[0] as i64;
            let max = self.zone_music.silence_interval_max[0] as i64;
            (
                Duration::milliseconds(rng.random_range(min..=max)),
                self.zone_sound_entry.id,
            )
        }
    }
}

pub struct ZoneMusicManager {
    // TODO: This belongs outside. MapManager? Dedicated AreaManager?
    area_table: AreaTable,
    zone_music: ZoneMusic,
    zone_intro_music_table: ZoneIntroMusicTable,
    sound_entries: SoundEntries,
    on_tile_changed: Option<tokio::sync::watch::Receiver<Option<Arc<ADTNode>>>>,
    cooldowns: HashMap<SoundEntriesKey, NaiveDateTime>,
    mpq_loader: Arc<MPQLoader>,
    current_zone_music: Option<CurrentZoneMusic>,
    current_sound_handle: Option<StaticSoundHandle>,
    pending_cooldown: Option<(SoundEntriesKey, Duration)>,
    continuous_zone_music: bool,
}

impl ZoneMusicManager {
    pub fn new(mpq_loader: Arc<MPQLoader>, args: &CliArgs) -> Self {
        Self {
            area_table: load_dbc(&mpq_loader, "DBFilesClient\\AreaTable.dbc"),
            zone_music: load_dbc(&mpq_loader, "DBFilesClient\\ZoneMusic.dbc"),
            zone_intro_music_table: load_dbc(&mpq_loader, "DBFilesClient\\ZoneIntroMusicTable.dbc"),
            sound_entries: load_dbc(&mpq_loader, "DBFilesClient\\SoundEntries.dbc"),
            on_tile_changed: None,
            cooldowns: HashMap::new(),
            mpq_loader: mpq_loader.clone(),
            current_zone_music: None,
            current_sound_handle: None,
            pending_cooldown: None,
            continuous_zone_music: args.continuous_zone_music,
        }
    }

    pub fn update(&mut self, app: &GameApplication, _delta_time: f32) {
        if self.on_tile_changed.is_none() {
            self.setup_listeners(app);
        }

        self.update_tile_event(app, _delta_time);
        self.update_cooldowns();
        self.update_running_sound(app);
    }

    pub fn setup_listeners(&mut self, app: &GameApplication) {
        self.on_tile_changed = Some(
            app.game_state
                .map_manager
                .read()
                .expect("GameState to exist when creating ZoneMusicManager")
                .tile_watcher
                .clone(),
        );
    }

    pub fn update_tile_event(&mut self, app: &GameApplication, _delta_time: f32) {
        // TODO: Decouple from main update and maybe make an async task.

        let listener = self
            .on_tile_changed
            .as_mut()
            .expect("setup_listeners to set on_tile_changed");

        if listener
            .has_changed()
            .expect("on_tile_changed channel closed")
        {
            let player_position = {
                app.game_state
                    .player_location
                    .read()
                    .expect("lock poisoned")
                    .clone()
            }
            .into();
            // We need to clone the Arc to unborrow listener and thus unborrow self...
            let tile_change = listener.borrow_and_update().clone();
            self.on_tile_changed(app, player_position, tile_change);
        }
    }

    fn on_tile_changed(&mut self, app: &GameApplication, player_position: Vec3, tile_change: Option<Arc<ADTNode>>) {
        // TODO: Actually, listening for tile changes is not enough, as there can be different areas within one tile.
        if let Some(tile) = tile_change {
            // TODO: Util for ExtendedLocalizedString to string based on the locale.

            let Some(offset) = tile.calculate_terrain_tile_offset_for(player_position.into()) else {
                panic!("Player position not in tile??");
            };

            let Some(area) = self.area_table.get(tile.terrain[offset].area_id) else {
                error!(
                    "Didn't find a corresponding area for area id {:?}",
                    tile.terrain[offset].area_id
                );
                return;
            };

            debug!("Area: {:?}", area); // TODO: Debug Elwynn right before westfall, area 916 has no zone music

            let zone_intro_opt = self
                .zone_intro_music_table
                .get(area.intro_sound)
                .and_then(|zim| self.sound_entries.get(zim.sound_id).map(|zmse| (zim, zmse)));

            let Some(zone_music) = self.zone_music.get(area.zone_music) else {
                warn!(
                    "Didn't find a corresponding zone music for area {:?}",
                    area.id
                );
                self.stop_playing();
                return;
            };
            trace!("ZoneMusic: {:?}", zone_music);
            // TODO: Day and night.
            let Some(zone_sound) = self.sound_entries.get(zone_music.sounds[0]) else {
                error!(
                    "Didn't find a corresponding sound entry for zone music set name {:?}",
                    zone_music.set_name
                );
                self.stop_playing();
                return;
            };
            trace!("ZoneSound: {:?}", zone_sound);

            let play_intro = zone_intro_opt
                .map(|(zim, _)| !self.cooldowns.contains_key(&zim.sound_id))
                .unwrap_or(false);

            if let Some(old_zone_music) = self.current_zone_music.as_ref() {
                if old_zone_music.zone_music.id == zone_music.id
                    && old_zone_music.intro_opt.as_ref().map(|row| row.id) == zone_intro_opt.map(|(row, _)| row.id)
                {
                    debug!("Same zone music as before, skipping");
                    return;
                }
            }

            // we need to unpack and clone so that we can drop &self before calling stop_playing
            let zim = zone_intro_opt.map(|(zim, _)| zim.clone());
            let intro = zone_intro_opt.map(|(_, intro)| intro.clone());
            let zone_music = zone_music.clone();
            let zone_sound = zone_sound.clone();

            self.stop_playing();

            self.current_zone_music = Some(CurrentZoneMusic::new(
                zim, intro, play_intro, zone_music, zone_sound,
            ));

            self.start_playing(app);
        } else {
            trace!("Map unloaded, stopping zone music");
            self.stop_playing();
        }
    }

    fn stop_playing(&mut self) {
        if let Some(handle) = self.current_sound_handle.as_mut() {
            let tween = Tween {
                start_time: StartTime::Immediate,
                duration: std::time::Duration::from_secs(1),
                easing: Easing::Linear,
            };

            handle.stop(tween);
        }
        self.current_zone_music = None;
    }

    fn start_playing(&mut self, app: &GameApplication) {
        let Some(current_music) = self.current_zone_music.as_ref() else {
            warn!("Call to start_playing without current zone music");
            return;
        };

        let (sound_path, volume) = current_music.get_next_sound();
        let (cooldown, key) = current_music.get_cooldown_duration();
        trace!("Starting to play zone music from path: {}", sound_path);

        // TODO: StreamingSoundData once AssetLoader can do std::io::Read+Seek.
        let whole_buf = self
            .mpq_loader
            .load_raw_owned(&sound_path)
            .unwrap_or_else(|| panic!("File could not be found: {}", sound_path)); // This will nicely block the game thread, cool.
        let sound_handle = StaticSoundData::from_cursor(std::io::Cursor::new(whole_buf))
            .unwrap()
            .volume(volume);

        match app
            .audio_playback_manager
            .write()
            .unwrap()
            .play(sound_handle)
        {
            Ok(handle) => {
                self.current_sound_handle = Some(handle);
                self.pending_cooldown = Some((key, cooldown));
            }
            Err(err) => {
                warn!("Failed to play sound: {:?}", err);
                self.cooldowns
                    .insert(key, Utc::now().naive_utc() + cooldown);
            }
        }
    }

    fn update_running_sound(&mut self, app: &GameApplication) {
        if let Some(handle) = self.current_sound_handle.as_ref() {
            if handle.state() != PlaybackState::Stopped {
                return;
            }

            trace!("Finished playing the current track");
            if let Some((key, cooldown)) = self.pending_cooldown.take() {
                self.cooldowns
                    .insert(key, Utc::now().naive_utc() + cooldown);
            }

            self.current_sound_handle = None;

            let Some(current_music) = self.current_zone_music.as_mut() else {
                return; // Most likely the zone has been left while the sound was playing (or that was the cause for stopping)
            };

            if current_music.is_still_intro() {
                current_music.finish_intro();
                self.start_playing(app);
                return;
            }
        }

        if self.current_sound_handle.is_none() {
            if let Some(current_music) = self.current_zone_music.as_ref() {
                if self.continuous_zone_music
                    || !self
                        .cooldowns
                        .contains_key(&current_music.current_entry().id)
                {
                    self.start_playing(app);
                }
            }
        }
    }

    #[inline]
    fn update_cooldowns(&mut self) {
        let now = Utc::now().naive_utc();
        self.cooldowns
            .retain(|_, cooldown_time| *cooldown_time > now);
    }
}

#[cfg(test)]
mod tests {
    use crate::audio::zone_music_manager::CurrentZoneMusic;
    use wow_dbc::wrath_tables::sound_entries::{SoundEntriesKey, SoundEntriesRow};
    use wow_dbc::wrath_tables::sound_entries_advanced::SoundEntriesAdvancedKey;
    use wow_dbc::wrath_tables::zone_music::ZoneMusicRow;

    fn build_zone_music_row() -> ZoneMusicRow {
        ZoneMusicRow {
            id: wow_dbc::wrath_tables::zone_music::ZoneMusicKey::new(1),
            set_name: "TestSet".to_string(),
            silence_interval_min: [0, 0],
            silence_interval_max: [1, 1],
            sounds: [1; 2],
        }
    }

    #[test]
    pub fn highest_index_if_all_filled() {
        let entry = SoundEntriesRow {
            id: SoundEntriesKey::new(1),
            directory_base: "".to_string(),
            file: [
                "file1.ogg".to_string(),
                "file2.ogg".to_string(),
                "file3.ogg".to_string(),
                "file4.ogg".to_string(),
                "file5.ogg".to_string(),
                "file6.ogg".to_string(),
                "file7.ogg".to_string(),
                "file8.ogg".to_string(),
                "file9.ogg".to_string(),
                "file10.ogg".to_string(),
            ],
            volume_float: 1.0,
            distance_cutoff: 0.0,
            min_distance: 0.0,
            freq: [0; 10],
            flags: 0,
            e_a_x_def: 0,
            sound_entries_advanced_id: SoundEntriesAdvancedKey::new(0),
            sound_type: 0,
            name: "".to_string(),
        };

        let current_music = CurrentZoneMusic::new(None, None, false, build_zone_music_row(), entry);
        assert_eq!(current_music.highest_index(), 9);
    }

    #[test]
    pub fn highest_index_general() {
        let entry = SoundEntriesRow {
            id: SoundEntriesKey::new(1),
            directory_base: "".to_string(),
            file: [
                "file1.ogg".to_string(),
                "file2.ogg".to_string(),
                "file3.ogg".to_string(),
                "file4.ogg".to_string(),
                "file5.ogg".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ],
            volume_float: 1.0,
            distance_cutoff: 0.0,
            min_distance: 0.0,
            freq: [0; 10],
            flags: 0,
            e_a_x_def: 0,
            sound_entries_advanced_id: SoundEntriesAdvancedKey::new(0),
            sound_type: 0,
            name: "".to_string(),
        };

        let current_music = CurrentZoneMusic::new(None, None, false, build_zone_music_row(), entry);
        assert_eq!(current_music.highest_index(), 4);
    }

    #[test]
    pub fn pick_next_slice_works_for_one_entry() {
        let entry = SoundEntriesRow {
            id: SoundEntriesKey::new(1),
            directory_base: "".to_string(),
            file: [
                "file1.ogg".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ],
            volume_float: 1.0,
            distance_cutoff: 0.0,
            min_distance: 0.0,
            freq: [1; 10],
            flags: 0,
            e_a_x_def: 0,
            sound_entries_advanced_id: SoundEntriesAdvancedKey::new(0),
            sound_type: 0,
            name: "".to_string(),
        };

        let current_music = CurrentZoneMusic::new(None, None, false, build_zone_music_row(), entry);
        assert_eq!(current_music.pick_next_slice(), 0);
        assert_eq!(current_music.pick_next_slice(), 0);
        assert_eq!(current_music.pick_next_slice(), 0);
    }
}
