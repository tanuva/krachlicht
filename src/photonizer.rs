use dft::{Operation, Plan};
use std::cmp;
use std::sync::{Arc, Mutex};

use crate::intervaltimer::IntervalTimer;
use crate::playbackstate::PlaybackState;

pub struct Photonizer {
    samples: Vec<f32>,
    playback_state: Arc<Mutex<PlaybackState>>,
    plan: Plan<f32>,
    window_size: usize,
    timer: IntervalTimer,
}

impl Photonizer {
    pub fn new(samples: Vec<f32>, playback_state: Arc<Mutex<PlaybackState>>) -> Photonizer {
        let window_size = 1024;
        let update_freq_hz = 30.0;

        {
            let mut playback_state = playback_state.lock().unwrap();
            (*playback_state).bucket_count = window_size / 2;
            (*playback_state).freq_step = 44100.0 / window_size as f32;
        }

        Photonizer {
            samples,
            playback_state,
            plan: Plan::<f32>::new(Operation::Forward, window_size),
            window_size,
            timer: IntervalTimer::new(update_freq_hz, true),
        }
    }

    pub fn run(&mut self) {
        loop {
            self.update();
            self.timer.sleep_until_next_tick();
        }
    }

    fn update(&mut self) {
        let playback_state = self.playback_state.lock().unwrap();
        let file_pos = (*playback_state).file_pos;

        // FIXME This will result in slices with non-power-of-2 length near EOF
        let analysis_slice_end = cmp::min(file_pos + self.window_size, self.samples.len());
        let mut dft_io_data = self.samples[file_pos..analysis_slice_end].to_vec();

        dft::transform(&mut dft_io_data, &self.plan);

        // Normalize results
        // https://dsp.stackexchange.com/questions/11376/why-are-magnitudes-normalised-during-synthesis-idft-not-analysis-dft
        let scale_factor = 1.0 / (self.window_size as f32);
        let intensities: Vec<f32> = dft::unpack(&dft_io_data)
            .iter()
            .map(|c| c.norm() * scale_factor)
            .collect();

        let mut playback_state = self.playback_state.lock().unwrap();
        (*playback_state).intensities = intensities;
    }
}
