use std::{
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    str::FromStr,
};

use rosc::{encoder, OscMessage, OscPacket, OscType};

pub struct OlaOutput {
    sock: UdpSocket,
    target_addr: SocketAddr,
    buffer: Vec<u8>,
}

impl OlaOutput {
    pub fn new(target_addr: &(impl ToSocketAddrs + std::fmt::Debug)) -> Result<Self, String> {
        let our_addr = SocketAddr::from_str("127.0.0.1:0").unwrap();
        let sock = match UdpSocket::bind(our_addr) {
            Ok(sock) => sock,
            Err(error) => return Err(error.to_string()),
        };

        let mut addr_iter = match target_addr.to_socket_addrs() {
            Ok(iter) => iter,
            Err(err) => {
                return Err(format!("Cannot connect to OLA daemon: {err}"));
            }
        };
        let resolved_addr = match addr_iter.next() {
            Some(addr) => addr,
            None => {
                return Err(format!(
                    "Cannot connect to OLA daemon: No socket addresses for {:?}",
                    target_addr
                ));
            }
        };

        let mut buffer = Vec::with_capacity(512);
        for _ in 0..buffer.capacity() {
            buffer.push(0);
        }

        Ok(OlaOutput {
            sock,
            target_addr: resolved_addr,
            buffer,
        })
    }

    pub fn set(&mut self, channel: u8, value: u8) {
        self.buffer[channel as usize] = value;
    }

    pub fn set_rgb(&mut self, start_channel: u8, values: [u8; 3]) {
        for i in 0..3 {
            self.set(start_channel + i, values[i as usize]);
        }
    }

    pub fn flush(&mut self) {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/dmx/universe/0".to_string(),
            args: vec![OscType::Blob(Vec::clone(&self.buffer))],
        }))
        .unwrap();
        if let Err(err) = self.sock.send_to(&msg_buf, self.target_addr) {
            log::warn!("Failed to send OSC data: {err}");
        }
        self.blackout();
    }

    pub fn blackout(&mut self) {
        for i in 0..self.buffer.capacity() {
            self.buffer[i] = 0;
        }
    }
}
