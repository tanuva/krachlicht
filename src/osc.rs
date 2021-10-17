use std::{
    default,
    net::{SocketAddr, UdpSocket},
    str::FromStr,
    sync::{Arc, Mutex},
};

use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};

use crate::photonizer::{Mode, PhotonizerOptions};

pub struct OscSender {
    sock: UdpSocket,
    dst_addr: SocketAddr,
}

pub struct OscReceiver {
    sock: UdpSocket,
    options: Arc<Mutex<PhotonizerOptions>>,
}

impl OscSender {
    pub fn new(dst_addr: SocketAddr) -> Result<Self, String> {
        let src_addr = SocketAddr::from_str("0.0.0.0:0").unwrap();
        let sock = match UdpSocket::bind(src_addr) {
            Ok(sock) => sock,
            Err(error) => return Err(error.to_string()),
        };

        Ok(OscSender { sock, dst_addr })
    }

    pub fn send_buckets(&self, intensities: &[f32]) {
        const BUCKET_COUNT: usize = 12;
        assert!(intensities.len() == BUCKET_COUNT);

        let osc_intensities = intensities.iter().map(|v| OscType::Float(*v)).collect();
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/main/graph".to_string(),
            args: osc_intensities,
        }))
        .unwrap();
        self.sock.send_to(&msg_buf, self.dst_addr).unwrap();
    }

    pub fn send_master_intensity(&self, intensity: f32) {
        self.send_float_value("/main/masterIntensity", intensity);
    }

    pub fn send_background_intensity(&self, intensity: f32) {
        self.send_float_value("/main/backgroundIntensity", intensity);
    }

    pub fn send_pulse_width(&self, width_factor: f32) {
        self.send_float_value("/main/pulseWidth", width_factor);
    }

    pub fn send_pulse_speed(&self, pulse_speed: f32) {
        self.send_float_value("/main/pulseSpeed", pulse_speed);
    }

    fn send_float_value(&self, addr: &str, v: f32) {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![OscType::Float(v)],
        }))
        .unwrap();
        self.sock.send_to(&msg_buf, self.dst_addr).unwrap();
    }
}

impl OscReceiver {
    pub fn new(
        listen_addr: SocketAddr,
        options: Arc<Mutex<PhotonizerOptions>>,
    ) -> Result<Self, String> {
        let sock = match UdpSocket::bind(listen_addr) {
            Ok(sock) => sock,
            Err(error) => return Err(error.to_string()),
        };

        Ok(OscReceiver { sock, options })
    }

    pub fn run(&self) {
        let mut buf = [0u8; rosc::decoder::MTU];

        loop {
            match self.sock.recv_from(&mut buf) {
                Ok((size, addr)) => {
                    println!("Received packet with size {} from: {}", size, addr);
                    let packet = decoder::decode(&buf[..size]).unwrap();
                    self.handle_packet(packet);
                }
                Err(e) => {
                    println!("Error receiving from socket: {}", e);
                    break;
                }
            }
        }
    }

    fn handle_packet(&self, packet: OscPacket) {
        match packet {
            OscPacket::Message(msg) => {
                if !self.handle_message(&msg) {
                    println!("OSC address: {}", msg.addr);
                    println!("OSC arguments: {:?}", msg.args);
                }
            }
            OscPacket::Bundle(bundle) => {
                println!("OSC Bundle: {:?}", bundle);
            }
        }
    }

    fn handle_message(&self, msg: &OscMessage) -> bool {
        let mut options = self.options.lock().unwrap();
        match msg.addr.as_str() {
            "/main/lightbar" => {
                options.mode = Mode::LightBar;
                return true;
            }
            "/main/pixels" => {
                options.mode = Mode::Pixels;
                return true;
            }
            "/main/masterIntensity" => {
                match self.handle_float_message(msg) {
                    Ok(intensity) => options.master_intensity = intensity,
                    Err(msg) => println!("{}", msg),
                }
                return true;
            }
            "/main/backgroundIntensity" => {
                match self.handle_float_message(msg) {
                    Ok(intensity) => options.background_intensity = intensity,
                    Err(msg) => println!("{}", msg),
                }
                return true;
            }
            "/main/pulseWidth" => {
                match self.handle_float_message(msg) {
                    Ok(pulse_width) => options.pulse_width_factor = pulse_width,
                    Err(msg) => println!("{}", msg),
                }
                return true;
            }
            "/main/pulseSpeed" => {
                match self.handle_float_message(msg) {
                    Ok(speed) => options.pulse_speed = speed,
                    Err(msg) => println!("{}", msg),
                }
                return true;
            }
            _ => {
                return false;
            }
        }
    }

    fn handle_float_message(&self, msg: &OscMessage) -> Result<f32, String> {
        if let Some(arg) = msg.args.first() {
            if let OscType::Float(intensity) = arg {
                return Ok(*intensity);
            } else {
                return Err(format!(
                    "{} Unexpected OSC parameter type: {:?}",
                    msg.addr, arg
                ));
            }
        } else {
            return Err(format!("{} Missing OSC parameter: float", msg.addr));
        }
    }
}
