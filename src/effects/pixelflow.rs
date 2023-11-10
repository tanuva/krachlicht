use std::sync::{Arc, Mutex};

use palette::blend::Blend;
use palette::WithAlpha;

use crate::effects::LightingEffect;
use crate::effects::Pulse;
use crate::PhotonizerOptions;

pub struct PixelFlow {
    options: Arc<Mutex<PhotonizerOptions>>,
    pixel_count: usize,
    peak_falloff: f32,
    last_peak: f32,
    pulses: Vec<Pulse>,
}

impl PixelFlow {
    pub fn new(options: Arc<Mutex<PhotonizerOptions>>, pixel_count: usize) -> PixelFlow {
        PixelFlow {
            options,
            pixel_count,
            peak_falloff: 0.95,
            last_peak: 0.0,
            pulses: vec![],
        }
    }

    fn advance_pulses(&mut self) {
        let pulse_speed = self.options.lock().unwrap().pulse_speed;

        for pulse in &mut self.pulses {
            pulse.position += pulse_speed;
        }
    }

    fn remove_pulses(&mut self) {
        // TODO Try self.pulses.retain(|pulse| pulse.position < self.pixel_count as f32 - 1.0);
        // FIXME This is not flow direction agnostic!
        // This would be nicer as a do-while loop. And feels awkward in any case.
        let mut check_again = true;
        while check_again {
            check_again = false;

            for i in 0..self.pulses.len() {
                if self.pulses[i].position > self.pixel_count as f32 - 1.0 {
                    self.pulses.remove(i);
                    check_again = true;
                    break;
                }
            }
        }
    }

    fn create_pulse(&mut self, intensities: &Vec<f32>) {
        let accent_color = self.options.lock().unwrap().accent_color;
        let cur_val = intensities[2].clamp(0.0, 1.0);
        if cur_val > self.last_peak {
            if let Some(last_pulse) = self.pulses.last() {
                if last_pulse.position < 1.0 {
                    return;
                }
            }

            self.last_peak = cur_val;
            self.pulses.push(Pulse {
                color: accent_color,
                position: 0.0,
                intensity: 1.0,
            });
        }

        self.last_peak *= self.peak_falloff;
    }
}

impl LightingEffect for PixelFlow {
    fn step(&mut self, intensities: &Vec<f32>) -> Vec<palette::LinSrgb> {
        self.advance_pulses();
        self.remove_pulses();
        self.create_pulse(intensities);

        let black = palette::LinSrgba::new(0.0, 0.0, 0.0, 1.0);
        let mut frame_buffer = vec![black; self.pixel_count];

        // Pulse draw pass
        for pulse in &self.pulses {
            let pos = pulse.position;
            let pulse_speed = self.options.lock().unwrap().pulse_speed;

            // Interpolate pulse between the two nearest pixels
            let trailing_pixel = if pulse_speed > 0.0 {
                pos.floor()
            } else {
                pos.ceil()
            };

            let leading_pixel = if pulse_speed > 0.0 {
                pos.ceil()
            } else {
                pos.floor()
            };

            let trailing_alpha = (leading_pixel - pos).abs();
            let leading_alpha = (trailing_pixel - pos).abs();

            frame_buffer[trailing_pixel as usize] = frame_buffer[trailing_pixel as usize]
                .overlay(pulse.color.with_alpha(trailing_alpha));
            frame_buffer[leading_pixel as usize] =
                frame_buffer[leading_pixel as usize].overlay(pulse.color.with_alpha(leading_alpha));

            // TODO Draw pulse trail?
            //let pulse_length = 3.0f32;
        }

        // Alpha baking pass
        let mut baked_buffer = vec![black.color; self.pixel_count];
        for i in 0..baked_buffer.len() {
            baked_buffer[i] = black.overlay(frame_buffer[i]).color;
        }

        return baked_buffer;
    }
}
