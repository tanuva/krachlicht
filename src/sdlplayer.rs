extern crate sdl2;

use core::panic;
use sdl2::{audio::*, AudioSubsystem, Sdl};
use std::sync::{Arc, Mutex};

use crate::playbackstate::PlaybackState;

fn u8_to_i16(v: &[u8]) -> &[i16] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const i16, v.len() / 2) }
}

fn i16_to_f32(v: &[i16]) -> Vec<f32> {
    v.iter()
        .map(|&sample| sample as f32 / i16::MAX as f32)
        .collect()
}

struct WavFileCallback {
    samples: Vec<i16>,
    analysis_buffer: Vec<f32>,
    file_pos: usize,
    playback_state: Arc<Mutex<PlaybackState>>,
}

impl WavFileCallback {
    fn new(samples: Vec<i16>, playback_state: Arc<Mutex<PlaybackState>>) -> WavFileCallback {
        let analysis_buffer = WavFileCallback::convert_audio_buffer(&samples);

        WavFileCallback {
            samples,
            analysis_buffer,
            file_pos: 0,
            playback_state,
        }
    }

    fn convert_audio_buffer(samples: &[i16]) -> Vec<f32> {
        let analysis_buffer = i16_to_f32(samples);

        let _file_max = samples
            .iter()
            .reduce(|a, b| if a >= b { a } else { b })
            .expect("");
        let _file_min = samples
            .iter()
            .reduce(|a, b| if a < b { a } else { b })
            .expect("");
        let _ana_min = analysis_buffer
            .iter()
            .reduce(|a, b| if a < b { a } else { b })
            .expect("D'oh.");
        let _ana_max = analysis_buffer
            .iter()
            .reduce(|a, b| if a >= b { a } else { b })
            .expect("D'oh.");
        //println!("Sample extrema of input file: {}/{}", _file_min, _file_max);
        //println!("Sample extrema for analysis: {}/{}", _ana_min, _ana_max);

        return analysis_buffer;
    }
}

// Needed because AudioSpecWAV does not implement Send by itself
// https://github.com/Rust-SDL2/rust-sdl2/issues/1108
unsafe impl Send for WavFileCallback {}

impl AudioCallback for WavFileCallback {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        let mut playback_state = self.playback_state.lock().unwrap();

        for i in 0..out.len() {
            out[i] = self.samples[self.file_pos + i];
        }

        let window_size = playback_state.buffer.capacity();
        let window_end = self.file_pos + window_size;
        if window_end < self.analysis_buffer.len() {
            (*playback_state).buffer = self.analysis_buffer[self.file_pos..window_end].to_vec();
        }

        self.file_pos += out.len();
    }
}

pub struct SDLPlayer {
    _sdl_context: Sdl,
    _sdl_audio: AudioSubsystem,
    device: AudioDevice<WavFileCallback>,
}

impl SDLPlayer {
    fn get_samples_from_file(file_path: &str) -> Vec<i16> {
        let wav_file_spec =
            AudioSpecWAV::load_wav(file_path).expect("Cannot load wav file: {error_msg}");

        if wav_file_spec.channels != 1
            || wav_file_spec.format != AudioFormat::S16LSB
            || wav_file_spec.freq != 44100
        {
            panic!("WAV file needs to be s16le, 44100 kHz, mono.");
        }

        u8_to_i16(wav_file_spec.buffer()).to_vec()
    }

    pub fn new(file_path: &str, playback_state: Arc<Mutex<PlaybackState>>) -> SDLPlayer {
        let sdl_context = sdl2::init().expect("Cannot initialize SDL2 🤷‍♀️");
        let sdl_audio = sdl_context
            .audio()
            .expect("Cannot init SDL audio: {error_msg}");
        let samples = SDLPlayer::get_samples_from_file(file_path);
        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),
            samples: None, // Default sample buffer size
        };

        let device = sdl_audio
            .open_playback(None, &desired_spec, |spec| {
                if spec.freq != desired_spec.freq.expect("No desired freq?!")
                    || spec.channels != desired_spec.channels.expect("No desired channel count?!")
                {
                    panic!("Actual AudioSpec does not match desired spec.");
                }

                WavFileCallback::new(samples, playback_state)
            })
            .expect("Cannot open audio device: {error_msg}");

        SDLPlayer {
            _sdl_context: sdl_context,
            _sdl_audio: sdl_audio,
            device,
        }
    }

    pub fn run(&self) {
        self.device.resume();
    }
}
