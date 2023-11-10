use std::sync::{Arc, Mutex};

use palette::blend::Blend;
use palette::WithAlpha;

use crate::effects::LightingEffect;
use crate::PhotonizerOptions;

pub struct LightBar {
    options: Arc<Mutex<PhotonizerOptions>>,
    pixel_count: usize,
    peak_falloff: f32,
    last_peak: f32,
}

impl LightBar {
    pub fn new(options: Arc<Mutex<PhotonizerOptions>>, pixel_count: usize) -> LightBar {
        LightBar {
            options,
            pixel_count,
            peak_falloff: 0.9,
            last_peak: 0.0,
        }
    }
}

impl LightingEffect for LightBar {
    fn step(&mut self, intensities: &Vec<f32>) -> Vec<palette::LinSrgb> {
        let cur_val = intensities[2].clamp(0.0, 1.0);
        if cur_val > self.last_peak {
            self.last_peak = cur_val;
        }

        let black = palette::LinSrgba::new(0.0, 0.0, 0.0, 1.0);
        let accent_color = self
            .options
            .lock()
            .unwrap()
            .accent_color
            .with_alpha(self.last_peak);
        let blended = black.overlay(accent_color).color;
        self.last_peak *= self.peak_falloff;
        return vec![blended; self.pixel_count];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Translated from https://floating-point-gui.de/errors/comparison/
    fn nearly_equal(a: f32, b: f32, epsilon: f32) -> bool {
        let abs_a = a.abs();
        let abs_b = b.abs();
        let diff = (a - b).abs();

        if a == b {
            // shortcut, handles infinities
            return true;
        } else if a == 0.0 || b == 0.0 || f32::is_subnormal(abs_a + abs_b) {
            // a or b is zero or both are extremely close to it
            // relative error is less meaningful here
            return diff < (epsilon * f32::MIN);
        } else {
            // use relative error
            return diff / (abs_a + abs_b).min(f32::MAX) < epsilon;
        }
    }

    #[test]
    fn blah() {
        let black = palette::LinSrgb::new(0.0f32, 0.0, 0.0).opaque();
        let mut pixel = palette::LinSrgb::new(1.0, 0.0, 0.0).opaque();

        pixel.alpha = 0.25;
        let result_25 = black.overlay(pixel);
        assert!(nearly_equal(result_25.red, 0.25, 0.01));
        assert!(nearly_equal(result_25.green, 0.0, 0.01));
        assert!(nearly_equal(result_25.blue, 0.0, 0.01));
        assert!(nearly_equal(result_25.alpha, 1.0, 0.01));

        pixel.alpha = 0.50;
        let result_50 = black.overlay(pixel);
        assert!(nearly_equal(result_50.red, 0.50, 0.01));
        assert!(nearly_equal(result_50.green, 0.0, 0.01));
        assert!(nearly_equal(result_50.blue, 0.0, 0.01));
        assert!(nearly_equal(result_50.alpha, 1.0, 0.01));

        pixel.alpha = 0.80;
        let result_80 = black.overlay(pixel);
        assert!(nearly_equal(result_80.red, 0.80, 0.01));
        assert!(nearly_equal(result_80.green, 0.0, 0.01));
        assert!(nearly_equal(result_80.blue, 0.0, 0.01));
        assert!(nearly_equal(result_80.alpha, 1.0, 0.01));
    }
}
