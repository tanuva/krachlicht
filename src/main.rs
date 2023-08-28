pub(crate) mod audiosource;
pub(crate) mod intervaltimer;
pub(crate) mod mqtt;
pub(crate) mod olaoutput;
pub(crate) mod osc;
pub(crate) mod photonizer;
pub(crate) mod playbackstate;
pub(crate) mod pulseinput;
pub(crate) mod sdlplayer;

use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::process;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use clap::Parser;
use config_file::FromConfigFile;
use log;
use mqtt::MqttClient;
use olaoutput::OlaOutput;
use photonizer::Photonizer;
use playbackstate::PlaybackState;
use pulseinput::PulseInput;
use sdlplayer::SDLPlayer;
use serde::Deserialize;

use crate::audiosource::AudioSource;
use crate::osc::OscReceiver;
use crate::osc::OscSender;
use crate::photonizer::PhotonizerOptions;

/// krachlicht creates blinkenlights from sound
#[derive(Parser)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "CONFIG_FILE")]
    config_file_path: Option<PathBuf>,

    /// The s16le wav file to play
    #[arg(short = 'f', long, value_name = "FILE")]
    sound_file_path: Option<PathBuf>,

    /// The PulseAudio device to listen on
    #[arg(short = 'd', long, value_name = "DEVICE")]
    pa_device: Option<String>,
}

#[derive(Deserialize)]
struct Config {
    pa_device: Option<String>,
    sound_file_path: Option<PathBuf>,

    osc_listen_addr: String,
    osc_dst_addr: String,

    ola_host_addr: String,

    mqtt_broker_url: String,
    mqtt_discovery_prefix: String,
    mqtt_unique_id: String,
}

fn read_config(args: &Cli) -> Result<Config, String> {
    let config_path = match &args.config_file_path {
        Some(path) => path.to_owned(),
        None => PathBuf::from_str("krachlicht.toml").unwrap(),
    };

    let config = match Config::from_config_file(config_path) {
        Ok(config) => config,
        Err(err) => return Err(format!("Cannot read configuration file: {:?}", err)),
    };

    return Ok(config);
}

fn validate_config(args: &Cli, disk_config: &Config) -> Result<Config, String> {
    if args.pa_device.is_some() && args.sound_file_path.is_some() {
        return Err(format!(
            "Must not provide both a PulseAudio device and a sound file"
        ));
    }

    let config = Config {
        pa_device: if args.pa_device.is_some() {
            args.pa_device.clone()
        } else {
            disk_config.pa_device.clone()
        },
        sound_file_path: if args.sound_file_path.is_some() {
            args.sound_file_path.clone()
        } else {
            disk_config.sound_file_path.clone()
        },
        osc_listen_addr: disk_config.osc_listen_addr.clone(),
        osc_dst_addr: disk_config.osc_dst_addr.clone(),

        ola_host_addr: disk_config.ola_host_addr.clone(),

        mqtt_broker_url: disk_config.mqtt_broker_url.clone(),
        mqtt_discovery_prefix: disk_config.mqtt_discovery_prefix.clone(),
        mqtt_unique_id: disk_config.mqtt_unique_id.clone(),
    };

    return Ok(config);
}

fn create_player(
    config: &Config,
    playback_state: Arc<Mutex<PlaybackState>>,
) -> Result<Box<dyn AudioSource>, &str> {
    if let Some(sound_file_path) = config.sound_file_path.as_deref() {
        return Ok(Box::new(SDLPlayer::new(
            sound_file_path.to_str().unwrap(),
            Arc::clone(&playback_state),
        )));
    };

    if let Some(pa_device) = config.pa_device.as_deref() {
        return Ok(Box::new(PulseInput::new(
            Arc::clone(&playback_state),
            pa_device,
        )));
    }

    return Err("No PulseAudio device or audio file given");
}

fn main() {
    env_logger::init();

    let args = Cli::parse();
    let disk_config = match read_config(&args) {
        Ok(disk_config) => disk_config,
        Err(msg) => {
            log::error!("{}", msg);
            process::exit(1);
        }
    };
    let config = match validate_config(&args, &disk_config) {
        Ok(config) => config,
        Err(msg) => {
            log::error!("Failed to validate configuration: {}", msg);
            process::exit(1);
        }
    };

    let photonizer_options = Arc::new(Mutex::new(PhotonizerOptions::new()));

    let window_size = 1024;
    let playback_state = Arc::new(Mutex::new(PlaybackState::new(window_size)));
    let mut player = match create_player(&config, Arc::clone(&playback_state)) {
        Ok(player) => player,
        Err(err) => {
            log::error!("Cannot set up audio source: {}", err);
            process::exit(1);
        }
    };

    let ola = match OlaOutput::new(&config.ola_host_addr) {
        Ok(ola) => ola,
        Err(msg) => {
            log::error!("Cannot set up OLA output: {}", msg);
            process::exit(1);
        }
    };

    let osc_sender = match OscSender::new(&config.osc_dst_addr) {
        Ok(osc_sender) => osc_sender,
        Err(msg) => {
            log::error!("Cannot set up OSC publisher: {}", msg);
            process::exit(1);
        }
    };

    let mut photonizer = Photonizer::new(
        Arc::clone(&playback_state),
        Arc::clone(&photonizer_options),
        ola,
        osc_sender,
    );

    let osc_receiver =
        match OscReceiver::new(&config.osc_listen_addr, Arc::clone(&photonizer_options)) {
            Ok(osc_receiver) => osc_receiver,
            Err(msg) => {
                log::error!("Cannot set up OSC receiver: {}", msg);
                process::exit(1);
            }
        };

    let mqtt_client = match MqttClient::new(
        &config.mqtt_broker_url,
        &config.mqtt_discovery_prefix,
        &config.mqtt_unique_id,
        Arc::clone(&photonizer_options),
    ) {
        Ok(mqtt_client) => mqtt_client,
        Err(msg) => {
            log::error!("Cannot set up MQTT client: {msg}");
            process::exit(1);
        }
    };

    ctrlc::set_handler(move || {
        log::info!("Interrupted, shutting down...");
        let mut options = photonizer_options.lock().unwrap();
        options.shutdown = true;
        let mut state = playback_state.lock().unwrap();
        state.shutdown = true;
    })
    .expect("Error setting Ctrl-C handler");

    let res = thread::Builder::new()
        .name("Photonizer".to_string())
        .spawn(move || {
            photonizer.run();
        });
    if let Err(err) = res {
        log::error!("Failed to create thread: {}", err);
        process::exit(1);
    }

    let res = thread::Builder::new()
        .name("OSC".to_string())
        .spawn(move || {
            osc_receiver.run();
        });
    if let Err(err) = res {
        log::error!("Failed to create thread: {}", err);
        process::exit(1);
    }

    let res = thread::Builder::new()
        .name("MQTT".to_string())
        .spawn(move || {
            mqtt_client.run();
        });
    if let Err(err) = res {
        log::error!("Failed to create thread: {}", err);
        process::exit(1);
    }

    player.run();
}
