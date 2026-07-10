//! Serial transport (P3). Cross-platform via the `serialport` crate.
//!
//! Enumerates ports with USB identity (VID:PID:serial) for device-identity
//! reconnect later, and exposes DTR/RTS control so opening a port does NOT
//! auto-reset a board (the classic Arduino/ESP auto-reset trap).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use serialport::{SerialPort, SerialPortType};
use tauri::ipc::Channel;

#[derive(Serialize)]
pub struct SerialPortEntry {
    pub name: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub serial_number: Option<String>,
    pub product: Option<String>,
}

#[derive(Default)]
pub struct SerialManager {
    ports: Mutex<HashMap<u32, Box<dyn SerialPort>>>,
    next_id: AtomicU32,
}

/// List serial ports with USB identity where available.
#[tauri::command]
pub fn serial_list() -> Vec<SerialPortEntry> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let (vid, pid, serial_number, product) = match p.port_type {
                SerialPortType::UsbPort(u) => (Some(u.vid), Some(u.pid), u.serial_number, u.product),
                _ => (None, None, None, None),
            };
            SerialPortEntry {
                name: p.port_name,
                vid,
                pid,
                serial_number,
                product,
            }
        })
        .collect()
}

#[tauri::command]
pub fn serial_open(
    manager: tauri::State<'_, SerialManager>,
    path: String,
    baud: u32,
    assert_dtr_rts: bool,
    on_output: Channel<Vec<u8>>,
) -> Result<u32, String> {
    let mut port = serialport::new(&path, baud)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|e| e.to_string())?;

    // Do NOT assert DTR/RTS by default — that resets many boards on connect.
    let _ = port.write_data_terminal_ready(assert_dtr_rts);
    let _ = port.write_request_to_send(assert_dtr_rts);

    let mut reader = port.try_clone().map_err(|e| e.to_string())?;
    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);

    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    if on_output.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => break, // port removed / closed
            }
        }
    });

    manager.ports.lock().insert(id, port);
    Ok(id)
}

#[tauri::command]
pub fn serial_write(
    manager: tauri::State<'_, SerialManager>,
    id: u32,
    data: String,
) -> Result<(), String> {
    let mut ports = manager.ports.lock();
    let p = ports.get_mut(&id).ok_or("no such serial port")?;
    p.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    p.flush().map_err(|e| e.to_string())
}

/// Write raw bytes (debugger hex send / custom line endings).
#[tauri::command]
pub fn serial_write_bytes(
    manager: tauri::State<'_, SerialManager>,
    id: u32,
    data: Vec<u8>,
) -> Result<(), String> {
    let mut ports = manager.ports.lock();
    let p = ports.get_mut(&id).ok_or("no such serial port")?;
    p.write_all(&data).map_err(|e| e.to_string())?;
    p.flush().map_err(|e| e.to_string())
}

/// Control DTR/RTS lines (board reset / boot-mode control).
#[tauri::command]
pub fn serial_set_signal(
    manager: tauri::State<'_, SerialManager>,
    id: u32,
    dtr: bool,
    rts: bool,
) -> Result<(), String> {
    let mut ports = manager.ports.lock();
    let p = ports.get_mut(&id).ok_or("no such serial port")?;
    p.write_data_terminal_ready(dtr).map_err(|e| e.to_string())?;
    p.write_request_to_send(rts).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn serial_close(manager: tauri::State<'_, SerialManager>, id: u32) -> Result<(), String> {
    manager.ports.lock().remove(&id);
    Ok(())
}
