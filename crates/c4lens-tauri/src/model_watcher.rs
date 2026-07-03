use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, UNIX_EPOCH};

use c4lens_core::{
    load_effective_model_from_repo_recovering_generated_overlay, scan_repo, CommandError,
    RepoHandle, ScanOptions, ScanSummary, ValidationIssue, ValidationReport, ValidationSeverity,
    ValidationStage,
};
use tauri::{Emitter, Window};

use crate::events::{
    IndexUpdatedPayload, ModelChangedPayload, ScanProgressPayload, ValidationFailedPayload,
    INDEX_UPDATED, MODEL_CHANGED, SCAN_PROGRESS, VALIDATION_FAILED,
};

const AUTHORED_MODEL_PATH: &str = "c4/model.yml";
const GENERATED_MODEL_PATH: &str = "c4/model.generated.yml";
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const MODEL_DEBOUNCE_INTERVAL: Duration = Duration::from_millis(150);
const SOURCE_DEBOUNCE_INTERVAL: Duration = Duration::from_millis(500);

pub struct ModelWatcherHandle {
    stop: Sender<()>,
    join: Option<JoinHandle<()>>,
}

impl Drop for ModelWatcherHandle {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn spawn_model_watcher(repo: RepoHandle, window: Window) -> ModelWatcherHandle {
    let (stop_tx, stop_rx) = mpsc::channel();
    let join = thread::spawn(move || watch_model_files(repo, window, stop_rx));

