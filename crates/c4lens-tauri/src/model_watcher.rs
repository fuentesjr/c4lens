use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, UNIX_EPOCH};

use c4lens_core::{
    load_effective_model_from_repo_recovering_generated_overlay, CommandError, RepoHandle,
    ValidationIssue, ValidationReport, ValidationSeverity, ValidationStage,
};
use tauri::{Emitter, Window};

use crate::events::{
    ModelChangedPayload, ValidationFailedPayload, MODEL_CHANGED, VALIDATION_FAILED,
};

const AUTHORED_MODEL_PATH: &str = "c4/model.yml";
const GENERATED_MODEL_PATH: &str = "c4/model.generated.yml";
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(150);

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
    let mut previous = snapshot_control_files(&repo);

    loop {
        match stop_rx.recv_timeout(POLL_INTERVAL) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        let next = snapshot_control_files(&repo);
        if next == previous {
            continue;
        }

        match stop_rx.recv_timeout(DEBOUNCE_INTERVAL) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        previous = snapshot_control_files(&repo);
        emit_model_watch_evaluation(&window, evaluate_model_change(&repo));
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

fn snapshot_control_files(repo: &RepoHandle) -> ControlFilesSnapshot {
    let root = PathBuf::from(&repo.root_path);
    ControlFilesSnapshot {
        authored: snapshot_control_file(&root.join(AUTHORED_MODEL_PATH)),
        generated: snapshot_control_file(&root.join(GENERATED_MODEL_PATH)),
    }
}

fn snapshot_control_file(path: &Path) -> ControlFileSnapshot {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ControlFileSnapshot::Missing;
        }
        Err(error) => return ControlFileSnapshot::Inaccessible(error.to_string()),
    };

    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    ControlFileSnapshot::Present {
        len: metadata.len(),
        modified_nanos,
        is_dir: metadata.is_dir(),
        is_symlink: metadata.file_type().is_symlink(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use c4lens_core::{repo_handle_from_path, ValidationStage};

    use super::{evaluate_model_change, ModelWatchEvaluation, ModelWatcherHandle};

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

    fn write_model(root: &PathBuf, contents: &str) {
        fs::create_dir_all(root.join("c4")).expect("create c4 dir");
        fs::write(root.join("c4/model.yml"), contents).expect("write model");
    }

    fn cleanup(root: PathBuf) {
        fs::remove_dir_all(root).ok();
    }
}
