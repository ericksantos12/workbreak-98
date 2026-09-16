pub mod clock;
mod commands;
pub mod power;
pub mod service;
pub mod storage;
pub mod timer;

use std::{thread, time::Duration};

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state_path = app.path().app_local_data_dir()?.join("state.json");
            app.manage(service::AppTimerService::from_storage(
                clock::SystemClock,
                storage::JsonStorage::new(state_path),
            ));
            spawn_checkpoint_worker(app.handle().clone())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_timer_snapshot,
            commands::apply_timer_action
        ])
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
