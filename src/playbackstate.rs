#[derive(Clone)]
pub struct PlaybackState {
    pub shutdown: bool,

    pub buffer: Vec<f32>,

    pub bucket_count: usize,
    pub freq_step: f32,
}

impl PlaybackState {
    pub fn new(window_size: usize) -> PlaybackState {
        let mut buffer = Vec::with_capacity(window_size);
        for _ in 0..buffer.capacity() {
            buffer.push(0.0);
        }

        PlaybackState {
            shutdown: false,

            buffer,

            bucket_count: 0,
            freq_step: 0.0,
        }
    }
}
