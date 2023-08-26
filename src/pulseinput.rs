extern crate pulse_simple;

use std::sync::{Arc, Mutex};

use pulse_simple::Record;

use crate::{audiosource::AudioSource, playbackstate::PlaybackState};

pub struct PulseInput {
    playback_state: Arc<Mutex<PlaybackState>>,
    pulse: Record<[f32; 1]>,
    buffer: Vec<[f32; 1]>,
}

impl PulseInput {
    pub fn new(playback_state: Arc<Mutex<PlaybackState>>, device: &str) -> Self {
        let sample_rate = 44100;
        let pulse = Record::new(
            "krachlicht",
            "Live audio analyzer",
            Some(&device),
            sample_rate,
        );

        // Pre-filling is necessary according to pulse_simple example
        let window_size = playback_state.lock().unwrap().buffer.capacity();
        let mut buffer = Vec::with_capacity(window_size);
        for _ in 0..buffer.capacity() {
            buffer.push([0.0]);
        }

        PulseInput {
            playback_state,
            pulse,
            buffer,
        }
    }
}

impl AudioSource for PulseInput {
    fn run(&mut self) {
        loop {
            self.pulse.read(&mut self.buffer[..]);
            let mut playback_state = self.playback_state.lock().unwrap();
            playback_state.buffer = self.buffer.iter().map(|v| v[0]).collect();

            if playback_state.shutdown {
                break;
            }
        }
    }
}
