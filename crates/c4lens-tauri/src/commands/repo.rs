use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use base64::{engine::general_purpose, Engine as _};
use c4lens_core::{
    acquire_repo_write_lock, build_minimal_generated_model_from_authored_system,
    canonicalize_repo_root, default_index_path, get_element_code as core_get_element_code,
    list_element_symbols as core_list_element_symbols,
    load_effective_model_from_repo_recovering_generated_overlay, promote_generated_overlay,
    render_generated_model_yaml, repo_handle_from_path, repo_scan_token, scan_repo, search_repo,
    single_authored_internal_system_for_generation, validate_generated_overlay_paths,
    validate_generated_overlay_yaml_with_report, write_generated_overlay_to_temp_file,
    write_schema_json, write_schema_json_if_missing, CodeRef, CommandError, EffectiveModel, Model,
    RepoHandle, ScanOptions, ScanSummary, SearchResults, SymbolSearchResult, ValidationReport,
    BUNDLED_MODEL_SCHEMA_JSON, GENERATED_MODEL_PATH,
};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{command, Emitter, State, Window};

use crate::app_state::AppState;
use crate::events::{
    IndexUpdatedPayload, ModelChangedPayload, ScanProgressPayload, INDEX_UPDATED, MODEL_CHANGED,
    SCAN_PROGRESS,
};
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
pub struct GetElementSymbolsParams {
    pub slug: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateModelParams {
    #[serde(default)]
    pub scan_first: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyGeneratedSelection {
    #[serde(default)]
    pub accept_all: bool,
    #[serde(default)]
    pub accepted_change_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyGeneratedParams {
    pub generation_id: String,
    pub expected_authored_sha: Option<String>,
    pub expected_overlay_sha: Option<String>,
    pub expected_model_source_sha: String,
    pub expected_index_scan_token: String,
    pub expected_schema_version: String,
    #[serde(default)]
    pub selection: ApplyGeneratedSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationSummary {
    pub systems_added: usize,
    pub containers_added: usize,
    pub components_added: usize,
    pub relationships_added: usize,
    pub external_systems_added: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationChange {
    pub id: String,
    pub kind: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_key: Option<String>,
    pub selected_by_default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationDiff {
    pub candidate_id: String,
    pub repo: RepoHandle,
    pub overlay_path: String,
    pub base_authored_sha: Option<String>,
    pub base_overlay_sha: Option<String>,
    pub model_source_sha: String,
    pub index_scan_token: String,
    pub schema_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_yaml: Option<String>,
    pub after_yaml: String,
    pub summary: GenerationSummary,
    pub changes: Vec<GenerationChange>,
    pub validation: ValidationReport,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInEditorParams {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Svg,
    Png,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportViewScope {
    pub level: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportViewParams {
    pub format: ExportFormat,
    pub scope: ExportViewScope,
    pub svg: Option<String>,
    pub png_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportViewResult {
    pub saved_path: String,
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

    state.generation_candidates.clear()?;

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
    emit_scan_progress(&window, &repo, 0, 1, "Scanning codebase");
    let summary = scan_codebase_for_repo(repo, force, None)?;
    emit_scan_progress(&window, &summary.repo, 1, 1, "Scan complete");

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
pub fn get_element_symbols(
    params: GetElementSymbolsParams,
    state: State<AppState>,
) -> Result<Vec<SymbolSearchResult>, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    get_element_symbols_for_repo(&repo, &params.slug, params.limit, None)
}

#[command]
pub fn search(params: SearchParams, state: State<AppState>) -> Result<SearchResults, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    search_repo_for_repo(&repo, &params.query, params.limit, None)
}

#[command]
pub fn generate_model(
    params: Option<GenerateModelParams>,
    window: Window,
    state: State<AppState>,
) -> Result<GenerationDiff, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    let params = params.unwrap_or_default();
    let scan_first = params.scan_first;
    if params.scan_first {
        emit_scan_progress(&window, &repo, 0, 1, "Scanning before generation");
    }
    let diff = generate_model_for_repo(repo, params, None)?;
    if scan_first {
        emit_scan_progress(&window, &diff.repo, 1, 1, "Scan complete");
    }
    state.generation_candidates.store(diff.clone())?;
    Ok(diff)
}

#[command]
pub fn apply_generated(
    params: ApplyGeneratedParams,
    window: Window,
    state: State<AppState>,
) -> Result<(), CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    let candidate = state.generation_candidates.current()?;

    apply_generated_candidate_to_repo(&repo, &params, &candidate, None)?;

    state.generation_candidates.clear()?;

    if let Ok(model) = load_effective_model_from_repo_recovering_generated_overlay(repo) {
        let _ = window.emit(
            MODEL_CHANGED,
            ModelChangedPayload {
                repo_id: model.repo.id,
                source_sha: model.source_sha,
                validation: model.validation,
            },
        );
    }

    Ok(())
}

#[command]
pub fn repair_schema(
    window: Window,
    state: State<AppState>,
) -> Result<ValidationReport, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    let validation = repair_schema_for_repo(&repo)?;
    let model = load_effective_model_from_repo_recovering_generated_overlay(repo)?;
    let _ = window.emit(
        MODEL_CHANGED,
        ModelChangedPayload {
            repo_id: model.repo.id.clone(),
            source_sha: model.source_sha.clone(),
            validation: model.validation.clone(),
        },
    );
    Ok(validation)
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

#[command]
pub fn export_view(
    params: ExportViewParams,
    state: State<AppState>,
) -> Result<ExportViewResult, CommandError> {
    let repo = active_repo_from_mutex(&state.active_repo)?;
    export_view_for_repo_with_picker(&repo, &params, pick_export_path)
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

fn emit_scan_progress(
    window: &Window,
    repo: &RepoHandle,
    done: usize,
    total: usize,
    message: &str,
) {
    let _ = window.emit(
        SCAN_PROGRESS,
        ScanProgressPayload {
            repo_id: repo.id.clone(),
            done,
            total,
            message: message.to_string(),
        },
    );
}

fn repair_schema_for_repo(repo: &RepoHandle) -> Result<ValidationReport, CommandError> {
    write_schema_json(repo)?;
    Ok(load_effective_model_from_repo_recovering_generated_overlay(repo.clone())?.validation)
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

fn get_element_symbols_for_repo(
    repo: &RepoHandle,
    slug: &str,
    limit: Option<usize>,
    index_path: Option<PathBuf>,
) -> Result<Vec<SymbolSearchResult>, CommandError> {
    let index_path = index_path.unwrap_or_else(|| default_index_path(repo));
    if !index_path.is_file() {
        return Ok(Vec::new());
    }
    core_list_element_symbols(repo, &index_path, slug, limit.unwrap_or(12))
}

fn search_repo_for_repo(
    repo: &RepoHandle,
    query: &str,
    limit: Option<usize>,
    index_path: Option<PathBuf>,
) -> Result<SearchResults, CommandError> {
    search_repo(repo, index_path, query, limit.unwrap_or(20))
}

fn scan_codebase_for_repo(
    repo: RepoHandle,
    force: bool,
    index_path: Option<PathBuf>,
) -> Result<ScanSummary, CommandError> {
    scan_repo(repo, ScanOptions { force, index_path })
}

fn generate_model_for_repo(
    repo: RepoHandle,
    params: GenerateModelParams,
    index_path: Option<PathBuf>,
) -> Result<GenerationDiff, CommandError> {
    let mut index_scan_token = String::new();
    let scan_token_index_path = index_path.clone();
    if params.scan_first {
        index_scan_token = scan_codebase_for_repo(repo.clone(), false, index_path)?.scan_token;
    }
    if index_scan_token.is_empty() {
        index_scan_token = repo_scan_token(&repo, scan_token_index_path)?.unwrap_or_default();
    }

    let authored_internal_system = single_authored_internal_system_for_generation(&repo);
    let reused_system_slug = authored_internal_system
        .as_ref()
        .map(|system| system.slug.as_str());
    let generated = build_minimal_generated_model_from_authored_system(
        &repo,
        authored_internal_system.as_ref(),
    );

    let after_yaml = render_generated_model_yaml(&generated)?;
    let validation = validate_generated_overlay_yaml_with_report(repo.clone(), &after_yaml)?;

    let before_yaml = read_optional_repo_file(&repo, GENERATED_MODEL_PATH)?;
    let base_authored_sha = read_file_sha(&repo, "c4/model.yml")?;
    let base_overlay_sha = read_file_sha(&repo, GENERATED_MODEL_PATH)?;
    let source_parts = source_parts_for_repo(&repo)?;
    let model_source_sha = model_source_sha_for_repo(&repo, &source_parts);

    Ok(GenerationDiff {
        candidate_id: sha256_hex(
            format!(
                "{repo_id}:{scan}:{source}:{after}",
                repo_id = repo.id,
                scan = &index_scan_token,
                source = &model_source_sha,
                after = &after_yaml,
            )
            .as_bytes(),
        ),
        repo: repo.clone(),
        overlay_path: GENERATED_MODEL_PATH.to_string(),
        base_authored_sha,
        base_overlay_sha,
        model_source_sha: model_source_sha.clone(),
        index_scan_token,
        schema_version: sha256_hex(BUNDLED_MODEL_SCHEMA_JSON.as_bytes()),
        before_yaml,
        after_yaml,
        summary: generation_summary(&generated, reused_system_slug),
        changes: generation_changes(&generated, reused_system_slug),
        validation,
    })
}

fn verify_apply_generated_params(
    repo: &RepoHandle,
    params: &ApplyGeneratedParams,
    candidate: &GenerationDiff,
    index_path: Option<PathBuf>,
) -> Result<(), CommandError> {
    if candidate.candidate_id != params.generation_id {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Generation candidate id does not match request.",
        ));
    }

    if candidate.repo.id != repo.id {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Generation candidate belongs to a different repository.",
        ));
    }

    if params.expected_authored_sha != candidate.base_authored_sha {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Authored model hash changed while generation candidate was prepared.",
        ));
    }

    if params.expected_model_source_sha != candidate.model_source_sha {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Model source state changed while generation candidate was prepared.",
        ));
    }

    if params.expected_index_scan_token != candidate.index_scan_token {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Scan token changed while generation candidate was prepared.",
        ));
    }

    if params.expected_schema_version != candidate.schema_version {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Schema version changed while generation candidate was prepared.",
        ));
    }

    if params.expected_overlay_sha != candidate.base_overlay_sha {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Generation candidate overlay hash does not match current stored candidate.",
        ));
    }

    if !params.selection.accept_all || !params.selection.accepted_change_ids.is_empty() {
        return Err(CommandError::new(
            "generation.no_changes",
            "MVP apply requires acceptAll=true with no per-change selection.",
        ));
    }

    let current_authored_sha = read_file_sha(repo, "c4/model.yml")?;
    if current_authored_sha != params.expected_authored_sha {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Authored model changed while generation candidate was prepared.",
        ));
    }

    let current_overlay_sha = read_file_sha(repo, GENERATED_MODEL_PATH)?;
    if current_overlay_sha != params.expected_overlay_sha {
        return Err(CommandError::new(
            "generation.overlay_changed",
            "Generated overlay changed while generation candidate was prepared.",
        ));
    }

    let source_parts = source_parts_for_repo(repo)?;
    let current_model_source_sha = model_source_sha_for_repo(repo, &source_parts);
    if current_model_source_sha != params.expected_model_source_sha {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Model source state changed while generation candidate was prepared.",
        ));
    }

    let current_index_scan_token = repo_scan_token(repo, index_path)?.unwrap_or_default();
    if current_index_scan_token != params.expected_index_scan_token {
        return Err(CommandError::new(
            "generation.candidate_stale",
            "Index scan token changed while generation candidate was prepared.",
        ));
    }

    Ok(())
}

