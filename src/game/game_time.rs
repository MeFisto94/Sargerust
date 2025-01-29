use log::warn;
use std::sync::Mutex;
use std::sync::atomic::AtomicU16;
use wow_world_messages::DateTime;

#[derive(Debug, Default)]
pub struct GameTime {
    /// The game time is by default ticked in 30s increments, thus this value ranges from [0, 2880] (1440 minutes per day)
    inner: AtomicU16,
    overflow_seconds: Mutex<f32>,
}

impl GameTime {
    pub fn update_time_and_speed(&self, time: DateTime, scale: f32) {
        if scale != 0.01666667 {
            warn!("Non-default scale of {} is not implemented yet", scale);
        }

        // We lose up to one tick here (30s)
        let ticks: u16 = time.hours() as u16 * 120 + time.minutes() as u16 * 2;
        self.inner.store(ticks, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn as_30s_ticks(&self) -> u16 {
        self.inner.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn advance_time(&self, delta_time: f32) {
        let mut overflow = self.overflow_seconds.lock().unwrap();
        *overflow += delta_time;

        if *overflow >= 30.0 {
            *overflow -= 30.0;
            self.inner.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let _ = self.inner.compare_exchange(
                2880,
                0,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            );
        }
    }
}
