use dft::{Operation, Plan};
use std::cmp;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct Photonizer {
    samples: Vec<f32>,
    file_pos: usize,
    file_pos_rx: mpsc::Receiver<usize>,
    plan: Plan<f32>,
    window_size: usize,

    frames: u32,
    last_fps_print: Instant,

    interval: Duration,
}

impl Photonizer {
    pub fn new(samples: Vec<f32>, file_pos_rx: mpsc::Receiver<usize>) -> Photonizer {
        let window_size = 1024;

        let update_freq_hz = 30.0;
        let frame_duration_microsec = 1000.0 / update_freq_hz * 1000.0;

        Photonizer {
            samples,
            file_pos: 0,
            file_pos_rx,
            plan: Plan::<f32>::new(Operation::Forward, window_size),
            window_size,

            frames: 0,
            last_fps_print: Instant::now(),

            interval: Duration::from_micros(frame_duration_microsec as u64),
        }
    }

    pub fn run(&mut self) {
        loop {
            let tick_begin = Instant::now();
            self.update();
            self.update_fps();
            self.sleep(&tick_begin);
        }
    }

    fn update(&mut self) {
        // Empty the file_pos channel, we're only interested in the most recent value.
        //let file_pos = self.file_pos_rx.try_recv();
        for recvd_file_pos in self.file_pos_rx.try_iter() {
            //println!("recvd_file_pos: {}", file_pos);
            self.file_pos = recvd_file_pos;
        }

        // FIXME This will result in slices with non-power-of-2 length near EOF
        let analysis_slice_end = cmp::min(self.file_pos + self.window_size, self.samples.len());
        let mut dft_io_data = self.samples[self.file_pos..analysis_slice_end].to_vec();

        dft::transform(&mut dft_io_data, &self.plan);

        // Normalize results
        // https://dsp.stackexchange.com/questions/11376/why-are-magnitudes-normalised-during-synthesis-idft-not-analysis-dft
        let scale_factor = 1.0 / (self.window_size as f32);
        let _intensities: Vec<f32> = dft::unpack(&dft_io_data)
            .iter()
            .map(|c| c.norm() * scale_factor)
            .collect();

        /*let max_intensity = intensities
            .iter()
            .reduce(|a, b| if a >= b { a } else { b })
            .expect("No maximum in output data?!");

        println!("output_max: {}\tbucket[2]: {}", _max_intensity, intensities[2]);*/
    }

    fn update_fps(&mut self) {
        self.frames += 1;

        if Instant::now() - self.last_fps_print > Duration::from_secs(1) {
            println!("FPS: {}", self.frames);
            self.frames = 0;
            self.last_fps_print = Instant::now();
        }
    }

    fn sleep(&self, tick_begin: &Instant) {
        let next_tick = if *tick_begin + self.interval > Instant::now() {
            *tick_begin + self.interval
        } else {
            println!("Photonizer skipped a frame");
            Instant::now() + self.interval
        };

        std::thread::sleep(next_tick - Instant::now());
    }
}
