extern crate tui;

use std::io::{self, Stdout};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::u64;
use termion::event;
use termion::raw::{IntoRawMode, RawTerminal};
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{BarChart, Block, Borders};
use tui::Terminal;

use crate::intervaltimer::IntervalTimer;
use crate::playbackstate::PlaybackState;

pub struct UI {
    terminal: Terminal<TermionBackend<RawTerminal<Stdout>>>,
    timer: IntervalTimer,
    playback_state: Arc<Mutex<PlaybackState>>,
    spectrum_labels: Vec<String>,
}

impl UI {
    fn prepare_spectrum_labels(playback_state: Arc<Mutex<PlaybackState>>) -> Vec<String> {
        /*let labels: Vec<String> = [].to_vec();
        for i in 0..12 {
            labels.push(i.to_string());
        }

        return labels;*/

        (0..12).map(|i| i.to_string()).collect()
    }

    /*fn map_spectrum_values(&self) -> Vec<(&'static str, u64)> {

    }*/

    pub fn new(playback_state: Arc<Mutex<PlaybackState>>) -> UI {
        let stdout = io::stdout()
            .into_raw_mode()
            .expect("Failed to bring stdout into raw mode");
        let backend = TermionBackend::new(stdout);
        let spectrum_labels = UI::prepare_spectrum_labels(Arc::clone(&playback_state));

        UI {
            terminal: Terminal::new(backend).expect("Failed to create a Terminal"),
            timer: IntervalTimer::new(30.0, true),
            playback_state,
            spectrum_labels,
        }
    }

    pub fn run(&mut self) {
        self.terminal.clear();

        loop {
            let playback_state = {
                let playback_state = self.playback_state.lock().unwrap();
                (*playback_state).clone()
            };

            let spectrum_labels: Vec<String> = (0..12).map(|i| i.to_string()).collect();
            /*let owned_data: Vec<(String, u64)> = spectrum_labels
            .into_iter()
            .enumerate()
            .map(|(k, v)| (v, intensities[k] as u64 * std::u64::MAX))
            .collect();*/

            let converted_intsies: Vec<u64> = playback_state
                .intensities
                .iter()
                // TODO Conversion is broken!
                .map(|v| *v as u64 * std::u64::MAX)
                .collect();
            let prepared_data: Vec<_> = spectrum_labels
                .iter()
                .zip(converted_intsies)
                .map(|(k, v)| (k.as_str(), v))
                .collect();

            self.terminal
                .draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .margin(1)
                        .constraints([Constraint::Percentage(100)].as_ref())
                        .split(f.size());

                    let spectrum_chart = BarChart::default()
                        .block(Block::default().title("Spectrum").borders(Borders::ALL))
                        .bar_width(2)
                        .bar_gap(0)
                        .bar_style(Style::default().fg(Color::Yellow).bg(Color::Red))
                        .value_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                        .label_style(Style::default().fg(Color::White))
                        .data(&prepared_data)
                        .max(12);
                    f.render_widget(spectrum_chart, chunks[0]);
                })
                .expect("D'oh!");
            self.timer.sleep_until_next_tick();
        }
    }
}
