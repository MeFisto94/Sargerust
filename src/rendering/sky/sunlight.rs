use crate::game::map_light_settings_provider::{LightBandTuple, interpolate_for_time};
use crate::util::spherical_to_cartesian;
use rend3::Renderer;
use rend3::types::{DirectionalLight, DirectionalLightChange, DirectionalLightHandle};
use std::convert::Into;
use std::default::Default;
use std::sync::{Arc, LazyLock};

#[derive(Debug)]
pub struct Sunlight {
    light_handle: Option<DirectionalLightHandle>,
    intensity: f32,
}

static PHI_TABLE: LazyLock<[LightBandTuple<f32>; 4]> = LazyLock::new(|| {
    [
        (0, 2.2165682).into(),
        (720, 1.9198623).into(),
        (1440, 2.2165682).into(),
        (2160, 1.9198623).into(),
    ]
});

static THETA_TABLE: LazyLock<[LightBandTuple<f32>; 4]> = LazyLock::new(|| {
    [
        (0, 3.926991).into(),
        (720, 3.926991).into(),
        (1440, 3.926991).into(),
        (2160, 3.926991).into(),
    ]
});

impl Sunlight {
    pub fn new(intensity: f32) -> Self {
        Self {
            light_handle: None,
            intensity,
        }
    }

    pub fn get_sun_direction(&self, day_progression: u16) -> glam::Vec3 {
        let phi = interpolate_for_time(&*PHI_TABLE, day_progression);
        let theta = interpolate_for_time(&*THETA_TABLE, day_progression);

        spherical_to_cartesian(1.0, phi, theta)
    }

    pub fn update(&mut self, renderer: &Arc<Renderer>, day_progression: u16, color: glam::Vec3) {
        let direction = self.get_sun_direction(day_progression);

        let handle = self.light_handle.get_or_insert_with(|| {
            renderer.add_directional_light(DirectionalLight {
                color: color.normalize(),
                direction,
                distance: 400.0,
                resolution: 4096,
                intensity: self.intensity,
            })
        });

        renderer.update_directional_light(
            handle,
            DirectionalLightChange {
                color: Some(color),
                direction: Some(direction),
                ..Default::default()
            },
        );
    }
}
