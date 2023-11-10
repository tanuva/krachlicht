pub(crate) mod lightbar;
pub(crate) mod pixelflow;
pub(crate) mod staticcolor;
pub(crate) mod thunderstruck;

pub trait LightingEffect {
    fn step(&mut self, intensities: &Vec<f32>) -> Vec<palette::LinSrgb>;
}

struct Pulse {
    color: palette::LinSrgb,
    intensity: f32,
    position: f32,
}
