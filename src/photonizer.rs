extern crate dft;

use dft::{Operation, Plan};
use std::sync::{Arc, Mutex};

use crate::intervaltimer::IntervalTimer;
use crate::oscoutput::OscOutput;
use crate::playbackstate::PlaybackState;

fn to_dmx(v: f32) -> u8 {
    (v * 255 as f32) as u8
}

struct Color {
    r: f32,
    g: f32,
    b: f32,
}

impl Color {
    fn scaled(&self, f: f32) -> Color {
        Color {
            r: self.r * f,
            g: self.g * f,
            b: self.b * f,
        }
    }

    fn to_dmx(&self) -> [u8; 3] {
        [to_dmx(self.r), to_dmx(self.g), to_dmx(self.b)]
    }
}

pub struct Photonizer {
    playback_state: Arc<Mutex<PlaybackState>>,
    plan: Plan<f32>,
    window_size: usize,
    timer: IntervalTimer,
    osc: OscOutput,
}

impl Photonizer {
    pub fn new(playback_state: Arc<Mutex<PlaybackState>>, osc: OscOutput) -> Photonizer {
        let update_freq_hz = 30.0;
        let window_size = {
            let playback_state = playback_state.lock().unwrap();
            playback_state.buffer.capacity()
        };

        {
            let mut playback_state = playback_state.lock().unwrap();
            (*playback_state).bucket_count = window_size / 2;
            (*playback_state).freq_step = 44100.0 / window_size as f32;
            println!(
                "Buckets: {}\nBucket bandwidth: {} Hz\nMax frequency: {} Hz",
                playback_state.bucket_count,
                playback_state.freq_step,
                playback_state.bucket_count as f32 * playback_state.freq_step
            );
        }

        Photonizer {
            playback_state,
            plan: Plan::<f32>::new(Operation::Forward, window_size),
            window_size,
            timer: IntervalTimer::new(update_freq_hz, true),
            osc,
        }
    }

    pub fn run(&mut self) {
        loop {
            self.update();
            self.timer.sleep_until_next_tick();
        }
    }

    fn update(&mut self) {
        let mut dft_io_data = self.playback_state.lock().unwrap().buffer.clone();
        dft::transform(&mut dft_io_data, &self.plan);

        // Normalize results
        // https://dsp.stackexchange.com/questions/11376/why-are-magnitudes-normalised-during-synthesis-idft-not-analysis-dft
        // This uses just c.norm() without scaling?!
        // https://github.com/astro/rust-pulse-simple/blob/master/examples/spectrum/src/main.rs
        //let scale_factor = 1.0 / (self.window_size as f32);
        // Chosen by looking at actual output...
        let scale_factor = 1.0 / 300.0;
        let limit: f32 = 1.0;
        let intensities: Vec<f32> = dft::unpack(&dft_io_data)
            .iter()
            .map(|c| limit.min(c.norm() * scale_factor))
            .collect();

        println!(
            "max: {}\tbucket[2]: {}",
            intensities
                .iter()
                .reduce(|a, b| {
                    if a > b {
                        a
                    } else {
                        b
                    }
                })
                .unwrap(),
            intensities[2]
        );

        self.photonize(&intensities);
    }

    fn photonize(&mut self, intensities: &Vec<f32>) {
        self.blink(&intensities);
        self.osc.flush();
    }

    fn blink(&mut self, intensities: &Vec<f32>) {
        let fg_color = Color {
            r: 0.4,
            g: 1.0,
            b: 0.6,
        };

        let scaled = fg_color.scaled(intensities[2]);
        for channel in 0..18 {
            self.osc.set_rgb(channel * 3, scaled.to_dmx());
        }
    }
}