fn apply_generated_candidate_to_repo(
    repo: &RepoHandle,
    params: &ApplyGeneratedParams,
    candidate: &GenerationDiff,
    index_path: Option<PathBuf>,
) -> Result<(), CommandError> {
    apply_generated_candidate_to_repo_with_hook(repo, params, candidate, index_path, || Ok(()))
}

fn apply_generated_candidate_to_repo_with_hook<F>(
    repo: &RepoHandle,
    params: &ApplyGeneratedParams,
    candidate: &GenerationDiff,
    index_path: Option<PathBuf>,
    before_promote: F,
) -> Result<(), CommandError>
where
    F: FnOnce() -> Result<(), CommandError>,
{
    let _write_lock = acquire_repo_write_lock(repo)?;
    verify_apply_generated_params(repo, params, candidate, index_path.clone())?;

    let repo_root = canonicalize_repo_root(repo)?;
    let (_, generated_path) = validate_generated_overlay_paths(&repo_root)?;
    let generated_dir = generated_path.parent().ok_or_else(|| {
        CommandError::new("generation.failed", "Generated model path is invalid.")
    })?;
    let temp_overlay_path =
        write_generated_overlay_to_temp_file(generated_dir, &candidate.after_yaml)?;

    match validate_generated_overlay_yaml_with_report(repo.clone(), &candidate.after_yaml) {
        Ok(report) if report.ok => {}
        Ok(report) => {
            let _ = fs::remove_file(&temp_overlay_path);
            return Err(CommandError::with_details(
                "generation.invalid",
                "Generated model is invalid.",
                serde_json::json!({ "validation": report }),
            ));
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_overlay_path);
            return Err(error);
        }
    }

    if let Err(error) = write_schema_json_if_missing(generated_dir) {
        let _ = fs::remove_file(&temp_overlay_path);
        return Err(error);
    }

    if let Err(error) = before_promote() {
        let _ = fs::remove_file(&temp_overlay_path);
        return Err(error);
    }

    if let Err(error) = verify_apply_generated_params(repo, params, candidate, index_path) {
        let _ = fs::remove_file(&temp_overlay_path);
        return Err(error);
    }

    if let Err(error) = promote_generated_overlay(&temp_overlay_path, &generated_path) {
        let _ = fs::remove_file(&temp_overlay_path);
        return Err(error);
    }

    Ok(())
}

