pub(crate) mod intervaltimer;
pub(crate) mod photonizer;
pub(crate) mod playbackstate;
pub(crate) mod pulseinput;
pub(crate) mod sdlplayer;
//pub(crate) mod ui;

use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use photonizer::Photonizer;
use playbackstate::PlaybackState;
use pulseinput::PulseInput;
use sdlplayer::SDLPlayer;
//use ui::UI;

fn main() {
    let window_size = 1024;
    let playback_state = Arc::new(Mutex::new(PlaybackState::new(window_size)));

    //let file_path = "/Users/marcel/Downloads/tmt_s16le.wav";
    //let player = SDLPlayer::new(file_path, Arc::clone(&playback_state));

    let device = "1__Channel_2.monitor".to_string();
    let mut player = PulseInput::new(Arc::clone(&playback_state), device);

    let mut photonizer = Photonizer::new(Arc::clone(&playback_state));
    //let mut ui = UI::new(Arc::clone(&playback_state));

    let res = thread::Builder::new()
        .name("Photonizer".to_string())
        .spawn(move || {
            photonizer.run();
        });
    if let Err(error) = res {
        panic!("Failed to create thread: {}", error);
    }

    /*let res = thread::Builder::new()
        .name("UI".to_string())
        .spawn(move || {
            ui.run();
        });
    if let Err(error) = res {
        panic!("Failed to create thread: {}", error);
    }*/

    player.run();
    //std::thread::sleep(Duration::from_millis(3 * 1000));
}
