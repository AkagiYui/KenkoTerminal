//! Telnet transport (R1). Minimal client: TCP + IAC option negotiation
//! (refuse all options), IAC-unescape inbound, IAC-escape outbound.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::Mutex;
use tauri::ipc::Channel;

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

#[derive(Default)]
enum State {
    #[default]
    Data,
    Iac,
    Opt(u8),
    Sub,
    SubIac,
}

/// Streaming telnet filter: extracts data bytes and produces IAC replies
/// (we refuse every option: WILL->DONT, DO->WONT).
#[derive(Default)]
pub struct TelnetFilter {
    state: State,
}

impl TelnetFilter {
    pub fn feed(&mut self, input: &[u8], replies: &mut Vec<u8>) -> Vec<u8> {
        let mut data = Vec::with_capacity(input.len());
        for &b in input {
            self.state = match std::mem::take(&mut self.state) {
                State::Data if b == IAC => State::Iac,
                State::Data => {
                    data.push(b);
                    State::Data
                }
                State::Iac => match b {
                    IAC => {
                        data.push(IAC); // escaped literal 0xFF
                        State::Data
                    }
                    WILL | WONT | DO | DONT => State::Opt(b),
                    SB => State::Sub,
                    _ => State::Data, // GA/NOP/etc — ignore
                },
                State::Opt(cmd) => {
                    let reply = match cmd {
                        WILL => DONT,
                        DO => WONT,
                        _ => 0, // WONT/DONT need no reply
                    };
                    if reply != 0 {
                        replies.extend_from_slice(&[IAC, reply, b]);
                    }
                    State::Data
                }
                State::Sub if b == IAC => State::SubIac,
                State::Sub => State::Sub,
                State::SubIac if b == SE => State::Data,
                State::SubIac => State::Sub,
            };
        }
        data
    }
}

#[derive(Default)]
pub struct TelnetManager {
    streams: Mutex<HashMap<u32, TcpStream>>,
    next_id: AtomicU32,
}

#[tauri::command]
pub fn telnet_open(
    manager: tauri::State<'_, TelnetManager>,
    host: String,
    port: u16,
    on_output: Channel<Vec<u8>>,
) -> Result<u32, String> {
    let stream = TcpStream::connect((host.as_str(), port)).map_err(|e| e.to_string())?;
    stream.set_nodelay(true).ok();
    let mut reader = stream.try_clone().map_err(|e| e.to_string())?;
    let mut replies_sock = stream.try_clone().map_err(|e| e.to_string())?;
    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);

    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut filter = TelnetFilter::default();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let mut replies = Vec::new();
                    let data = filter.feed(&buf[..n], &mut replies);
                    if !replies.is_empty() {
                        let _ = replies_sock.write_all(&replies);
                    }
                    if !data.is_empty() && on_output.send(data).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    manager.streams.lock().insert(id, stream);
    Ok(id)
}

#[tauri::command]
pub fn telnet_write(
    manager: tauri::State<'_, TelnetManager>,
    id: u32,
    data: String,
) -> Result<(), String> {
    let mut streams = manager.streams.lock();
    let s = streams.get_mut(&id).ok_or("no such telnet session")?;
    let mut esc = Vec::with_capacity(data.len());
    for &b in data.as_bytes() {
        esc.push(b);
        if b == IAC {
            esc.push(IAC); // escape outbound 0xFF
        }
    }
    s.write_all(&esc).map_err(|e| e.to_string())?;
    s.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn telnet_close(manager: tauri::State<'_, TelnetManager>, id: u32) -> Result<(), String> {
    if let Some(s) = manager.streams.lock().remove(&id) {
        let _ = s.shutdown(Shutdown::Both);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_iac_and_replies_refuse() {
        let mut f = TelnetFilter::default();
        let mut replies = Vec::new();
        // "hi" + IAC WILL ECHO(1) + "!" + IAC IAC (literal 0xFF)
        let input = [b'h', b'i', IAC, WILL, 1, b'!', IAC, IAC];
        let data = f.feed(&input, &mut replies);
        assert_eq!(data, vec![b'h', b'i', b'!', 0xFF]);
        assert_eq!(replies, vec![IAC, DONT, 1]); // refused the WILL ECHO
    }

    #[test]
    fn subnegotiation_is_skipped() {
        let mut f = TelnetFilter::default();
        let mut replies = Vec::new();
        let input = [b'a', IAC, SB, 24, b'x', b'y', IAC, SE, b'b'];
        let data = f.feed(&input, &mut replies);
        assert_eq!(data, vec![b'a', b'b']);
        assert!(replies.is_empty());
    }
}
