pub mod clock;
mod commands;
pub mod power;
pub mod service;
pub mod storage;
pub mod timer;

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_timer_snapshot,
            commands::apply_timer_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
