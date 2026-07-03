mod app_state;
mod commands;
mod events;
mod generation_candidate_store;
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
            commands::repo::get_element_code,
            commands::repo::search,
            commands::repo::generate_model,
            commands::repo::apply_generated,
            commands::repo::open_in_editor,
            commands::repo::export_view,
        ])
        .run(tauri::generate_context!())
        .expect("error while running c4lens tauri app");
}