fn source_parts_for_repo(repo: &RepoHandle) -> Result<Vec<(String, String)>, CommandError> {
    let mut parts = Vec::new();

    if let Some(contents) = read_optional_repo_file(repo, "c4/model.yml")? {
        parts.push(("c4/model.yml".to_string(), contents));
    }

    if let Some(contents) = read_optional_repo_file(repo, GENERATED_MODEL_PATH)? {
        parts.push((GENERATED_MODEL_PATH.to_string(), contents));
    }

    parts.push((
        "schema.json".to_string(),
        BUNDLED_MODEL_SCHEMA_JSON.to_string(),
    ));
    Ok(parts)
}

fn model_source_sha_for_repo(repo: &RepoHandle, source_parts: &[(String, String)]) -> String {
    load_effective_model_from_repo_recovering_generated_overlay(repo.clone())
        .map(|effective| effective.source_sha)
        .unwrap_or_else(|_| stable_source_sha(source_parts))
}

fn stable_source_sha(parts: &[(String, String)]) -> String {
    let mut digest = Sha256::new();
    for (path, value) in parts {
        digest.update(path.as_bytes());
        digest.update([0]);
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

fn read_file_sha(repo: &RepoHandle, relative_path: &str) -> Result<Option<String>, CommandError> {
    Ok(read_optional_repo_file(repo, relative_path)?
        .map(|contents| sha256_hex(contents.as_bytes())))
}

fn read_optional_repo_file(
    repo: &RepoHandle,
    relative_path: &str,
) -> Result<Option<String>, CommandError> {
    let repo_root = Path::new(&repo.root_path).canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.invalid",
            "Failed to resolve repository path.",
            serde_json::json!({ "error": error.to_string() }),
        )
    })?;
    let path = repo_root.join(relative_path);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            validate_control_parent_if_present(&repo_root, relative_path)?;
            return Ok(None);
        }
        Err(error) => {
            return Err(CommandError::with_details(
                "fs.read_failed",
                "Failed to inspect repository control file.",
                serde_json::json!({ "path": relative_path, "error": error.to_string() }),
            ));
        }
    };

    if metadata.file_type().is_symlink() {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Repository control file must not be a symlink.",
            serde_json::json!({ "path": relative_path }),
        ));
    }

    if !metadata.is_file() {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Repository control path is not a file.",
            serde_json::json!({ "path": relative_path }),
        ));
    }

    let canonical_path = path.canonicalize().map_err(|error| {
        CommandError::with_details(
            "fs.read_failed",
            "Failed to resolve repository control file.",
            serde_json::json!({ "path": relative_path, "error": error.to_string() }),
        )
    })?;
    if !canonical_path.starts_with(&repo_root) {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Repository control file resolves outside the repository.",
            serde_json::json!({ "path": relative_path }),
        ));
    }

    fs::read_to_string(canonical_path)
        .map(Some)
        .map_err(|error| {
            CommandError::with_details(
                "fs.read_failed",
                "Failed to read repository control file.",
                serde_json::json!({ "path": relative_path, "error": error.to_string() }),
            )
        })
}