    ModelWatcherHandle {
        stop: stop_tx,
        join: Some(join),
    }
}

fn watch_model_files(repo: RepoHandle, window: Window, stop_rx: Receiver<()>) {
    let mut previous_control = snapshot_control_files(&repo);
    let mut previous_source = snapshot_source_files(&repo);

    loop {
        match stop_rx.recv_timeout(POLL_INTERVAL) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        let next_control = snapshot_control_files(&repo);
        let next_source = snapshot_source_files(&repo);
        let control_changed = next_control != previous_control;
        let source_changed = next_source != previous_source;
        if !control_changed && !source_changed {
            continue;
        }

        let debounce = if source_changed {
            SOURCE_DEBOUNCE_INTERVAL
        } else {
            MODEL_DEBOUNCE_INTERVAL
        };
        match stop_rx.recv_timeout(debounce) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        let debounced_control = snapshot_control_files(&repo);
        let debounced_source = snapshot_source_files(&repo);
        if debounced_control != previous_control {
            previous_control = debounced_control;
            emit_model_watch_evaluation(&window, evaluate_model_change(&repo));
        }
        if debounced_source != previous_source {
            previous_source = debounced_source;
            emit_source_watch_evaluation(&window, &repo);
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ModelWatchEvaluation {
    ModelChanged(ModelChangedPayload),
    ValidationFailed(ValidationFailedPayload),
}

pub(crate) fn evaluate_model_change(repo: &RepoHandle) -> ModelWatchEvaluation {
    match load_effective_model_from_repo_recovering_generated_overlay(repo.clone()) {
        Ok(model) => ModelWatchEvaluation::ModelChanged(ModelChangedPayload {
            repo_id: model.repo.id,
            source_sha: model.source_sha,
            validation: model.validation,
        }),
        Err(error) => ModelWatchEvaluation::ValidationFailed(ValidationFailedPayload {
            repo_id: repo.id.clone(),
            validation: validation_report_from_error(error),
        }),
    }
}

fn emit_model_watch_evaluation(window: &Window, evaluation: ModelWatchEvaluation) {
    match evaluation {
        ModelWatchEvaluation::ModelChanged(payload) => {
            let _ = window.emit(MODEL_CHANGED, payload);
        }
        ModelWatchEvaluation::ValidationFailed(payload) => {
            let _ = window.emit(VALIDATION_FAILED, payload);
        }
    }
}

pub(crate) fn evaluate_source_change(repo: &RepoHandle) -> Result<ScanSummary, CommandError> {
    scan_repo(repo.clone(), ScanOptions::default())
}

fn emit_source_watch_evaluation(window: &Window, repo: &RepoHandle) {
    let _ = window.emit(
        SCAN_PROGRESS,
        ScanProgressPayload {
            repo_id: repo.id.clone(),
            done: 0,
            total: 1,
            message: "Re-indexing changed sources".to_string(),
        },
    );

    if let Ok(summary) = evaluate_source_change(repo) {
        let _ = window.emit(
            INDEX_UPDATED,
            IndexUpdatedPayload {
                repo_id: summary.repo.id.clone(),
                summary,
            },
        );
        let _ = window.emit(
            SCAN_PROGRESS,
            ScanProgressPayload {
                repo_id: repo.id.clone(),
                done: 1,
                total: 1,
                message: "Source index updated".to_string(),
            },
        );
    }
}

fn validation_report_from_error(error: CommandError) -> ValidationReport {
    let path = validation_issue_path(&error);
    let issue = ValidationIssue {
        severity: ValidationSeverity::Error,
        stage: validation_stage_for_error(&error.code),
        code: error.code,
        message: error.message,
        path,
        line: None,
        column: None,
    };

    ValidationReport {
        ok: false,
        source_sha: None,
        issues: vec![issue],
    }
}

fn validation_stage_for_error(code: &str) -> ValidationStage {
    if code.starts_with("schema.") {
        ValidationStage::Schema
    } else if code.starts_with("scan.") {
        ValidationStage::Scan
    } else if code.starts_with("semantic.") {
        ValidationStage::Semantic
    } else {
        ValidationStage::Parse
    }
}

fn validation_issue_path(error: &CommandError) -> Option<String> {
    if let Some(path) = error
        .details
        .as_ref()
        .and_then(|details| details.get("path"))
        .and_then(|path| path.as_str())
    {
        return Some(path.to_string());
    }

    if error.message.contains(AUTHORED_MODEL_PATH) {
        return Some(AUTHORED_MODEL_PATH.to_string());
    }
    if error.message.contains(GENERATED_MODEL_PATH) {
        return Some(GENERATED_MODEL_PATH.to_string());
    }

    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlFilesSnapshot {
    authored: ControlFileSnapshot,
    generated: ControlFileSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ControlFileSnapshot {
    Missing,
    Present {
        len: u64,
        modified_nanos: u128,
        is_dir: bool,
        is_symlink: bool,
    },
    Inaccessible(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceFilesSnapshot {
    files: std::collections::BTreeMap<String, SourceFileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceFileSnapshot {
    Present {
        len: u64,
        modified_nanos: u128,
        is_symlink: bool,
    },
    Inaccessible(String),
}

fn snapshot_control_files(repo: &RepoHandle) -> ControlFilesSnapshot {
    let root = PathBuf::from(&repo.root_path);
    ControlFilesSnapshot {
        authored: snapshot_control_file(&root.join(AUTHORED_MODEL_PATH)),
        generated: snapshot_control_file(&root.join(GENERATED_MODEL_PATH)),
    }
}

fn snapshot_source_files(repo: &RepoHandle) -> SourceFilesSnapshot {
    let root = PathBuf::from(&repo.root_path);
    let mut files = std::collections::BTreeMap::new();
    snapshot_source_dir(&root, &root, &mut files);
    SourceFilesSnapshot { files }
}

fn snapshot_source_dir(
    root: &Path,
    current: &Path,
    files: &mut std::collections::BTreeMap<String, SourceFileSnapshot>,
) {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(error) => {
            if let Some(path) = relative_posix_path(root, current) {
                files.insert(path, SourceFileSnapshot::Inaccessible(error.to_string()));
            }
            return;
        }
    };

    let mut entries = entries.collect::<Result<Vec<_>, _>>().unwrap_or_default();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                if let Some(relative_path) = relative_posix_path(root, &path) {
                    files.insert(
                        relative_path,
                        SourceFileSnapshot::Inaccessible(error.to_string()),
                    );
                }
                continue;
            }
        };

        if metadata.is_dir() {
            if should_snapshot_source_dir(&path) {
                snapshot_source_dir(root, &path, files);
            }
            continue;
        }

        let Some(relative_path) = relative_posix_path(root, &path) else {
            continue;
        };
        files.insert(
            relative_path,
            SourceFileSnapshot::Present {
                len: metadata.len(),
                modified_nanos: modified_nanos(&metadata),
                is_symlink: metadata.file_type().is_symlink(),
            },
        );
    }
}

fn should_snapshot_source_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    !matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | "tmp"
            | "log"
            | "coverage"
            | "c4"
    )
}

fn snapshot_control_file(path: &Path) -> ControlFileSnapshot {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ControlFileSnapshot::Missing;
        }
        Err(error) => return ControlFileSnapshot::Inaccessible(error.to_string()),
    };

    ControlFileSnapshot::Present {
        len: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
        is_dir: metadata.is_dir(),
        is_symlink: metadata.file_type().is_symlink(),
    }
}

fn modified_nanos(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn relative_posix_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parts = relative
        .components()
        .map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            std::path::Component::CurDir => Some(String::new()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some(
        parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use c4lens_core::{repo_handle_from_path, ValidationStage};

    use super::{
        evaluate_model_change, snapshot_source_files, ModelWatchEvaluation, ModelWatcherHandle,
    };

    #[test]
    fn evaluates_valid_model_change_as_model_changed() {
        let root = fresh_test_dir("valid-model-change");
        write_model(&root, "name: Watched Model\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let evaluation = evaluate_model_change(&repo);

        match evaluation {
            ModelWatchEvaluation::ModelChanged(payload) => {
                assert_eq!(payload.repo_id, repo.id);
                assert!(!payload.source_sha.is_empty());
                assert!(payload.validation.ok);
            }
            ModelWatchEvaluation::ValidationFailed(_) => {
                panic!("valid model should emit model-changed");
            }
        }

        cleanup(root);
    }

    #[test]
    fn evaluates_invalid_model_change_as_validation_failed() {
        let root = fresh_test_dir("invalid-model-change");
        write_model(&root, "name: [unterminated\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let evaluation = evaluate_model_change(&repo);

        match evaluation {
            ModelWatchEvaluation::ModelChanged(_) => {
                panic!("invalid model should emit validation-failed");
            }
            ModelWatchEvaluation::ValidationFailed(payload) => {
                assert_eq!(payload.repo_id, repo.id);
                assert!(!payload.validation.ok);
                assert_eq!(payload.validation.issues.len(), 1);
                assert_eq!(payload.validation.issues[0].code, "parse.invalid_yaml");
                assert_eq!(payload.validation.issues[0].stage, ValidationStage::Parse);
                assert_eq!(
                    payload.validation.issues[0].path.as_deref(),
                    Some("c4/model.yml")
                );
            }
        }

        cleanup(root);
    }

    #[test]
    fn source_snapshot_ignores_control_files_and_detects_source_edits() {
        let root = fresh_test_dir("source-snapshot");
        write_model(&root, "name: Watched Model\n");
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let initial = snapshot_source_files(&repo);

        write_model(&root, "name: Renamed Model\n");
        assert_eq!(snapshot_source_files(&repo), initial);

        write_file(
            &root,
            "src/main.rs",
            "fn main() {\n    println!(\"changed\");\n}\n",
        );
        assert_ne!(snapshot_source_files(&repo), initial);

        cleanup(root);
    }

    #[test]
    fn watcher_handle_drop_signals_thread_to_stop() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (stopped_tx, stopped_rx) = mpsc::channel();
        let handle = ModelWatcherHandle {
            stop: stop_tx,
            join: Some(thread::spawn(move || {
                let _ = stop_rx.recv();
                let _ = stopped_tx.send(());
            })),
        };

        drop(handle);

        stopped_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("watcher thread should stop");
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

    fn write_model(root: &Path, contents: &str) {
        fs::create_dir_all(root.join("c4")).expect("create c4 dir");
        fs::write(root.join("c4/model.yml"), contents).expect("write model");
    }

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write file");
    }

    fn cleanup(root: PathBuf) {
        fs::remove_dir_all(root).ok();
    }
}
