pub mod clock;
mod commands;
pub mod power;
pub mod service;
pub mod storage;
pub mod timer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(service::AppTimerService::new(clock::SystemClock))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_timer_snapshot,
            commands::apply_timer_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