fn validate_control_parent_if_present(
    repo_root: &Path,
    relative_path: &str,
) -> Result<(), CommandError> {
    let relative_parent = Path::new(relative_path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let Some(relative_parent) = relative_parent else {
        return Ok(());
    };

    let parent_path = repo_root.join(relative_parent);
    let metadata = match fs::symlink_metadata(&parent_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(CommandError::with_details(
                "fs.read_failed",
                "Failed to inspect repository control directory.",
                serde_json::json!({ "path": relative_parent.display().to_string(), "error": error.to_string() }),
            ));
        }
    };

    if metadata.file_type().is_symlink() {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Repository control directory must not be a symlink.",
            serde_json::json!({ "path": relative_parent.display().to_string() }),
        ));
    }

    if !metadata.is_dir() {
        return Err(CommandError::with_details(
            "path.invalid_target",
            "Repository control parent exists but is not a directory.",
            serde_json::json!({ "path": relative_parent.display().to_string() }),
        ));
    }

    let canonical_parent = parent_path.canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.path_denied",
            "Failed to resolve repository control directory.",
            serde_json::json!({ "path": relative_parent.display().to_string(), "error": error.to_string() }),
        )
    })?;
    if !canonical_parent.starts_with(repo_root) {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Repository control directory resolves outside the repository.",
            serde_json::json!({ "path": relative_parent.display().to_string() }),
        ));
    }

    Ok(())
}

fn generation_summary(model: &Model, reused_system_slug: Option<&str>) -> GenerationSummary {
    let systems_added = model
        .systems
        .values()
        .filter(|system| {
            system.base.generated && Some(system.base.slug.as_str()) != reused_system_slug
        })
        .count();
    let containers_added = model
        .systems
        .values()
        .map(|system| {
            system
                .containers
                .values()
                .filter(|container| container.base.generated)
                .count()
        })
        .sum();
    let components_added = model
        .systems
        .values()
        .flat_map(|system| system.containers.values())
        .map(|container| {
            container
                .components
                .values()
                .filter(|component| component.base.generated)
                .count()
        })
        .sum();

    GenerationSummary {
        systems_added,
        containers_added,
        components_added,
        relationships_added: model
            .relationships
            .iter()
            .filter(|relationship| relationship.generated)
            .count(),
        external_systems_added: model
            .systems
            .values()
            .filter(|system| {
                system.base.generated
                    && system.external
                    && Some(system.base.slug.as_str()) != reused_system_slug
            })
            .count(),
    }
}

