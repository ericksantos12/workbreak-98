pub mod clock;
mod commands;
pub mod power;
pub mod service;
pub mod storage;
pub mod timer;

use std::{thread, time::Duration};

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state_path = app.path().app_local_data_dir()?.join("state.json");
            let service = service::AppTimerService::from_storage(
                clock::SystemClock,
                storage::JsonStorage::new(state_path),
            );
            let power_subscription = power::PowerSubscription::register();
            if power_subscription.is_err() {
                service.set_power_available(false)?;
            }
            app.manage(service);
            spawn_checkpoint_worker(app.handle().clone())?;
            if let Ok((subscription, events)) = power_subscription {
                spawn_power_worker(app.handle().clone(), subscription, events)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_timer_snapshot,
            commands::apply_timer_action,
            commands::resolve_close
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let service = window.state::<service::AppTimerService>();
                if service.prepare_close().is_ok() {
                    let _ = window.destroy();
                } else {
                    let _ = window.emit("close-save-failed", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn spawn_checkpoint_worker(app: tauri::AppHandle) -> std::io::Result<()> {
    thread::Builder::new()
        .name("timer-checkpoint".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_secs(1));
            let _ = app.state::<service::AppTimerService>().checkpoint();
        })?;
    Ok(())
}

fn spawn_power_worker(
    app: tauri::AppHandle,
    subscription: power::PowerSubscription,
    events: std::sync::mpsc::Receiver<power::PowerNotification>,
) -> std::io::Result<()> {
    thread::Builder::new()
        .name("power-events".into())
        .spawn(move || {
            let _subscription = subscription;
            for notification in events {
                let service = app.state::<service::AppTimerService>();
                match notification {
                    power::PowerNotification::Event(power::PowerEvent::Suspending { at_100ns }) => {
                        let _ = service.pause_for_suspend(at_100ns);
                    }
                    power::PowerNotification::Event(power::PowerEvent::Resumed) => {}
                    power::PowerNotification::ClockError(_) => {
                        let _ = service.set_power_available(false);
                    }
                }
            }
        })?;
    Ok(())
}
