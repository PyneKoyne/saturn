use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use titan::cpu::error;
use titan::cpu::error::Error::MemoryUnmapped;
use titan::cpu::memory::section::ListenResponder;

pub const KEYBOARD_ADDRESS: u32 = 0xFFFF0000;
pub const KEYBOARD_HOLDING: u32 = 0xFFFF0080;
pub const TRANSMIT: u32 = 0xFFFF000C;
pub const KEYBOARD_SELECTOR: u32 = KEYBOARD_ADDRESS >> 16;

pub struct KeyboardState {
    last: Option<char>,
    keys: Vec<char>,
    holding: [bool; 128],
    socket: Option<UdpSocket>,
}

pub struct KeyboardHandler {
    pub state: Arc<Mutex<KeyboardState>>,
}

impl KeyboardState {
    pub fn push_key(&mut self, key: char, up: bool) {
        if !up {
            self.keys.push(key)
        }

        let offset = key as usize;

        if offset < self.holding.len() {
            self.holding[offset] = !up
        }
    }

    fn pop_key(&mut self) -> Option<char> {
        if let Some(value) = self.keys.pop() {
            self.last = Some(value)
        }

        self.last
    }

    fn poll_network(&mut self) {
        let Some(socket) = &self.socket else {return};
        let mut buf = [0u8; 1];
        while let Ok((1, _)) = socket.recv_from(&mut buf) {
            self.keys.insert(0, buf[0] as char)
        }
    }

    pub fn new() -> KeyboardState {
        let socket = UdpSocket::bind("0.0.0.0:9000").ok();
        if let Some(s) = &socket {
            s.set_nonblocking(true).ok();
            s.set_broadcast(true).ok();
        }
        KeyboardState {
            last: None,
            keys: vec![],
            holding: [false; 128],
            socket,
        }
    }
}

impl KeyboardHandler {
    pub fn new() -> KeyboardHandler {
        KeyboardHandler {
            state: Arc::new(Mutex::new(KeyboardState::new())),
        }
    }
}

impl ListenResponder for KeyboardHandler {
    fn read(&self, address: u32) -> error::Result<u8> {
        let keyboard_handled = KEYBOARD_ADDRESS..KEYBOARD_ADDRESS + 12;
        let keyboard_holding = KEYBOARD_HOLDING..KEYBOARD_HOLDING + 128;

        if keyboard_handled.contains(&address) {
            let offset = address - KEYBOARD_ADDRESS;

            let mut state = self.state.lock().unwrap();

            Ok(match offset {
                0 => {
                    state.poll_network();
                    if state.keys.is_empty() {
                        0
                    } else {
                        1
                    }
                }
                4 => state.pop_key().map(|c| c as u8).unwrap_or(0),
                8 => 1,
                _ => 0,
            })
        } else if keyboard_holding.contains(&address) {
            let offset = address - KEYBOARD_HOLDING;

            let state = self.state.lock().unwrap();

            if let Some(item) = state.holding.get(offset as usize) {
                Ok(*item as u8)
            } else {
                Err(MemoryUnmapped(address))
            }
        } else {
            Err(MemoryUnmapped(address))
        }
    }

    fn write(&mut self, address: u32, value: u8) -> error::Result<()> {
        let transmit_handled = TRANSMIT..TRANSMIT + 4;
        if transmit_handled.contains(&address) {
            if address == TRANSMIT {
                if let Some(socket) = &self.state.lock().unwrap().socket {
                    socket.send_to(&[value], "255.255.255.255:9000").ok();
                }
            }
            Ok(())
        }
        else {
            Err(MemoryUnmapped(address))
        }
    }
}