fn generation_changes(model: &Model, reused_system_slug: Option<&str>) -> Vec<GenerationChange> {
    let mut changes = Vec::new();

    for system in model.systems.values() {
        if system.base.generated && Some(system.base.slug.as_str()) != reused_system_slug {
            changes.push(GenerationChange {
                id: format!("system:{}", system.base.slug),
                kind: "add".to_string(),
                target: "system".to_string(),
                slug: Some(system.base.slug.clone()),
                relationship_key: None,
                selected_by_default: true,
            });
        }

        for container in system.containers.values() {
            if container.base.generated {
                changes.push(GenerationChange {
                    id: format!("container:{}", container.base.slug),
                    kind: "add".to_string(),
                    target: "container".to_string(),
                    slug: Some(container.base.slug.clone()),
                    relationship_key: None,
                    selected_by_default: true,
                });
            }

            for component in container.components.values() {
                if component.base.generated {
                    changes.push(GenerationChange {
                        id: format!("component:{}", component.base.slug),
                        kind: "add".to_string(),
                        target: "component".to_string(),
                        slug: Some(component.base.slug.clone()),
                        relationship_key: None,
                        selected_by_default: true,
                    });
                }
            }
        }
    }

    for relationship in &model.relationships {
        if relationship.generated {
            let relationship_key = format!(
                "{}:{}:{}",
                relationship.from, relationship.to, relationship.description
            );
            changes.push(GenerationChange {
                id: format!("relationship:{relationship_key}"),
                kind: "add".to_string(),
                target: "relationship".to_string(),
                slug: None,
                relationship_key: Some(relationship_key),
                selected_by_default: true,
            });
        }
    }

    changes
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
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

fn export_view_for_repo_with_picker<F>(
    repo: &RepoHandle,
    params: &ExportViewParams,
    pick_path: F,
) -> Result<ExportViewResult, CommandError>
where
    F: FnOnce(&RepoHandle, &ExportViewParams) -> Option<PathBuf>,
{
    let bytes = export_payload_bytes(params)?;
    let path = pick_path(repo, params).ok_or_else(|| {
        CommandError::new("export.cancelled", "Export destination was not selected.")
    })?;

    fs::write(&path, bytes).map_err(|error| {
        CommandError::with_details(
            "export.failed",
            "Failed to write exported view.",
            serde_json::json!({ "path": path.display().to_string(), "error": error.to_string() }),
        )
    })?;

    Ok(ExportViewResult {
        saved_path: path.to_string_lossy().to_string(),
    })
}

fn export_payload_bytes(params: &ExportViewParams) -> Result<Vec<u8>, CommandError> {
    match params.format {
        ExportFormat::Svg => params
            .svg
            .as_deref()
            .filter(|svg| !svg.trim().is_empty())
            .map(|svg| svg.as_bytes().to_vec())
            .ok_or_else(|| CommandError::new("export.failed", "SVG export payload is missing.")),
        ExportFormat::Png => {
            let encoded = params
                .png_base64
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    CommandError::new("export.failed", "PNG export payload is missing.")
                })?;
            general_purpose::STANDARD.decode(encoded).map_err(|error| {
                CommandError::with_details(
                    "export.failed",
                    "PNG export payload is not valid base64.",
                    serde_json::json!({ "error": error.to_string() }),
                )
            })
        }
    }
}

fn pick_export_path(repo: &RepoHandle, params: &ExportViewParams) -> Option<PathBuf> {
    let extension = export_extension(&params.format);
    let title = match params.format {
        ExportFormat::Svg => "Export SVG view",
        ExportFormat::Png => "Export PNG view",
    };
    let mut dialog = FileDialog::new()
        .set_title(title)
        .set_file_name(default_export_file_name(repo, params));
    dialog = match params.format {
        ExportFormat::Svg => dialog.add_filter("SVG", &["svg"]),
        ExportFormat::Png => dialog.add_filter("PNG", &["png"]),
    };
    let selected = dialog.save_file()?;
    if selected.extension().is_some() {
        Some(selected)
    } else {
        Some(selected.with_extension(extension))
    }
}

fn default_export_file_name(repo: &RepoHandle, params: &ExportViewParams) -> String {
    let scope = params.scope.slug.as_deref().unwrap_or(&params.scope.level);
    format!(
        "{}-{}.{}",
        filename_part(&repo.name),
        filename_part(scope),
        export_extension(&params.format)
    )
}

