use std::path::PathBuf;
use std::sync::Mutex;

use c4lens_core::{
    load_effective_model_from_repo_recovering_generated_overlay, repo_handle_from_path, scan_repo,
    CommandError, EffectiveModel, RepoHandle, ScanOptions, ScanSummary,
};
use rfd::FileDialog;
use serde::Deserialize;
use tauri::{command, Emitter, State, Window};

use crate::app_state::AppState;
use crate::events::{IndexUpdatedPayload, ModelChangedPayload, INDEX_UPDATED, MODEL_CHANGED};
use crate::model_watcher::spawn_model_watcher;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCodebaseParams {
    #[serde(default)]
    pub force: bool,
}

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
            ModelChangedPayload {
                repo_id: model.repo.id,
                source_sha: model.source_sha,
                validation: model.validation,
            },
        );
    }

    {
        let mut guard = state
            .model_watcher
            .lock()
            .map_err(|_| CommandError::new("fs.write_failed", "Failed to update app state."))?;
        *guard = Some(spawn_model_watcher(repo.clone(), window));
    }

    Ok(repo)
}

#[command]
pub fn get_model(state: State<AppState>) -> Result<EffectiveModel, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;

    load_effective_model_from_repo_recovering_generated_overlay(repo)
}

#[command]
pub fn scan_codebase(
    params: Option<ScanCodebaseParams>,
    window: Window,
    state: State<AppState>,
) -> Result<ScanSummary, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    let force = params.unwrap_or_default().force;
    let summary = scan_codebase_for_repo(repo, force, None)?;

    let _ = window.emit(
        INDEX_UPDATED,
        IndexUpdatedPayload {
            repo_id: summary.repo.id.clone(),
            summary: summary.clone(),
        },
    );

    Ok(summary)
}

fn active_repo_from_mutex(
    active_repo: &Mutex<Option<RepoHandle>>,
) -> Result<RepoHandle, CommandError> {
    let guard = active_repo
        .lock()
        .map_err(|_| CommandError::new("repo.path_denied", "App state unavailable."))?;
    guard
        .clone()
        .ok_or_else(|| CommandError::new("repo.not_open", "No repository is open."))
}

fn scan_codebase_for_repo(
    repo: RepoHandle,
    force: bool,
    index_path: Option<PathBuf>,
) -> Result<ScanSummary, CommandError> {
    scan_repo(repo, ScanOptions { force, index_path })
}

fn pick_folder_via_dialog() -> Option<PathBuf> {
    let mut dialog = FileDialog::new();
    if let Ok(default_directory) = std::env::current_dir() {
        dialog = dialog.set_directory(default_directory);
    }
    dialog.pick_folder()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use c4lens_core::repo_handle_from_path;

    use crate::app_state::AppState;

    use super::{active_repo_from_mutex, scan_codebase_for_repo};

    #[test]
    fn active_repo_returns_repo_not_open_when_no_repo_is_active() {
        let state = AppState::default();

        let error = active_repo_from_mutex(&state.active_repo).expect_err("repo should be missing");

        assert_eq!(error.code, "repo.not_open");
    }

    #[test]
    fn scan_codebase_for_repo_indexes_active_repo_files() {
        let root = fresh_test_dir("scan-command-repo");
        let index_root = fresh_test_dir("scan-command-index");
        write_file(&root, "c4/model.yml", "name: Scan Repo\n");
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let summary =
            scan_codebase_for_repo(repo.clone(), false, Some(index_root.join("index.sqlite3")))
                .expect("scan active repo");

        assert_eq!(summary.repo.id, repo.id);
        assert_eq!(summary.scanned_files, 2);
        assert_eq!(summary.changed_files, 2);
        assert_eq!(summary.deleted_files, 0);

        cleanup(index_root);
        cleanup(root);
    }

    fn fresh_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("c4lens-tauri-{name}-{unique}"));
        fs::create_dir_all(&root).expect("create test root");
        root
    }

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn cleanup(root: PathBuf) {
        fs::remove_dir_all(root).ok();
    }
}
