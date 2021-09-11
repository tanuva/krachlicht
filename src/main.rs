extern crate dft;

pub(crate) mod intervaltimer;
pub(crate) mod photonizer;
pub(crate) mod playbackstate;
pub(crate) mod sdlplayer;

use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use photonizer::Photonizer;
use playbackstate::PlaybackState;
use sdlplayer::SDLPlayer;

fn main() {
    let file_path = "/Users/marcel/Downloads/tmt_s16le.wav";

    let playback_state = Arc::new(Mutex::new(PlaybackState::new()));
    let player = SDLPlayer::new(file_path, Arc::clone(&playback_state));
    let analysis_buffer = player.get_audio_buffer();
    let mut photonizer = Photonizer::new(analysis_buffer, Arc::clone(&playback_state));

    match thread::Builder::new()
        .name("Photonizer".to_string())
        .spawn(move || {
            photonizer.run();
        }) {
        Ok(_) => (),
        Err(error) => panic!("Failed to create thread: {}", error),
    };

    match thread::Builder::new()
        .name("UI".to_string())
        .spawn(move || {
            ui.run();
        }) {
        Ok(_) => (),
        Err(error) => panic!("Failed to create thread: {}", error),
    };

    player.start();
    std::thread::sleep(Duration::from_millis(10 * 1000));
}
