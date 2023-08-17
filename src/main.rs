pub(crate) mod audiosource;
pub(crate) mod intervaltimer;
pub(crate) mod olaoutput;
pub(crate) mod osc;
pub(crate) mod photonizer;
pub(crate) mod playbackstate;
pub(crate) mod pulseinput;
pub(crate) mod sdlplayer;

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use olaoutput::OlaOutput;
use photonizer::Photonizer;
use playbackstate::PlaybackState;
use pulseinput::PulseInput;
use sdlplayer::SDLPlayer;

use crate::audiosource::AudioSource;
use crate::osc::OscReceiver;
use crate::osc::OscSender;
use crate::photonizer::PhotonizerOptions;

fn main() {
    let osc_listen_addr = SocketAddr::from_str("0.0.0.0:8000").unwrap();
    let osc_dst_addr = SocketAddr::from_str("192.168.0.141:9000").unwrap();

    let window_size = 1024;
    let playback_state = Arc::new(Mutex::new(PlaybackState::new(window_size)));
    let photonizer_options = Arc::new(Mutex::new(PhotonizerOptions::new()));

    let file_path = "/Users/marcel/Downloads/tmt_s16le.wav";
    let player = SDLPlayer::new(file_path, Arc::clone(&playback_state));
    //let device = "1__Channel_2.monitor".to_string();
    //let mut player = PulseInput::new(Arc::clone(&playback_state), device);

    let ola_addr = SocketAddr::from_str("127.0.0.1:7770").unwrap();
    let ola = match OlaOutput::new(ola_addr) {
        Ok(ola) => ola,
        Err(msg) => panic!("Cannot set up OLA output: {}", msg),
    };

    let osc_sender = match OscSender::new(osc_dst_addr) {
        Ok(osc_sender) => osc_sender,
        Err(msg) => panic!("Cannot set up OSC: {}", msg),
    };

    let mut photonizer = Photonizer::new(
        Arc::clone(&playback_state),
        Arc::clone(&photonizer_options),
        ola,
        osc_sender,
    );

    let osc_receiver = match OscReceiver::new(osc_listen_addr, Arc::clone(&photonizer_options)) {
        Ok(osc_receiver) => osc_receiver,
        Err(msg) => panic!("Cannot set up OSC: {}", msg),
    };

    let res = thread::Builder::new()
        .name("Photonizer".to_string())
        .spawn(move || {
            photonizer.run();
        });
    if let Err(error) = res {
        panic!("Failed to create thread: {}", error);
    }

    let res = thread::Builder::new()
        .name("OSC".to_string())
        .spawn(move || {
            osc_receiver.run();
        });
    if let Err(error) = res {
        panic!("Failed to create thread: {}", error);
    }

    player.run();
}
