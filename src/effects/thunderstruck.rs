use std::sync::{Arc, Mutex};

use palette::blend::Blend;
use palette::WithAlpha;
use rand::Rng;

use crate::effects::LightingEffect;
use crate::effects::Pulse;
use crate::PhotonizerOptions;

pub struct Thunderstruck {
    options: Arc<Mutex<PhotonizerOptions>>,
    pixel_count: usize,
    peak_falloff: f32,
    last_peak: f32,
    pulses: Vec<Pulse>,
}

impl Thunderstruck {
    pub fn new(options: Arc<Mutex<PhotonizerOptions>>, pixel_count: usize) -> Thunderstruck {
        Thunderstruck {
            options,
            pixel_count,
            peak_falloff: 0.9,
            last_peak: 0.0,
            pulses: vec![],
        }
    }

    fn decay_strikes(&mut self) {
        for pulse in &mut self.pulses {
            pulse.intensity *= self.peak_falloff;
        }
    }

    fn remove_strikes(&mut self) {
        self.pulses.retain(|pulse| pulse.intensity > 0.1);
    }

    fn create_strike(&mut self, intensities: &Vec<f32>) {
        let cur_val = intensities[2].clamp(0.0, 1.0);
        if cur_val < self.last_peak {
            return;
        }

        self.last_peak = cur_val;
        let white = palette::LinSrgb::new(1.0, 1.0, 1.0);
        self.pulses.push(Pulse {
            color: white,
            position: rand::thread_rng().gen_range(0..self.pixel_count) as f32,
            intensity: 1.0,
        });

        self.last_peak *= self.peak_falloff;
    }
}

impl LightingEffect for Thunderstruck {
    fn step(&mut self, intensities: &Vec<f32>) -> Vec<palette::LinSrgb> {
        self.decay_strikes();
        self.remove_strikes();
        self.create_strike(intensities);

        let black = palette::LinSrgba::new(0.0, 0.0, 0.0, 1.0);
        let accent_color = self.options.lock().unwrap().accent_color.with_alpha(0.3);
        let mut frame_buffer = vec![accent_color; self.pixel_count];

        // Pulse draw pass
        for pulse in &self.pulses {
            frame_buffer[pulse.position as usize] = frame_buffer[pulse.position as usize]
                .overlay(pulse.color.with_alpha(pulse.intensity));
        }

        // Alpha baking pass
        let mut baked_buffer = vec![black.color; self.pixel_count];
        for i in 0..baked_buffer.len() {
            baked_buffer[i] = black.overlay(frame_buffer[i]).color;
        }

        return baked_buffer;
    }
}