fn filename_part(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if matches!(character, '-' | '_') {
            output.push(character);
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    let output = output.trim_matches('-');
    if output.is_empty() {
        "view".to_string()
    } else {
        output.to_string()
    }
}

fn export_extension(format: &ExportFormat) -> &'static str {
    match format {
        ExportFormat::Svg => "svg",
        ExportFormat::Png => "png",
    }
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

    use c4lens_core::{
        load_effective_model_from_repo_recovering_generated_overlay, repo_handle_from_path,
    };

    use crate::app_state::AppState;

    use super::{
        active_repo_from_mutex, apply_generated_candidate_to_repo,
        apply_generated_candidate_to_repo_with_hook, export_view_for_repo_with_picker,
        generate_model_for_repo, get_element_code_for_repo, get_element_symbols_for_repo,
        open_command_for_os, open_in_editor_with_opener, repair_schema_for_repo,
        resolve_repo_relative_path, scan_codebase_for_repo, search_repo_for_repo,
        verify_apply_generated_params, ApplyGeneratedParams, ApplyGeneratedSelection, ExportFormat,
        ExportViewParams, ExportViewScope, GenerateModelParams, GenerationDiff,
    };

    fn build_apply_params_from_diff(diff: &GenerationDiff) -> ApplyGeneratedParams {
        ApplyGeneratedParams {
            generation_id: diff.candidate_id.clone(),
            expected_authored_sha: diff.base_authored_sha.clone(),
            expected_overlay_sha: diff.base_overlay_sha.clone(),
            expected_model_source_sha: diff.model_source_sha.clone(),
            expected_index_scan_token: diff.index_scan_token.clone(),
            expected_schema_version: diff.schema_version.clone(),
            selection: ApplyGeneratedSelection {
                accept_all: true,
                accepted_change_ids: vec![],
            },
        }
    }

    fn generate_params_repo_for_test(root_suffix: &str) -> (std::path::PathBuf, GenerationDiff) {
        let root = fresh_test_dir(root_suffix);
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"invoice-api\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let diff = generate_model_for_repo(repo, GenerateModelParams { scan_first: false }, None)
            .expect("generate model candidate");

        (root, diff)
    }

    #[test]
    fn current_generation_candidate_missing_when_none_is_stored() {
        let state = AppState::default();

        let error = state
            .generation_candidates
            .current()
            .expect_err("candidate missing");

        assert_eq!(error.code, "generation.candidate_not_found");
    }

    #[test]
    fn verify_apply_generated_params_rejects_stale_candidate() {
        let (root, diff) = generate_params_repo_for_test("apply-generated-stale-candidate");
        let repo = repo_handle_from_path(&root).expect("repo handle");

        let mut params = build_apply_params_from_diff(&diff);
        params.expected_model_source_sha =
            "000000000000000000000000000000000000000000000000000000000000000000".into();

        let error = verify_apply_generated_params(&repo, &params, &diff, None)
            .expect_err("stale candidate");

        assert_eq!(error.code, "generation.candidate_stale");

        cleanup(root);
    }

    #[test]
    fn verify_apply_generated_params_rejects_changed_overlay() {
        let (root, diff) = generate_params_repo_for_test("apply-generated-overlay-changed");
        write_file(&root, "c4/model.generated.yml", "name: Existing\n");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let params = build_apply_params_from_diff(&diff);

        let error = verify_apply_generated_params(&repo, &params, &diff, None)
            .expect_err("overlay changed");

        assert_eq!(error.code, "generation.overlay_changed");

        cleanup(root);
    }

    #[test]
    fn verify_apply_generated_params_rejects_per_change_apply_request() {
        let (root, diff) = generate_params_repo_for_test("apply-generated-per-change");
        let repo = repo_handle_from_path(&root).expect("repo handle");

        let mut params = build_apply_params_from_diff(&diff);
        params.selection.accept_all = false;
        params.selection.accepted_change_ids = vec!["system:billing".into()];

        let error = verify_apply_generated_params(&repo, &params, &diff, None)
            .expect_err("accept-all-only");

        assert_eq!(error.code, "generation.no_changes");

        cleanup(root);
    }

    #[test]
    fn verify_apply_generated_params_rejects_change_ids_with_accept_all() {
        let (root, diff) = generate_params_repo_for_test("apply-generated-ids-with-accept-all");
        let repo = repo_handle_from_path(&root).expect("repo handle");

        let mut params = build_apply_params_from_diff(&diff);
        params.selection.accepted_change_ids = vec!["container:invoice_api".into()];

        let error = verify_apply_generated_params(&repo, &params, &diff, None)
            .expect_err("change ids are not supported");

        assert_eq!(error.code, "generation.no_changes");

        cleanup(root);
    }

    #[test]
    fn verify_apply_generated_params_rejects_current_authored_model_change() {
        let (root, diff) = generate_params_repo_for_test("apply-generated-authored-changed");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Authored Model
systems:
  billing:
    name: Billing
"#,
        );
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let params = build_apply_params_from_diff(&diff);

        let error = verify_apply_generated_params(&repo, &params, &diff, None)
            .expect_err("authored changed");

        assert_eq!(error.code, "generation.candidate_stale");

        cleanup(root);
    }

    #[test]
    fn verify_apply_generated_params_rejects_current_index_token_change() {
        let root = fresh_test_dir("apply-generated-index-changed");
        let index_root = fresh_test_dir("apply-generated-index-changed-root");
        let index_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"invoice-api\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let diff = generate_model_for_repo(
            repo.clone(),
            GenerateModelParams { scan_first: true },
            Some(index_path.clone()),
        )
        .expect("generate model candidate");
        write_file(
            &root,
            "src/main.rs",
            "fn main() { println!(\"changed\"); }\n",
        );
        scan_codebase_for_repo(repo.clone(), false, Some(index_path.clone()))
            .expect("rescan changed repo");
        let params = build_apply_params_from_diff(&diff);

        let error = verify_apply_generated_params(&repo, &params, &diff, Some(index_path))
            .expect_err("index changed");

        assert_eq!(error.code, "generation.candidate_stale");

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn apply_generated_candidate_to_repo_writes_overlay_and_schema() {
        let (root, diff) = generate_params_repo_for_test("apply-generated-happy-path");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let params = build_apply_params_from_diff(&diff);

        apply_generated_candidate_to_repo(&repo, &params, &diff, None).expect("apply candidate");

        assert_eq!(
            fs::read_to_string(root.join("c4/model.generated.yml")).expect("generated overlay"),
            diff.after_yaml
        );
        assert_eq!(
            fs::read_to_string(root.join("c4/schema.json")).expect("schema"),
            c4lens_core::BUNDLED_MODEL_SCHEMA_JSON
        );
        let effective = load_effective_model_from_repo_recovering_generated_overlay(repo)
            .expect("effective model");
        assert!(effective
            .model
            .systems
            .values()
            .any(|system| system.containers.contains_key("invoice_api")));

        cleanup(root);
    }

    #[test]
    fn apply_generated_candidate_to_repo_rechecks_overlay_before_promote() {
        let (root, diff) = generate_params_repo_for_test("apply-generated-late-overlay-change");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let params = build_apply_params_from_diff(&diff);

        let error =
            apply_generated_candidate_to_repo_with_hook(&repo, &params, &diff, None, || {
                write_file(&root, "c4/model.generated.yml", "name: Late Change\n");
                Ok(())
            })
            .expect_err("late overlay change should be rejected");

        assert_eq!(error.code, "generation.overlay_changed");
        assert_eq!(
            fs::read_to_string(root.join("c4/model.generated.yml")).expect("generated overlay"),
            "name: Late Change\n"
        );

        cleanup(root);
    }

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
    fn search_repo_for_repo_returns_indexed_results() {
        let root = fresh_test_dir("search-command-repo");
        let index_root = fresh_test_dir("search-command-index");
        let index_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Search Repo
systems:
  app:
    name: Search App
    code: src/search.rs
"#,
        );
        write_file(
            &root,
            "src/search.rs",
            "pub struct SearchIndex;\npub fn search_everything() {}\n",
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_codebase_for_repo(repo.clone(), false, Some(index_path.clone())).expect("scan repo");

        let results =
            search_repo_for_repo(&repo, "search", Some(20), Some(index_path)).expect("search repo");

        assert_eq!(results.query, "search");
        assert_eq!(results.elements[0].slug, "app");
        assert_eq!(results.files[0].path, "src/search.rs");
        assert_eq!(results.symbols[0].name, "SearchIndex");

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn export_view_for_repo_writes_svg_payload() {
        let root = fresh_test_dir("export-view-svg");
        let output_path = root.join("exports").join("view.svg");
        fs::create_dir_all(output_path.parent().expect("output parent")).expect("create output");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let params = ExportViewParams {
            format: ExportFormat::Svg,
            scope: ExportViewScope {
                level: "context".to_string(),
                slug: None,
            },
            svg: Some("<svg><text>c4lens</text></svg>".to_string()),
            png_base64: None,
        };

        let result =
            export_view_for_repo_with_picker(&repo, &params, |_, _| Some(output_path.clone()))
                .expect("export svg");

        assert_eq!(result.saved_path, output_path.to_string_lossy());
        assert_eq!(
            fs::read_to_string(&output_path).expect("read export"),
            "<svg><text>c4lens</text></svg>"
        );

        cleanup(root);
    }

    #[test]
    fn export_view_for_repo_decodes_png_payload() {
        let root = fresh_test_dir("export-view-png");
        let output_path = root.join("view.png");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let params = ExportViewParams {
            format: ExportFormat::Png,
            scope: ExportViewScope {
                level: "context".to_string(),
                slug: None,
            },
            svg: None,
            png_base64: Some("AQIDBA==".to_string()),
        };

        export_view_for_repo_with_picker(&repo, &params, |_, _| Some(output_path.clone()))
            .expect("export png");

        assert_eq!(fs::read(&output_path).expect("read png"), [1, 2, 3, 4]);

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
    fn get_element_symbols_for_repo_reads_symbols_under_element_code_path() {
        let root = fresh_test_dir("element-symbols-command-repo");
        let index_root = fresh_test_dir("element-symbols-command-index");
        let index_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Symbol Repo
systems:
  app:
    name: App
    containers:
      api:
        name: API
        components:
          billing:
            name: Billing
            code: src/billing
"#,
        );
        write_file(
            &root,
            "src/billing/mod.rs",
            "pub struct BillingWorker;\npub fn bill_customer() {}\n",
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_codebase_for_repo(repo.clone(), false, Some(index_path.clone())).expect("scan repo");

        let symbols = get_element_symbols_for_repo(&repo, "billing", Some(4), Some(index_path))
            .expect("list symbols");

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "BillingWorker");
        assert_eq!(symbols[0].path, "src/billing/mod.rs");
        assert_eq!(symbols[1].name, "bill_customer");

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn repair_schema_for_repo_replaces_stale_schema_and_clears_warning() {
        let root = fresh_test_dir("repair-schema-command-repo");
        write_file(&root, "c4/model.yml", "name: Repair Schema\n");
        write_file(&root, "c4/schema.json", "{\"title\":\"stale\"}\n");
        let repo = repo_handle_from_path(&root).expect("repo handle");

        let before = load_effective_model_from_repo_recovering_generated_overlay(repo.clone())
            .expect("load model");
        assert!(before
            .validation
            .issues
            .iter()
            .any(|issue| issue.code == "schema.drift"));

        let validation = repair_schema_for_repo(&repo).expect("repair schema");

        assert!(!validation
            .issues
            .iter()
            .any(|issue| issue.code == "schema.drift"));

        cleanup(root);
    }

    #[test]
    fn generate_model_for_repo_returns_candidate_without_writing_overlay() {
        let root = fresh_test_dir("generate-model-command-repo");
        let index_root = fresh_test_dir("generate-model-command-index");
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"billing-service\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let diff = generate_model_for_repo(
            repo.clone(),
            GenerateModelParams { scan_first: true },
            Some(index_root.join("index.sqlite3")),
        )
        .expect("generate model candidate");

        assert_eq!(diff.repo.id, repo.id);
        assert_eq!(diff.overlay_path, "c4/model.generated.yml");
        assert!(!diff.candidate_id.is_empty());
        assert!(!diff.index_scan_token.is_empty());
        assert_eq!(diff.base_authored_sha, None);
        assert_eq!(diff.base_overlay_sha, None);
        let serialized = serde_json::to_value(&diff).expect("serialize generation diff");
        assert!(serialized
            .as_object()
            .expect("serialized object")
            .contains_key("baseAuthoredSha"));
        assert!(serialized["baseAuthoredSha"].is_null());
        assert!(serialized
            .as_object()
            .expect("serialized object")
            .contains_key("baseOverlaySha"));
        assert!(serialized["baseOverlaySha"].is_null());
        assert_eq!(diff.model_source_sha.len(), 64);
        assert_eq!(diff.schema_version.len(), 64);
        assert!(diff.before_yaml.is_none());
        assert!(diff.after_yaml.contains("billing_service:"));
        assert_eq!(diff.summary.containers_added, 1);
        assert!(diff.validation.ok);
        assert_eq!(
            diff.validation.source_sha.as_deref().map(str::len),
            Some(64)
        );
        assert!(!root.join("c4/model.generated.yml").exists());

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn generate_model_for_repo_reports_reused_authored_system_as_existing() {
        let root = fresh_test_dir("generate-model-command-reused-system");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Authored Model
systems:
  billing:
    name: Billing Platform
"#,
        );
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"invoice-api\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let diff = generate_model_for_repo(repo, GenerateModelParams { scan_first: false }, None)
            .expect("generate model candidate");

        assert!(diff.after_yaml.contains("  billing:"));
        assert!(diff.after_yaml.contains("name: Billing Platform"));
        assert!(diff.after_yaml.contains("invoice_api:"));
        assert_eq!(diff.summary.systems_added, 0);
        assert_eq!(diff.summary.containers_added, 1);
        assert!(!diff
            .changes
            .iter()
            .any(|change| change.target == "system" && change.slug.as_deref() == Some("billing")));

        cleanup(root);
    }

    #[test]
    fn generate_model_for_repo_hashes_empty_existing_overlay() {
        let root = fresh_test_dir("generate-model-command-empty-overlay");
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"billing-service\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");
        write_file(&root, "c4/model.generated.yml", "");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let diff = generate_model_for_repo(repo, GenerateModelParams { scan_first: false }, None)
            .expect("generate model candidate");

        assert_eq!(diff.before_yaml.as_deref(), Some(""));
        assert_eq!(
            diff.base_overlay_sha.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );

        cleanup(root);
    }

    #[test]
    fn generate_model_for_repo_records_existing_scan_token_without_scan() {
        let root = fresh_test_dir("generate-model-command-existing-index");
        let index_root = fresh_test_dir("generate-model-command-existing-index-root");
        let index_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"billing-service\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let summary = scan_codebase_for_repo(repo.clone(), false, Some(index_path.clone()))
            .expect("scan repo");
        let diff = generate_model_for_repo(
            repo,
            GenerateModelParams { scan_first: false },
            Some(index_path),
        )
        .expect("generate model candidate");

        assert_eq!(diff.index_scan_token, summary.scan_token);

        cleanup(index_root);
        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn generate_model_for_repo_rejects_symlinked_c4_parent_without_overlay() {
        use std::os::unix::fs::symlink;

        let root = fresh_test_dir("generate-model-command-symlink-c4");
        let outside = fresh_test_dir("generate-model-command-symlink-c4-outside");
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"billing-service\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");
        symlink(&outside, root.join("c4")).expect("symlink c4");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = generate_model_for_repo(repo, GenerateModelParams { scan_first: false }, None)
            .expect_err("symlinked c4 parent should be rejected");

        assert_eq!(error.code, "repo.path_denied");
        assert!(!outside.join("model.generated.yml").exists());

        cleanup(root);
        cleanup(outside);
    }

    #[test]
    fn generate_model_for_repo_is_deterministic_without_scan() {
        let root = fresh_test_dir("generate-model-command-repo-deterministic");
        write_file(
            &root,
            "Cargo.toml",
            "[package]\nname = \"billing-service\"\nversion = \"0.1.0\"\n",
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let first = generate_model_for_repo(
            repo.clone(),
            GenerateModelParams { scan_first: false },
            None,
        )
        .expect("generate model first");
        let second = generate_model_for_repo(
            repo.clone(),
            GenerateModelParams { scan_first: false },
            None,
        )
        .expect("generate model second");

        assert_eq!(first.after_yaml, second.after_yaml);
        assert_eq!(first.summary, second.summary);
        assert_eq!(first.candidate_id, second.candidate_id);
        assert!(first.index_scan_token.is_empty());
        assert!(!first.changes.is_empty());

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
