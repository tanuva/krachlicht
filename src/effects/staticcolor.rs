use std::sync::{Arc, Mutex};

use crate::effects::LightingEffect;
use crate::PhotonizerOptions;

pub struct StaticColor {
    options: Arc<Mutex<PhotonizerOptions>>,
    pixel_count: usize,
}

impl StaticColor {
    pub fn new(options: Arc<Mutex<PhotonizerOptions>>, pixel_count: usize) -> StaticColor {
        StaticColor {
            options,
            pixel_count,
        }
    }
}

impl LightingEffect for StaticColor {
    fn step(&mut self, _: &Vec<f32>) -> Vec<palette::LinSrgb> {
        let color = self.options.lock().unwrap().accent_color;
        vec![color; self.pixel_count]
    }
}
