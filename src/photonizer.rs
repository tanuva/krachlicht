extern crate dft;

use dft::{Operation, Plan};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::intervaltimer::IntervalTimer;
use crate::olaoutput::OlaOutput;
use crate::osc::OscSender;
use crate::playbackstate::PlaybackState;

fn to_dmx(v: f32) -> u8 {
    (v * 255 as f32) as u8
}

#[derive(Clone, Copy)]
pub enum Mode {
    LightBar,
    Pixels,
}

pub struct PhotonizerOptions {
    pub mode: Mode,

    pub master_intensity: f32,
    pub background_intensity: f32,
    pub pulse_speed: f32,
    pub pulse_width_factor: f32,
}

impl PhotonizerOptions {
    pub fn new() -> PhotonizerOptions {
        PhotonizerOptions {
            mode: Mode::Pixels,

            master_intensity: 1.0,
            background_intensity: 0.0,
            pulse_speed: 0.1,
            pulse_width_factor: 0.5,
        }
    }
}

#[derive(Default, Clone)]
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

    fn mix(&mut self, other: Color) {
        self.r = (self.r + other.r) / 2.0;
        self.g = (self.g + other.g) / 2.0;
        self.b = (self.b + other.b) / 2.0;
        /*self.r = 1.0f32.min(self.r + other.r);
        self.g = 1.0f32.min(self.g + other.g);
        self.b = 1.0f32.min(self.b + other.b);*/
    }

    fn to_dmx(&self) -> [u8; 3] {
        [to_dmx(self.r), to_dmx(self.g), to_dmx(self.b)]
    }
}

struct Pulse {
    position: f32,
    color: Color,
}

pub struct Photonizer {
    playback_state: Arc<Mutex<PlaybackState>>,
    options: Arc<Mutex<PhotonizerOptions>>,
    plan: Plan<f32>,
    window_size: usize,
    timer: IntervalTimer,
    ola: OlaOutput,
    osc: OscSender,

    pulses: Vec<Pulse>,
    osc_options_sent: Instant,
}

impl Photonizer {
    pub fn new(
        playback_state: Arc<Mutex<PlaybackState>>,
        options: Arc<Mutex<PhotonizerOptions>>,
        ola: OlaOutput,
        osc: OscSender,
    ) -> Photonizer {
        const UPDATE_FREQ_HZ: f32 = 30.0;

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

        let pulses = vec![
            Pulse {
                color: Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                },
                position: 1.0,
            },
        ];

        Photonizer {
            playback_state,
            options,
            plan: Plan::<f32>::new(Operation::Forward, window_size),
            window_size,
            timer: IntervalTimer::new(UPDATE_FREQ_HZ, true),
            ola,
            osc,

            pulses,
            osc_options_sent: Instant::now(),
        }
    }

    pub fn run(&mut self) {
        let mut intensities = vec![0.0f32; self.window_size];

        loop {
            self.transform(&mut intensities);
            self.photonize(&intensities);
            self.send_osc(&intensities);
            self.timer.sleep_until_next_tick();
        }
    }

    fn transform(&mut self, intensities: &mut Vec<f32>) {
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
        *intensities = dft::unpack(&dft_io_data)
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

    fn send_osc(&mut self, intensities: &Vec<f32>) {
        self.osc.send_buckets(&intensities[1..13]);

        // Don't spam the network with current option values, only very new
        // OSC listeners are interested in them.
        if self.osc_options_sent.elapsed() > Duration::from_millis(1000) {
            let options = self.options.lock().unwrap();
            self.osc.send_master_intensity(options.master_intensity);
            self.osc
                .send_background_intensity(options.background_intensity);
            self.osc.send_pulse_width(options.pulse_width_factor);
            self.osc.send_pulse_speed(options.pulse_speed);

            self.osc_options_sent = Instant::now();
        }
    }

    fn photonize(&mut self, intensities: &Vec<f32>) {
        let mode = self.options.lock().unwrap().mode;
        match mode {
            Mode::LightBar => self.light_bar(&intensities),
            Mode::Pixels => self.pixel_pulses(&intensities),
        }
        self.ola.flush();
    }

    fn light_bar(&mut self, intensities: &Vec<f32>) {
        let fg_color = Color {
            r: 0.4,
            g: 1.0,
            b: 0.6,
        };

        let scaled = fg_color.scaled(intensities[2]);
        for channel in 0..18 {
            self.ola.set_rgb(channel * 3, scaled.to_dmx());
        }
    }

    fn pixel_pulses(&mut self, intensities: &Vec<f32>) {
    }
}
