use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use c4lens_core::{
    default_index_path, get_element_code as core_get_element_code,
    load_effective_model_from_repo_recovering_generated_overlay, repo_handle_from_path, scan_repo,
    CodeRef, CommandError, EffectiveModel, RepoHandle, ScanOptions, ScanSummary,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetElementCodeParams {
    pub slug: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInEditorParams {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
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

#[command]
pub fn get_element_code(
    params: GetElementCodeParams,
    state: State<AppState>,
) -> Result<Option<CodeRef>, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    get_element_code_for_repo(&repo, &params.slug, None)
}

#[command]
pub fn open_in_editor(
    params: OpenInEditorParams,
    state: State<AppState>,
) -> Result<(), CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    open_in_editor_with_opener(
        &repo,
        &params.path,
        params.line,
        params.column,
        default_open_in_editor,
    )
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

fn get_element_code_for_repo(
    repo: &RepoHandle,
    slug: &str,
    index_path: Option<PathBuf>,
) -> Result<Option<CodeRef>, CommandError> {
    let index_path = index_path.unwrap_or_else(|| default_index_path(repo));
    if !index_path.is_file() {
        return Ok(None);
    }
    core_get_element_code(repo, &index_path, slug)
}

fn scan_codebase_for_repo(
    repo: RepoHandle,
    force: bool,
    index_path: Option<PathBuf>,
) -> Result<ScanSummary, CommandError> {
    scan_repo(repo, ScanOptions { force, index_path })
}

fn open_in_editor_with_opener<F>(
    repo: &RepoHandle,
    path: &str,
    line: Option<u32>,
    column: Option<u32>,
    open_file: F,
) -> Result<(), CommandError>
where
    F: FnOnce(&Path, Option<u32>, Option<u32>) -> Result<(), CommandError>,
{
    let target = resolve_repo_relative_path(&repo.root_path, path)?;
    open_file(&target, line, column)
}

fn resolve_repo_relative_path(
    repo_root: &str,
    requested_path: &str,
) -> Result<PathBuf, CommandError> {
    let requested = requested_path.trim();
    if requested.is_empty() {
        return Err(CommandError::new(
            "path.missing",
            "No path was provided for open_in_editor.",
        ));
    }
    let requested_path = Path::new(requested);
    if requested_path.is_absolute() {
        return Err(CommandError::new(
            "path.must_be_relative",
            "The path must be repository-relative.",
        ));
    }
    if requested_path.components().any(|component| {
        matches!(
            component,
            Component::CurDir | Component::ParentDir | Component::Prefix(_)
        )
    }) {
        return Err(CommandError::new(
            "path.invalid",
            "The path must be a clean repository-relative POSIX path.",
        ));
    }
    if requested.contains('\\')
        || requested.contains('\0')
        || requested.split('/').any(|part| part.is_empty())
    {
        return Err(CommandError::new(
            "path.invalid",
            "The path must be a clean repository-relative POSIX path.",
        ));
    }

    let requested = Path::new(requested);
    let repo_root = Path::new(repo_root).canonicalize().map_err(|error| {
        CommandError::new(
            "repo.invalid",
            format!("Failed to resolve repository path: {error}"),
        )
    })?;
    let candidate = repo_root.join(requested);
    let resolved = candidate.canonicalize().map_err(|error| {
        CommandError::new(
            "path.not_found",
            format!("Source file could not be resolved: {error}"),
        )
    })?;

    if !resolved.starts_with(&repo_root) {
        return Err(CommandError::new(
            "path.out_of_repo",
            "The requested source path is outside the repository.",
        ));
    }

    if !resolved.is_file() {
        return Err(CommandError::new(
            "path.invalid_target",
            "The requested source path is not a file.",
        ));
    }

    Ok(resolved)
}

fn default_open_in_editor(
    path: &Path,
    _line: Option<u32>,
    _column: Option<u32>,
) -> Result<(), CommandError> {
    let status = open_command_for_os(std::env::consts::OS, path)
        .status()
        .map_err(|error| {
            CommandError::new(
                "path.open_failed",
                format!("Unable to start editor process: {error}"),
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(CommandError::new(
            "path.open_failed",
            "The OS editor command failed to launch.",
        ))
    }
}

fn open_command_for_os(os: &str, path: &Path) -> Command {
    let program = editor_open_program_for_os(os);
    let mut command = Command::new(program);
    command.arg(path);
    command
}

fn editor_open_program_for_os(os: &str) -> &'static OsStr {
    match os {
        "macos" => OsStr::new("open"),
        "windows" => OsStr::new("explorer.exe"),
        _ => OsStr::new("xdg-open"),
    }
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
    use std::path::MAIN_SEPARATOR;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use c4lens_core::repo_handle_from_path;

    use crate::app_state::AppState;

    use super::{
        active_repo_from_mutex, get_element_code_for_repo, open_command_for_os,
        open_in_editor_with_opener, resolve_repo_relative_path, scan_codebase_for_repo,
    };

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

    #[test]
    fn get_element_code_for_repo_reads_scanned_code_ref() {
        let root = fresh_test_dir("element-code-command-repo");
        let index_root = fresh_test_dir("element-code-command-index");
        let index_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Source Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_codebase_for_repo(repo.clone(), false, Some(index_path.clone())).expect("scan repo");

        let code_ref = get_element_code_for_repo(&repo, "app", Some(index_path))
            .expect("resolve code")
            .expect("code ref");

        assert_eq!(code_ref.element_slug, "app");
        assert_eq!(code_ref.path, "src/main.rs");
        assert_eq!(code_ref.language.as_deref(), Some("rust"));
        assert_eq!(code_ref.snippet.as_deref(), Some("fn main() {}\n"));

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn get_element_code_for_repo_returns_none_when_index_has_not_been_created() {
        let root = fresh_test_dir("element-code-missing-index-repo");
        let index_root = fresh_test_dir("element-code-missing-index-root");
        cleanup(index_root.clone());
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Source Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let code_ref =
            get_element_code_for_repo(&repo, "app", Some(index_root.join("index.sqlite3")))
                .expect("missing index should be a cache miss");

        assert!(code_ref.is_none());

        cleanup(root);
    }

    #[test]
    fn open_in_editor_rejects_absolute_paths() {
        let root = fresh_test_dir("open-editor-absolute-repo");
        write_file(&root, "src/main.rs", "fn main() {}\n");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let was_called = AtomicBool::new(false);

        let error = open_in_editor_with_opener(
            &repo,
            "/tmp/should-not-open.rs",
            None,
            None,
            |_path, _line, _column| {
                was_called.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .expect_err("absolute path should be rejected");

        assert_eq!(error.code, "path.must_be_relative");
        assert!(!was_called.load(Ordering::SeqCst));

        cleanup(root);
    }

    #[test]
    fn open_in_editor_rejects_invalid_path_syntax() {
        let root = fresh_test_dir("open-editor-invalid-syntax-repo");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let was_called = AtomicBool::new(false);

        for invalid_path in ["src\\main.rs", "src//main.rs", "src/\0main.rs"] {
            let error = open_in_editor_with_opener(
                &repo,
                invalid_path,
                None,
                None,
                |_path, _line, _column| {
                    was_called.store(true, Ordering::SeqCst);
                    Ok(())
                },
            )
            .expect_err("invalid syntax should be rejected");

            assert_eq!(error.code, "path.invalid");
        }
        assert!(!was_called.load(Ordering::SeqCst));

        cleanup(root);
    }

    #[test]
    fn open_in_editor_rejects_parent_traversal() {
        let root = fresh_test_dir("open-editor-boundary-repo");
        let outside_root = root
            .parent()
            .expect("temp root parent")
            .join(format!("c4lens-open-editor-outside-{}", random_suffix()));
        write_file(&outside_root, "main.rs", "fn outside() {}\n");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let was_called = AtomicBool::new(false);
        let escape_path = format!(
            "..{MAIN_SEPARATOR}{}{MAIN_SEPARATOR}main.rs",
            outside_root
                .file_name()
                .expect("outside directory name")
                .to_string_lossy()
        );

        let error =
            open_in_editor_with_opener(&repo, &escape_path, None, None, |_path, _line, _column| {
                was_called.store(true, Ordering::SeqCst);
                Ok(())
            })
            .expect_err("parent traversal should be rejected");

        assert_eq!(error.code, "path.invalid");
        assert!(!was_called.load(Ordering::SeqCst));

        cleanup(outside_root);
        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn open_in_editor_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = fresh_test_dir("open-editor-symlink-repo");
        let outside_root = root.parent().expect("temp root parent").join(format!(
            "c4lens-open-editor-symlink-outside-{}",
            random_suffix()
        ));
        write_file(&outside_root, "main.rs", "fn outside() {}\n");
        fs::create_dir_all(root.join("src")).expect("create src");
        symlink(outside_root.join("main.rs"), root.join("src/link.rs")).expect("create symlink");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let was_called = AtomicBool::new(false);

        let error = open_in_editor_with_opener(
            &repo,
            "src/link.rs",
            None,
            None,
            |_path, _line, _column| {
                was_called.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .expect_err("symlink outside repo should be rejected");

        assert_eq!(error.code, "path.out_of_repo");
        assert!(!was_called.load(Ordering::SeqCst));

        cleanup(outside_root);
        cleanup(root);
    }

    #[test]
    fn open_in_editor_uses_injected_opener_without_launching_editor() {
        let root = fresh_test_dir("open-editor-mock-repo");
        write_file(&root, "src/main & file.rs", "fn main() {}\n");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let source_path = PathBuf::from(&repo.root_path).join("src/main & file.rs");
        let mut observed_path = None;
        let mut observed_line = None;
        let mut observed_column = None;

        let error = open_in_editor_with_opener(
            &repo,
            "src/main & file.rs",
            Some(12),
            Some(3),
            |path, line, column| {
                observed_path = Some(path.to_path_buf());
                observed_line = Some(line);
                observed_column = Some(column);
                Ok(())
            },
        );

        assert!(error.is_ok());
        assert_eq!(observed_path, Some(source_path));
        assert_eq!(observed_line, Some(Some(12)));
        assert_eq!(observed_column, Some(Some(3)));

        cleanup(root);
    }

    #[test]
    fn open_command_for_windows_does_not_route_path_through_shell() {
        let path = PathBuf::from(r"C:\repo\src\main & worker.rs");
        let command = open_command_for_os("windows", &path);
        let args = command.get_args().collect::<Vec<_>>();

        assert_eq!(command.get_program(), std::ffi::OsStr::new("explorer.exe"));
        assert_eq!(args, vec![path.as_os_str()]);
    }

    #[test]
    fn resolve_repo_relative_path_rejects_relative_escape_without_file() {
        let root = fresh_test_dir("open-editor-missing-repo");
        let repo = repo_handle_from_path(&root).expect("repo handle");

        assert!(resolve_repo_relative_path(&repo.root_path, "../missing").is_err());

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

    fn random_suffix() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    }

    fn cleanup(root: PathBuf) {
        fs::remove_dir_all(root).ok();
    }
}
