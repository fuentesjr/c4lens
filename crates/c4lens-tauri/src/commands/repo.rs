use std::path::PathBuf;

use c4lens_core::{
    load_effective_model_from_repo_recovering_generated_overlay, repo_handle_from_path,
    CommandError, EffectiveModel, RepoHandle,
};
use rfd::FileDialog;
use tauri::{command, Emitter, State, Window};

use crate::app_state::AppState;
use crate::events::MODEL_CHANGED;

#[command]
pub fn open_repo(
    path: Option<String>,
    window: Window,
    state: State<AppState>,
) -> Result<RepoHandle, CommandError> {
    let chosen_path = path.map(PathBuf::from).or_else(pick_folder_via_dialog);
    let chosen_path = chosen_path.ok_or_else(|| {
        CommandError::new("repo.path_missing", "No repository path was provided.")
    })?;

    let repo = repo_handle_from_path(&chosen_path)?;
    {
        let mut guard = state
            .active_repo
            .lock()
            .map_err(|_| CommandError::new("fs.write_failed", "Failed to update app state."))?;
        *guard = Some(repo.clone());
    }

    if let Ok(model) = load_effective_model_from_repo_recovering_generated_overlay(repo.clone()) {
        let _ = window.emit(
            MODEL_CHANGED,
            serde_json::json!({
                "repoId": model.repo.id,
                "sourceSha": model.source_sha,
                "validation": model.validation,
            }),
        );
    }

    Ok(repo)
}

#[command]
pub fn get_model(state: State<AppState>) -> Result<EffectiveModel, CommandError> {
    let repo = {
        let guard = state
            .active_repo
            .lock()
            .map_err(|_| CommandError::new("repo.path_denied", "App state unavailable."))?;
        guard
            .clone()
            .ok_or_else(|| CommandError::new("repo.not_open", "No repository is open."))?
    };

    load_effective_model_from_repo_recovering_generated_overlay(repo)
}

fn pick_folder_via_dialog() -> Option<PathBuf> {
    let mut dialog = FileDialog::new();
    if let Ok(default_directory) = std::env::current_dir() {
        dialog = dialog.set_directory(default_directory);
    }
    dialog.pick_folder()
}
