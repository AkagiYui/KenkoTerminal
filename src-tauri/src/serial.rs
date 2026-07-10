//! Serial transport (P3/R12). Cross-platform via `serialport`.
//!
//! Enumerates with USB identity (VID:PID:serial). A session survives board resets
//! and re-enumeration: on disconnect it reconnects by device identity (hotplug poll),
//! streaming to the same channel so the terminal/debugger scrollback is preserved.
//! Opening never asserts DTR/RTS (no auto-reset). Includes the esptool reset sequence.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use tauri::ipc::Channel;

#[derive(Serialize, Clone)]
pub struct SerialPortEntry {
    pub name: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub serial_number: Option<String>,
    pub product: Option<String>,
}

type SharedPort = Arc<Mutex<Option<Box<dyn SerialPort>>>>;

struct SerialSession {
    port: SharedPort,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct SerialManager {
    ports: Mutex<HashMap<u32, SerialSession>>,
    next_id: AtomicU32,
}

#[derive(Clone, Default)]
struct Identity {
    vid: Option<u16>,
    pid: Option<u16>,
    serial: Option<String>,
}

fn list_raw() -> Vec<SerialPortInfo> {
    serialport::available_ports().unwrap_or_default()
}

fn identity_of(path: &str) -> Identity {
    for p in list_raw() {
        if p.port_name == path {
            if let SerialPortType::UsbPort(u) = p.port_type {
                return Identity { vid: Some(u.vid), pid: Some(u.pid), serial: u.serial_number };
            }
        }
    }
    Identity::default()
}

/// Find the device again after re-enumeration: match by USB identity first
/// (path can change), then fall back to the original path reappearing.
fn find_device(id: &Identity, orig_path: &str) -> Option<String> {
    let ports = list_raw();
    if id.vid.is_some() {
        for p in &ports {
            if let SerialPortType::UsbPort(u) = &p.port_type {
                if Some(u.vid) == id.vid && Some(u.pid) == id.pid && u.serial_number == id.serial {
                    return Some(p.port_name.clone());
                }
            }
        }
    }
    ports.iter().find(|p| p.port_name == orig_path).map(|p| p.port_name.clone())
}

fn open_port(path: &str, baud: u32, assert_dtr_rts: bool) -> Result<Box<dyn SerialPort>, String> {
    let mut port = serialport::new(path, baud)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|e| e.to_string())?;
    let _ = port.write_data_terminal_ready(assert_dtr_rts);
    let _ = port.write_request_to_send(assert_dtr_rts);
    Ok(port)
}

#[tauri::command]
pub fn serial_list() -> Vec<SerialPortEntry> {
    list_raw()
        .into_iter()
        .map(|p| {
            let (vid, pid, serial_number, product) = match p.port_type {
                SerialPortType::UsbPort(u) => (Some(u.vid), Some(u.pid), u.serial_number, u.product),
                _ => (None, None, None, None),
            };
            SerialPortEntry { name: p.port_name, vid, pid, serial_number, product }
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
    let port = open_port(&path, baud, assert_dtr_rts)?;
    let identity = identity_of(&path);
    let shared: SharedPort = Arc::new(Mutex::new(Some(port)));
    let stop = Arc::new(AtomicBool::new(false));
    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);

    let shared_t = shared.clone();
    let stop_t = stop.clone();
    std::thread::spawn(move || {
        'outer: loop {
            let mut reader = match shared_t.lock().as_ref().and_then(|p| p.try_clone().ok()) {
                Some(r) => r,
                None => return,
            };
            let mut buf = [0u8; 4096];
            loop {
                if stop_t.load(Ordering::Relaxed) {
                    return;
                }
                match reader.read(&mut buf) {
                    Ok(0) => {}
                    Ok(n) => {
                        if on_output.send(buf[..n].to_vec()).is_err() {
                            return;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => break, // device gone → reconnect by identity
                }
            }
            let _ = on_output.send(b"\r\n[serial disconnected - waiting for device...]\r\n".to_vec());
            loop {
                if stop_t.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(800));
                if let Some(newpath) = find_device(&identity, &path) {
                    if let Ok(np) = open_port(&newpath, baud, assert_dtr_rts) {
                        *shared_t.lock() = Some(np);
                        let _ = on_output.send(b"\r\n[serial reconnected]\r\n".to_vec());
                        continue 'outer;
                    }
                }
            }
        }
    });

    manager.ports.lock().insert(id, SerialSession { port: shared, stop });
    Ok(id)
}

fn with_port<F: FnOnce(&mut dyn SerialPort) -> Result<(), String>>(
    manager: &SerialManager,
    id: u32,
    f: F,
) -> Result<(), String> {
    let sessions = manager.ports.lock();
    let s = sessions.get(&id).ok_or("no such serial port")?;
    let mut guard = s.port.lock();
    let p = guard.as_mut().ok_or("serial disconnected")?;
    f(p.as_mut())
}

#[tauri::command]
pub fn serial_write(manager: tauri::State<'_, SerialManager>, id: u32, data: String) -> Result<(), String> {
    with_port(&manager, id, |p| {
        p.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        p.flush().map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn serial_write_bytes(manager: tauri::State<'_, SerialManager>, id: u32, data: Vec<u8>) -> Result<(), String> {
    with_port(&manager, id, |p| {
        p.write_all(&data).map_err(|e| e.to_string())?;
        p.flush().map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn serial_set_signal(manager: tauri::State<'_, SerialManager>, id: u32, dtr: bool, rts: bool) -> Result<(), String> {
    with_port(&manager, id, |p| {
        p.write_data_terminal_ready(dtr).map_err(|e| e.to_string())?;
        p.write_request_to_send(rts).map_err(|e| e.to_string())
    })
}

/// esptool-style reset. `bootloader=false` resets to run; `true` enters the ROM
/// download mode (DTR→GPIO0, RTS→EN classic sequence).
#[tauri::command]
pub fn serial_esp_reset(manager: tauri::State<'_, SerialManager>, id: u32, bootloader: bool) -> Result<(), String> {
    with_port(&manager, id, |p| {
        if bootloader {
            let _ = p.write_data_terminal_ready(false);
            let _ = p.write_request_to_send(true); // EN low
            std::thread::sleep(Duration::from_millis(100));
            let _ = p.write_data_terminal_ready(true); // GPIO0 low
            let _ = p.write_request_to_send(false); // EN high
            std::thread::sleep(Duration::from_millis(50));
            let _ = p.write_data_terminal_ready(false); // GPIO0 high
        } else {
            let _ = p.write_request_to_send(true); // EN low
            std::thread::sleep(Duration::from_millis(100));
            let _ = p.write_request_to_send(false); // EN high
            let _ = p.write_data_terminal_ready(false);
        }
        Ok(())
    })
}

#[tauri::command]
pub fn serial_close(manager: tauri::State<'_, SerialManager>, id: u32) -> Result<(), String> {
    if let Some(s) = manager.ports.lock().remove(&id) {
        s.stop.store(true, Ordering::Relaxed);
        *s.port.lock() = None;
    }
    Ok(())
}
