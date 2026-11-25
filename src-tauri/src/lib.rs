mod airport;
mod api;
mod autostart;
mod commands;
mod company;
mod config;
mod elevated;
mod error;
mod paths;
mod process;
mod settings;
mod state;
mod sysproxy;

use process::CoreState;
use settings::AppSettings;
use state::SettingsState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WindowEvent,
};

const TRAY_ID: &str = "main";

fn tray_notify(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("tray-message", message.into());
}

fn tray_icon_image(proxy_on: bool) -> tauri::image::Image<'static> {
    // Full-bleed transparent leaf (no plate); 32px matches Win tray slot.
    let bytes: &'static [u8] = if proxy_on {
        include_bytes!("../icons/tray-on-32.png")
    } else {
        include_bytes!("../icons/tray-off-32.png")
    };
    tauri::image::Image::from_bytes(bytes).expect("tray icon png")
}

/// Update tray glyph + tooltip to match the master proxy switch.
pub fn sync_tray_icon(app: &tauri::AppHandle, proxy_on: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(tray_icon_image(proxy_on)));
        let tip = if proxy_on {
            "AClash · 代理已开启"
        } else {
            "AClash · 代理未开启"
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

/// Keep the process singleton so we do not create two tray icons.
#[cfg(windows)]
fn ensure_single_instance() -> bool {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::Threading::CreateMutexW;

    const ERROR_ALREADY_EXISTS: u32 = 183;
    static MUTEX_HANDLE: AtomicIsize = AtomicIsize::new(0);

    let name: Vec<u16> = "Local\\AClash.SingleInstance\0".encode_utf16().collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return true;
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        false
    } else {
        MUTEX_HANDLE.store(handle as isize, Ordering::SeqCst);
        true
    }
}

#[cfg(not(windows))]
fn ensure_single_instance() -> bool {
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if !ensure_single_instance() {
        // Second launch: try to focus the existing window via a simple exit.
        // The first instance already owns the tray.
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(CoreState::new())
        .setup(|app| {
            let settings = AppSettings::load().unwrap_or_default();
            let proxy_on = settings.proxy_enabled;
            let _ = crate::config::regenerate(&settings);
            app.manage(SettingsState(std::sync::Mutex::new(settings)));

            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
            let binary = paths::resolve_mihomo_binary(&resource_dir).or_else(|_| {
                // Dev fallback: src-tauri/resources/mihomo.exe next to Cargo.toml
                let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources")
                    .join("mihomo.exe");
                if dev.is_file() {
                    Ok(dev)
                } else {
                    Err(error::AppError::from("mihomo.exe not found"))
                }
            })?;
            process::set_binary(&*app.state::<CoreState>(), binary);

            let show_i = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let update_i =
                MenuItem::with_id(app, "update_airport", "更新机场", true, None::<&str>)?;
            let tun_i = MenuItem::with_id(app, "mode_tun", "切换 TUN", true, None::<&str>)?;
            let system_i =
                MenuItem::with_id(app, "mode_system", "切换系统代理", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &update_i, &tun_i, &system_i, &quit_i])?;

            let _tray = TrayIconBuilder::with_id(TRAY_ID)
                .icon(tray_icon_image(proxy_on))
                .menu(&menu)
                .tooltip(if proxy_on {
                    "AClash · 代理已开启"
                } else {
                    "AClash · 代理未开启"
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "update_airport" => {
                        let app2 = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let settings_state = app2.state::<SettingsState>();
                            match commands::update_airport(app2.clone(), settings_state, None).await
                            {
                                Ok(_) => tray_notify(&app2, "机场订阅已更新"),
                                Err(e) => tray_notify(&app2, format!("更新机场失败：{e}")),
                            }
                        });
                    }
                    "mode_tun" => {
                        let app2 = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let settings_state = app2.state::<SettingsState>();
                            match commands::set_tun(app2.clone(), settings_state, true).await {
                                Ok(_) => tray_notify(&app2, "已切换到 TUN 模式"),
                                Err(e) => tray_notify(&app2, format!("切换 TUN 失败：{e}")),
                            }
                        });
                    }
                    "mode_system" => {
                        let app2 = app.clone();
                        tauri::async_runtime::spawn(async move {
                            // Only leave TUN; do not force-enable the master proxy switch.
                            let settings_state = app2.state::<SettingsState>();
                            match commands::set_tun(app2.clone(), settings_state, false).await {
                                Ok(_) => tray_notify(&app2, "已切换到系统代理模式"),
                                Err(e) => tray_notify(&app2, format!("切换系统代理失败：{e}")),
                            }
                        });
                    }
                    "quit" => {
                        let _ = sysproxy::disable();
                        let _ = process::stop_core(app);
                        app.exit(0);
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
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_handle = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_handle.hide();
                    }
                });
            }

            // Auto-start core with current config, then restore system proxy if needed.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let settings_state = app_handle.state::<SettingsState>();
                let mut settings = match settings_state.0.lock() {
                    Ok(guard) => guard.clone(),
                    Err(_) => return,
                };
                let _ = process::prepare_ports(&mut settings, true);
                if let Ok(mut guard) = settings_state.0.lock() {
                    guard.mixed_port = settings.mixed_port;
                    guard.external_controller = settings.external_controller.clone();
                    let _ = guard.save();
                    settings = guard.clone();
                }
                let _ = process::start_core(&app_handle, &settings);
                let _ = process::wait_until_ready(&settings, 40).await;
                let _ = sysproxy::set_enabled(settings.system_proxy_active(), settings.mixed_port);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::save_settings,
            commands::start_core,
            commands::stop_core,
            commands::restart_core,
            commands::set_tun,
            commands::set_proxy_enabled,
            commands::set_proxy_mode,
            commands::add_airport_url,
            commands::add_airport_file,
            commands::update_airport,
            commands::remove_airport,
            commands::set_airport_enabled,
            commands::rename_airport,
            commands::enable_company_from_conf,
            commands::set_company_enabled,
            commands::clear_company_wg,
            commands::get_company_profile,
            commands::get_proxy_group,
            commands::list_airport_nodes,
            commands::get_rules,
            commands::get_policy_groups,
            commands::set_policy_group,
            commands::add_custom_rule,
            commands::set_custom_rule_enabled,
            commands::remove_custom_rule,
            commands::select_proxy_node,
            commands::get_connections,
            commands::close_connections,
            commands::read_logs,
            commands::open_data_dir,
            commands::read_text_file,
            commands::quit_app,
            commands::test_proxy_delay,
            commands::test_proxy_delays,
            commands::get_outbound_ip,
            commands::get_autostart,
            commands::set_autostart,
            commands::clear_logs,
        ])
        .build(tauri::generate_context!())
        .expect("error while building AClash")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let _ = process::stop_core(app);
            }
        });
}
