use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use log;
use mqtt::{Message, Receiver};
use paho_mqtt as mqtt;

use crate::photonizer::{Mode, PhotonizerOptions};

impl From<Mode> for json::JsonValue {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::LightBar => "Light Bar".into(),
            Mode::Pixels => "Pixel Flow".into(),
            Mode::Static => "None".into(),
        }
    }
}

pub struct MqttClient {
    client: mqtt::Client,
    receiver: Receiver<Option<Message>>,
    unique_id: String,
    topics: Topics,
    options: Arc<Mutex<PhotonizerOptions>>,
}

struct Topics {
    state: String,
    state_set: String,
    discovery: String,
}

impl MqttClient {
    fn make_lwt_message(topic: &str) -> mqtt::Message {
        let payload = json::object! {
            available: "offline"
        };

        return mqtt::Message::new_retained(topic, json::stringify(payload), 0);
    }

    pub fn new(
        url: &str,
        discovery_prefix: &str,
        unique_id: &str,
        options: Arc<Mutex<PhotonizerOptions>>,
    ) -> Result<MqttClient, String> {
        let topics = Topics {
            state: format!("krachlicht/{unique_id}/state"),
            state_set: format!("krachlicht/{unique_id}/state/set"),
            discovery: format!("{discovery_prefix}/light/{unique_id}/config"),
        };

        let client = match mqtt::Client::new(url) {
            Ok(client) => client,
            Err(err) => {
                return Err(format!("{:?}", err));
            }
        };

        let conn_opts = mqtt::ConnectOptionsBuilder::new()
            .keep_alive_interval(Duration::from_secs(20))
            .clean_session(true)
            .will_message(MqttClient::make_lwt_message(&topics.state))
            .finalize();

        if let Err(err) = client.connect(conn_opts) {
            return Err(format!("Cannot connect to {}: {:?}", url, err));
        }

        log::info!("Connected to broker at {url}");

        let receiver = client.start_consuming();
        if let Err(err) = client.subscribe(&topics.state_set, 0) {
            return Err(format!(
                "Failed to subscribe to topic {}: {:?}",
                &topics.state_set, err
            ));
        };

        let mqtt_client = MqttClient {
            client,
            receiver,
            unique_id: unique_id.to_string(),
            topics,
            options,
        };

        mqtt_client.publish_discovery();
        mqtt_client.publish_state();
        Ok(mqtt_client)
    }

    fn publish_discovery(&self) {
        if !self.client.is_connected() {
            if let Err(err) = self.client.reconnect() {
                log::warn!("Reconnection failed: {err}");
            }
        }

        let payload = json::object! {
            schema: "json",
            device_class: "light",
            device: {
                identifiers: self.unique_id.to_string(),
                manufacturer: "Marcel Kummer",
                model: "krachlicht",
                name: "krachlicht",
            },
            unique_id: self.unique_id.to_string(),
            name: "krachlicht",
            brightness: true,
            color_mode: true,
            supported_color_modes: json::array! { "rgb" },

            effect: true,
            effect_list: json::array! { "None", "Light Bar", "Pixel Flow" },

            availability_topic: self.topics.state.to_string(),
            availability_template: "{{ value_json.available }}",

            state_topic: self.topics.state.to_string(),
            command_topic: self.topics.state_set.to_string(),
            //brightness_state_topic: self.topics.state.to_string(),
            //brightness_command_topic: self.topics.state_set.to_string(),
            //brightness_value_template: "{{ value_json.brightness }}",
            //rgb_state_topic: self.topics.state.to_string(),
            //rgb_command_topic: self.topics.state_set.to_string(),
            //rgb_value_template: "{{ value_json.rgb | join(',') }}",
        };

        let payload_str = json::stringify(payload);
        let msg = mqtt::Message::new_retained(&self.topics.discovery, payload_str.clone(), 0);
        log::info!("Publishing {}: {}", self.topics.discovery, &payload_str);
        if let Err(err) = self.client.publish(msg) {
            log::warn!("Failed to publish HomeAssistant discovery: {err}");
        }
    }

