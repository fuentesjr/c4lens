mod app_state;
mod commands;
mod events;
mod model_watcher;

use app_state::AppState;
use tauri::Builder;

use c4lens_core::CommandError;

#[tauri::command]
fn ping() -> Result<String, CommandError> {
    Ok("c4lens-tauri phase0".to_string())
}

fn main() {
    Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            ping,
            commands::repo::open_repo,
            commands::repo::get_model,
            commands::repo::scan_codebase,
        ])
        .run(tauri::generate_context!())
        .expect("error while running c4lens tauri app");
}
