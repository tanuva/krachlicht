pub(crate) mod intervaltimer;
pub(crate) mod oscoutput;
pub(crate) mod photonizer;
pub(crate) mod playbackstate;
pub(crate) mod pulseinput;
pub(crate) mod sdlplayer;
//pub(crate) mod ui;

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use oscoutput::OscOutput;
use photonizer::Photonizer;
use playbackstate::PlaybackState;
use pulseinput::PulseInput;
use sdlplayer::SDLPlayer;
//use ui::UI;

fn main() {
    let window_size = 1024;
    let playback_state = Arc::new(Mutex::new(PlaybackState::new(window_size)));

    let file_path = "/Users/marcel/Downloads/tmt_s16le.wav";
    let player = SDLPlayer::new(file_path, Arc::clone(&playback_state));
    //let device = "1__Channel_2.monitor".to_string();
    //let mut player = PulseInput::new(Arc::clone(&playback_state), device);

    let address = SocketAddr::from_str("127.0.0.1:7770").unwrap();
    let osc = match OscOutput::new(address) {
        Ok(osc) => osc,
        Err(msg) => panic!("Cannot set up OSC: {}", msg),
    };
    let mut photonizer = Photonizer::new(Arc::clone(&playback_state), osc);
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
    std::thread::sleep(Duration::from_millis(10 * 1000));
}
