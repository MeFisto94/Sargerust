use kira::sound::SoundData;
use kira::{AudioManager, AudioManagerSettings, DefaultBackend, PlaySoundError};

pub struct AudioPlaybackManager {
    audio_manager: AudioManager,
}

impl AudioPlaybackManager {
    pub fn new(gain: f32) -> anyhow::Result<Self> {
        let mut settings = AudioManagerSettings::default();
        let main_track_builder = settings
            .main_track_builder
            .volume(kira::Value::Fixed(gain.into()));
        settings.main_track_builder = main_track_builder;

        Ok(Self {
            audio_manager: AudioManager::<DefaultBackend>::new(settings)?,
        })
    }

    pub fn play<T: SoundData>(&mut self, sound_data: T) -> Result<T::Handle, PlaySoundError<T::Error>> {
        self.audio_manager.play(sound_data)
    }
}
