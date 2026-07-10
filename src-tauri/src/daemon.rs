//! Daemon layer (P2): system tray, start-hidden, macOS activation policy.
//!
//! The Rust core stays alive with no window. Closing the window hides to the tray
//! rather than quitting, so sessions and tunnels keep running in the background.

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use tauri_plugin_autostart::ManagerExt;

pub fn setup<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);

    let show = MenuItem::with_id(app, "show", "Show KenkoTerminal", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Start at Login",
        true,
        autostart_on,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &PredefinedMenuItem::separator(app)?,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundled default window icon");

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("KenkoTerminal")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            "autostart" => {
                let mgr = app.autolaunch();
                if mgr.is_enabled().unwrap_or(false) {
                    let _ = mgr.disable();
                } else {
                    let _ = mgr.enable();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Reveal and focus the main window (from the tray or a second-launch attempt).
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        #[cfg(target_os = "macos")]
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}