    fn publish_state(&self) {
        // TODO Is this even needed?
        if !self.client.is_connected() {
            if let Err(err) = self.client.reconnect() {
                log::warn!("Reconnection failed: {err}");
                return;
            }
        }

        let options = self.options.lock().unwrap();
        let accent_rgb = options.accent_color.into_components();
        let payload = json::object! {
            available: "online",
            state: if options.enabled { "ON" } else { "OFF" },
            brightness: (options.master_intensity * 255 as f32) as u8,
            color: json::object! {
                r: (accent_rgb.0 * 255 as f32) as u8,
                g: (accent_rgb.1 * 255 as f32) as u8,
                b: (accent_rgb.2 * 255 as f32) as u8,
            },
            effect: options.mode,
        };

        let payload_str = json::stringify(payload);
        let msg = mqtt::Message::new_retained(&self.topics.state, payload_str.clone(), 0);
        log::info!("Publishing {}: {}", self.topics.state, &payload_str);
        if let Err(err) = self.client.publish(msg) {
            log::warn!("Publishing failed: {err}");
            return;
        }
    }

    pub fn run(&self) {
        loop {
            match self.receiver.recv() {
                Ok(msg) => {
                    if let Some(msg) = msg {
                        self.handle_message(msg);
                        self.publish_state();
                    }
                }
                Err(err) => log::warn!("Error receiving messages: {err}"),
            };
        }
    }

    fn handle_message(&self, msg: Message) {
        let json = match json::parse(&msg.payload_str()) {
            Ok(json) => json,
            Err(err) => {
                log::warn!(
                    "Failed to parse message payload from {}: {}",
                    msg.topic(),
                    err
                );
                return;
            }
        };

        log::info!(
            "Received {}: {}",
            msg.topic(),
            json::stringify(json.clone())
        );

        let mut options = self.options.lock().unwrap();
        if json.has_key("state") {
            if json["state"] == "ON" {
                options.enabled = true;
            } else if json["state"] == "OFF" {
                options.enabled = false;
            } else {
                log::warn!("Unexpected state value: {}", json["state"]);
            }
        }

        if json.has_key("brightness") {
            match json["brightness"].as_f32() {
                Some(brightness) => options.master_intensity = brightness / 255.0,
                None => log::warn!("Unexpected brightness value: {}", json["brightness"]),
            }
        }

        if json.has_key("color") {
            let json_color = &json["color"];
            if !json_color.has_key("r") || !json_color.has_key("g") || !json_color.has_key("b") {
                log::warn!("Unexpected color format: {json_color}");
            }

            match json_color["r"].as_f32() {
                Some(r) => options.accent_color.red = r / 255.0,
                None => log::warn!("Unexpected red value: {}", json_color["r"]),
            }
            match json_color["g"].as_f32() {
                Some(g) => options.accent_color.green = g / 255.0,
                None => log::warn!("Unexpected green value: {}", json_color["g"]),
            }
            match json_color["b"].as_f32() {
                Some(b) => options.accent_color.blue = b / 255.0,
                None => log::warn!("Unexpected blue value: {}", json_color["b"]),
            }
        }

        if json.has_key("effect") {
            match json["effect"].as_str() {
                Some(effect) => match effect {
                    "None" => options.mode = Mode::Static,
                    "Light Bar" => options.mode = Mode::LightBar,
                    "Pixel Flow" => options.mode = Mode::Pixels,
                    &_ => log::warn!("Unexpected effect: {effect}"),
                },
                None => log::warn!("Unexpected effect value: {}", json["effect"]),
            }
        }
    }
}

impl Drop for MqttClient {
    fn drop(&mut self) {
        if let Err(err) = self.client.disconnect(None) {
            // We don't really care about errors here, but let's make rustc happy.
            log::error!("{:?}", err);
        }
    }
}
