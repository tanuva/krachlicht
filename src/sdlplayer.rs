extern crate sdl2;

use core::panic;
use sdl2::{audio::*, AudioSubsystem, Sdl};
use std::rc::Rc;
use std::sync::mpsc;

fn u8_to_i16(v: &[u8]) -> &[i16] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const i16, v.len() / 2) }
}

fn i16_to_f32(v: &[i16]) -> Vec<f32> {
    v.iter()
        .map(|&sample| sample as f32 / i16::MAX as f32)
        .collect()
}

struct WavFileCallback {
    samples: Rc<Vec<i16>>,
    file_pos: usize,
    playback_pos_tx: mpsc::Sender<usize>,
}

impl WavFileCallback {
    fn new(samples: Rc<Vec<i16>>, playback_pos_tx: mpsc::Sender<usize>) -> WavFileCallback {
        WavFileCallback {
            samples,
            playback_pos_tx,
            file_pos: 0,
        }
    }
}

// Needed because AudioSpecWAV does not implement Send by itself
// https://github.com/Rust-SDL2/rust-sdl2/issues/1108
unsafe impl Send for WavFileCallback {}

impl AudioCallback for WavFileCallback {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        for i in 0..out.len() {
            out[i] = self.samples[self.file_pos + i];
        }

        self.file_pos += out.len();
        self.playback_pos_tx
            .send(self.file_pos)
            .expect("Failed to send file_pos");
    }
}

pub struct SDLPlayer {
    sdl_context: Sdl,
    sdl_audio: AudioSubsystem,
    //wav_file_spec: AudioSpecWAV,
    samples: Rc<Vec<i16>>,
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

    pub fn new(file_path: &str, playback_pos_tx: mpsc::Sender<usize>) -> SDLPlayer {
        let sdl_context = sdl2::init().expect("Cannot initialize SDL2 🤷‍♀️");
        let sdl_audio = sdl_context
            .audio()
            .expect("Cannot init SDL audio: {error_msg}");
        let samples = Rc::new(SDLPlayer::get_samples_from_file(file_path));
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

                WavFileCallback::new(Rc::clone(&samples), playback_pos_tx)
            })
            .expect("Cannot open audio device: {error_msg}");

        SDLPlayer {
            sdl_context,
            sdl_audio,
            samples,
            device,
        }
    }

    pub fn start(&self) {
        self.device.resume();
    }

    pub fn get_audio_buffer(&self) -> Vec<f32> {
        let analysis_buffer = i16_to_f32(&self.samples);

        let file_max = self
            .samples
            .iter()
            .reduce(|a, b| if a >= b { a } else { b })
            .expect("");
        let file_min = self
            .samples
            .iter()
            .reduce(|a, b| if a < b { a } else { b })
            .expect("");
        let ana_min = analysis_buffer
            .iter()
            .reduce(|a, b| if a < b { a } else { b })
            .expect("D'oh.");
        let ana_max = analysis_buffer
            .iter()
            .reduce(|a, b| if a >= b { a } else { b })
            .expect("D'oh.");
        println!("Sample extrema of input file: {}/{}", file_min, file_max);
        println!("Sample extrema for analysis: {}/{}", ana_min, ana_max);

        return analysis_buffer;
    }
}
