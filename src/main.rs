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

use clap::Parser;
use olaoutput::OlaOutput;
use photonizer::Photonizer;
use playbackstate::PlaybackState;
use pulseinput::PulseInput;
use sdlplayer::SDLPlayer;

use crate::audiosource::AudioSource;
use crate::osc::OscReceiver;
use crate::osc::OscSender;
use crate::photonizer::PhotonizerOptions;

#[derive(Parser)]
struct Cli {
    /// The s16le wav file to play
    #[arg(short, long, value_name = "FILE")]
    sound_file_path: Option<std::path::PathBuf>,

    /// The PulseAudio device to listen on
    #[arg(short, long, value_name = "DEVICE")]
    pa_device: Option<String>,
}

fn create_player(
    args: &Cli,
    playback_state: Arc<Mutex<PlaybackState>>,
) -> Result<Box<dyn AudioSource>, &str> {
    if let Some(sound_file_path) = args.sound_file_path.as_deref() {
        return Ok(Box::new(SDLPlayer::new(
            sound_file_path.to_str().unwrap(),
            Arc::clone(&playback_state),
        )));
    };

    if let Some(pa_device) = args.pa_device.as_deref() {
        return Ok(Box::new(PulseInput::new(
            Arc::clone(&playback_state),
            pa_device,
        )));
    };

    return Err("No PulseAudio device or audio file given");
}

fn main() {
    let args = Cli::parse();

    let osc_listen_addr = SocketAddr::from_str("0.0.0.0:8000").unwrap();
    let osc_dst_addr = SocketAddr::from_str("192.168.0.141:9000").unwrap();

    let photonizer_options = Arc::new(Mutex::new(PhotonizerOptions::new()));

    let window_size = 1024;
    let playback_state = Arc::new(Mutex::new(PlaybackState::new(window_size)));
    let mut player = match create_player(&args, Arc::clone(&playback_state)) {
        Ok(player) => player,
        Err(err) => panic!("Cannot set up audio source: {}", err),
    };

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
