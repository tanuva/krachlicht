extern crate dft;

use dft::{Operation, Plan};
use palette::blend::{Equations, Parameter};
use palette::{Blend, IntoColor, LinSrgb, WithAlpha};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

#[derive(Clone, Copy)]
pub enum Mode {
    LightBar,
    Pixels,
    Static,
}

pub struct PhotonizerOptions {
    pub mode: Mode,

    // Simple factors in [0; 1]
    pub master_intensity: f32,
    pub background_intensity: f32,

    // Step value applied every frame
    pub pulse_speed: f32,
    pub accent_color: palette::LinSrgb,
    pub background_color: palette::LinSrgb,
}

impl PhotonizerOptions {
    pub fn new() -> PhotonizerOptions {
        PhotonizerOptions {
            mode: Mode::Pixels,

            master_intensity: 1.0,
            background_intensity: 0.0,
            pulse_speed: 0.6,
            accent_color: LinSrgb::new(0.0, 1.0, 0.0),
            background_color: LinSrgb::new(0.0, 0.0, 0.0),
        }
    }
}

struct Pulse {
    position: f32,
    color: palette::LinSrgb,
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
    pulses: Vec<Pulse>,
    last_peak: f32,
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

        let pulses = vec![Pulse {
            color: palette::LinSrgb::new(1.0, 0.0, 0.0),
            position: 0.0,
        }];

        let black = palette::LinSrgba::new(0.0, 0.0, 0.0, 0.0);

        Photonizer {
            playback_state,
            options,
            plan: Plan::<f32>::new(Operation::Forward, window_size),
            window_size,
            timer: IntervalTimer::new(UPDATE_FREQ_HZ, true),
            ola,
            osc,

            pixel_count: PIXEL_COUNT,
            pulses,
            last_peak: 0.0,
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

    fn photonize(&mut self, intensities: &Vec<f32>) {
        let mode = self.options.lock().unwrap().mode;
        match mode {
            Mode::LightBar => self.light_bar(&intensities),
            Mode::Pixels => self.pixel_pulses(&intensities),
            Mode::Static => self.static_color(),
        }
        self.ola.flush();
    }

    fn light_bar(&mut self, intensities: &Vec<f32>) {
        const PEAK_FALLOFF: f32 = 0.90;

        let cur_val = intensities[2].clamp(0.0, 1.0);
        if cur_val > self.last_peak {
            self.last_peak = cur_val;
        }

        let blend_mode =
            Equations::from_parameters(Parameter::SourceAlpha, Parameter::OneMinusSourceAlpha);
        let black = palette::LinSrgba::new(0.0, 0.0, 0.0, 1.0);
        let accent_color = self
            .options
            .lock()
            .unwrap()
            .accent_color
            .with_alpha(self.last_peak);
        let blended = accent_color.blend(black, blend_mode).color;
        let master_intensity = self.options.lock().unwrap().master_intensity;

        for pixel in 0..18 {
            self.ola
                .set_rgb(pixel * 3, to_dmx(blended * master_intensity));
        }

        self.last_peak *= PEAK_FALLOFF;
    }

    fn advance_pulses(&mut self) {
        let pulse_speed = self.options.lock().unwrap().pulse_speed;

        for pulse in &mut self.pulses {
            pulse.position += pulse_speed;
        }
    }

    fn remove_pulses(&mut self) {
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
        const PEAK_FALLOFF: f32 = 0.95;

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
            });
        }

        self.last_peak *= PEAK_FALLOFF;
    }

    fn pixel_pulses(&mut self, intensities: &Vec<f32>) {
        self.advance_pulses();
        self.remove_pulses();
        self.create_pulse(intensities);

        let black = palette::LinSrgba::new(0.0, 0.0, 0.0, 1.0);
        let blend_mode =
            Equations::from_parameters(Parameter::SourceAlpha, Parameter::OneMinusSourceAlpha);
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

            frame_buffer[trailing_pixel as usize] = pulse
                .color
                .with_alpha(trailing_alpha)
                .blend(frame_buffer[trailing_pixel as usize], blend_mode);
            frame_buffer[leading_pixel as usize] = pulse
                .color
                .with_alpha(leading_alpha)
                .blend(frame_buffer[leading_pixel as usize], blend_mode);

            // TODO Draw pulse trail?
            //let pulse_length = 3.0f32;
        }

        let master_intensity = self.options.lock().unwrap().master_intensity;
        for i in 0..frame_buffer.len() {
            let pixel = frame_buffer[i];
            //print!("pixel: {:?}\t", pixel);
            let blended = pixel.blend(black, blend_mode).color;
            //println!("blended: {:?}", pixel);
            self.ola
                .set_rgb(i as u8 * 3, to_dmx(blended * master_intensity));
        }
    }

    fn static_color(&mut self) {
        let master_intensity = self.options.lock().unwrap().master_intensity;
        let color = self.options.lock().unwrap().accent_color;

        for pixel_idx in 0..self.pixel_count {
            self.ola
                .set_rgb(pixel_idx as u8 * 3, to_dmx(color * master_intensity))
        }
    }
}
