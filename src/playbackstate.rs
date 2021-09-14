pub struct PlaybackState {
    pub file_pos: usize,
    pub bucket_count: usize,
    pub freq_step: f32,
    pub intensities: Vec<f32>,
}

impl PlaybackState {
    pub fn new() -> PlaybackState {
        PlaybackState {
            file_pos: 0,
            bucket_count: 0,
            freq_step: 0.0,
            intensities: Vec::new(),
        }
    }
}
