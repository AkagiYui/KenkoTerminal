pub mod batch;
mod config;
mod connections;
mod daemon;
mod monitor;
mod pty;
mod serial;
pub mod sftp;
pub mod ssh;
mod ssh_config;
mod telnet;
pub mod tunnel;

use tauri::Manager;

use monitor::MonitorManager;
use pty::PtyManager;
use serial::SerialManager;
use sftp::SftpManager;
use ssh::SshManager;
use telnet::TelnetManager;
use tunnel::TunnelManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Single instance must be registered first; a second launch reveals the window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            daemon::show_main(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(PtyManager::default())
        .manage(SshManager::default())
        .manage(SerialManager::default())
        .manage(SftpManager::default())
        .manage(MonitorManager::default())
        .manage(TelnetManager::default())
        .manage(TunnelManager::default())
        .invoke_handler(tauri::generate_handler![
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            ssh::ssh_connect,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_close,
            serial::serial_list,
            serial::serial_open,
            serial::serial_write,
            serial::serial_write_bytes,
            serial::serial_set_signal,
            serial::serial_esp_reset,
            serial::serial_close,
            sftp::sftp_connect,
            sftp::sftp_list,
            sftp::sftp_realpath,
            sftp::sftp_read,
            sftp::sftp_write,
            sftp::sftp_mkdir,
            sftp::sftp_remove,
            sftp::sftp_rename,
            sftp::sftp_close,
            monitor::probe_system,
            monitor::monitor_start,
            monitor::monitor_stop,
            telnet::telnet_open,
            telnet::telnet_write,
            telnet::telnet_close,
            batch::batch_run,
            connections::conn_list,
            connections::conn_save,
            connections::conn_delete,
            connections::conn_password,
            tunnel::tunnel_list,
            tunnel::tunnel_add,
            tunnel::tunnel_remove,
        ])
        .setup(|app| {
            // Launch as a background/tray app on macOS: no dock icon, no window.
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            daemon::setup(app)?;
            // Auto-start persisted tunnels on launch (R5).
            let manager = app.state::<TunnelManager>();
            for rule in config::load_tunnels() {
                if rule.enabled {
                    manager.start(rule);
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close hides to tray instead of quitting — the daemon stays alive.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                {
                    let _ = window
                        .app_handle()
                        .set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
