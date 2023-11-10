extern crate dft;

use dft::{Operation, Plan};
use palette::LinSrgb;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::effects::lightbar::LightBar;
use crate::effects::pixelflow::PixelFlow;
use crate::effects::staticcolor::StaticColor;
use crate::effects::LightingEffect;
use crate::intervaltimer::IntervalTimer;
use crate::olaoutput::OlaOutput;
use crate::osc::OscSender;
use crate::playbackstate::PlaybackState;

// TODO Implement as a trait on LinSrgb?
fn to_dmx(srgb: palette::LinSrgb) -> [u8; 3] {
    let components = srgb.into_components();
    [
        (components.0 * 255 as f32) as u8,
        (components.1 * 255 as f32) as u8,
        (components.2 * 255 as f32) as u8,
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    LightBar,
    Pixels,
    Static,
}

pub struct PhotonizerOptions {
    pub shutdown: bool, // FIXME This doesn't technically belong here
    pub enabled: bool,
    pub mode: Mode,

    // Simple factors in [0; 1]
    pub master_intensity: f32,
    pub background_intensity: f32,

    // Step value applied every frame
    pub pulse_speed: f32, // TODO Not currently forwarded
    pub accent_color: palette::LinSrgb,
    pub background_color: palette::LinSrgb,
}

impl PhotonizerOptions {
    pub fn new() -> PhotonizerOptions {
        PhotonizerOptions {
            shutdown: false,
            enabled: true,
            mode: Mode::Pixels,

            master_intensity: 1.0,
            background_intensity: 0.0,
            pulse_speed: 0.6,
            accent_color: LinSrgb::new(0.0, 1.0, 0.0),
            background_color: LinSrgb::new(0.0, 0.0, 0.0),
        }
    }
}

pub struct Photonizer {
    playback_state: Arc<Mutex<PlaybackState>>,
    options: Arc<Mutex<PhotonizerOptions>>,
    plan: Plan<f32>,
    window_size: usize,
    timer: IntervalTimer,
    ola: OlaOutput,
    osc: OscSender,

    pixel_count: usize,
    effect: Box<dyn LightingEffect + Send>,
    last_mode: Mode,
    osc_options_sent: Instant,
    blacked_out: bool,
}

impl Photonizer {
    pub fn new(
        playback_state: Arc<Mutex<PlaybackState>>,
        options: Arc<Mutex<PhotonizerOptions>>,
        ola: OlaOutput,
        osc: OscSender,
    ) -> Photonizer {
        const UPDATE_FREQ_HZ: f32 = 30.0;
        const PIXEL_COUNT: usize = 18;

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
            options: Arc::clone(&options),
            plan: Plan::<f32>::new(Operation::Forward, window_size),
            window_size,
            timer: IntervalTimer::new(UPDATE_FREQ_HZ, true),
            ola,
            osc,

            pixel_count: PIXEL_COUNT,
            effect: Box::new(LightBar::new(Arc::clone(&options), PIXEL_COUNT)),
            last_mode: Mode::LightBar,
            osc_options_sent: Instant::now(),
            blacked_out: false,
        }
    }

    pub fn run(&mut self) {
        let mut intensities = vec![0.0f32; self.window_size];

        loop {
            if self.options.lock().unwrap().enabled {
                if self.options.lock().unwrap().mode != Mode::Static {
                    self.transform(&mut intensities);
                }
                self.photonize(&intensities);
                self.send_osc(&intensities);
            } else {
                self.blackout();
            }

            if self.options.lock().unwrap().shutdown {
                break;
            }

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

        let _max_intsy = intensities
            .iter()
            .reduce(|a, b| if a > b { a } else { b })
            .unwrap();
        //println!("max: {}\tbucket[2]: {}", max_intsy, intensities[2]);
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
            self.osc.send_pulse_speed(options.pulse_speed);

            self.osc_options_sent = Instant::now();
        }
    }

    fn blackout(&mut self) {
        if !self.blacked_out {
            self.ola.blackout();
            self.ola.flush();
            self.blacked_out = true;
        }
    }

    fn photonize(&mut self, intensities: &Vec<f32>) {
        let mode = self.options.lock().unwrap().mode;
        if mode != self.last_mode {
            self.effect = match mode {
                Mode::LightBar => {
                    Box::new(LightBar::new(Arc::clone(&self.options), self.pixel_count))
                }
                Mode::Pixels => {
                    Box::new(PixelFlow::new(Arc::clone(&self.options), self.pixel_count))
                }
                Mode::Static => Box::new(StaticColor::new(
                    Arc::clone(&self.options),
                    self.pixel_count,
                )),
            };

            self.last_mode = mode;
        }

        let frame = self.effect.step(intensities);
        let master_intensity = self.options.lock().unwrap().master_intensity;
        for i in 0..frame.len() {
            self.ola
                .set_rgb(i as u8 * 3, to_dmx(frame[i] * master_intensity));
        }
        self.ola.flush();
        self.blacked_out = false;
    }
}
