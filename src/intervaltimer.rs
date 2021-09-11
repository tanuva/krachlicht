use std::thread;
use std::time::{Duration, Instant};

pub struct IntervalTimer {
    interval: Duration,
    last_tick: Instant,
    thread_name: String,
    measure_fps: bool,
    last_fps_print: Instant,
    frames: u32,
}

impl IntervalTimer {
    pub fn new(freq_hz: f32, measure_fps: bool) -> IntervalTimer {
        let frame_duration_microsec = 1000.0 / freq_hz * 1000.0;
        let cur_thread = thread::current();
        let thread_name = if let Some(name) = cur_thread.name() {
            name
        } else {
            "unnamed"
        };

        IntervalTimer {
            interval: Duration::from_micros(frame_duration_microsec as u64),
            last_tick: Instant::now(),
            thread_name: thread_name.to_string(),
            measure_fps,
            last_fps_print: Instant::now(),
            frames: 0,
        }
    }

    pub fn sleep_until_next_tick(&mut self) {
        if self.measure_fps {
            self.update_fps();
        }

        let next_tick = if self.last_tick + self.interval > Instant::now() {
            self.last_tick + self.interval
        } else {
            println!("Photonizer skipped a frame");
            Instant::now() + self.interval
        };

        std::thread::sleep(next_tick - Instant::now());
        self.last_tick = next_tick
    }

    fn update_fps(&mut self) {
        self.frames += 1;

        if Instant::now() - self.last_fps_print > Duration::from_secs(1) {
            println!("{} FPS: {}", self.thread_name, self.frames);
            self.frames = 0;
            self.last_fps_print = Instant::now();
        }
    }
}
