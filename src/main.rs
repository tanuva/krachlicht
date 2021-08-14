extern crate dft;

mod photonizer;
mod sdlplayer;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use photonizer::Photonizer;
use sdlplayer::SDLPlayer;

fn main() {
    let file_path = "/Users/marcel/Downloads/tmt_s16le.wav";
    let (file_pos_tx, file_pos_rx) = mpsc::channel();

    let player = SDLPlayer::new(file_path, file_pos_tx);
    let analysis_buffer = player.get_audio_buffer();
    let mut photonizer = Photonizer::new(analysis_buffer, file_pos_rx);

    thread::spawn(move || {
        photonizer.run();
    });

    player.start();
    std::thread::sleep(Duration::from_millis(10 * 1000));
}
