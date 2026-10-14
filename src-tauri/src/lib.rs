pub mod commands;
pub mod surge_client;

use commands::ConnectionManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ConnectionManager::default())
        .invoke_handler(tauri::generate_handler![
            commands::connect_surge,
            commands::list_downloads,
            commands::disconnect_surge,
            commands::control_download,
            commands::add_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
