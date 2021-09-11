pub struct PlaybackState {
    pub file_pos: usize,
}

impl PlaybackState {
    pub fn new() -> PlaybackState {
        PlaybackState { file_pos: 0 }
    }
}
