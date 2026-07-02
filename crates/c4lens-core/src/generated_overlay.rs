use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    CommandError, RepoHandle, BUNDLED_MODEL_SCHEMA_JSON, GENERATED_MODEL_PATH, SCHEMA_PATH,
};

pub fn canonicalize_repo_root(repo: &RepoHandle) -> Result<PathBuf, CommandError> {
    Path::new(&repo.root_path).canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.invalid",
            "Failed to resolve repository path.",
            serde_json::json!({ "path": repo.root_path, "error": error.to_string() }),
        )
    })
}

pub fn read_generated_overlay(repo: &RepoHandle) -> Result<Option<String>, CommandError> {
    let repo_root = canonicalize_repo_root(repo)?;
    read_generated_overlay_from_path(&repo_root, &repo_root.join(GENERATED_MODEL_PATH))
}

pub fn read_generated_overlay_from_path(
    repo_root: &Path,
    generated_path: &Path,
) -> Result<Option<String>, CommandError> {
    let generated_dir = generated_path.parent().ok_or_else(|| {
        CommandError::new("generation.failed", "Generated model path is invalid.")
    })?;

    let dir_metadata = match fs::symlink_metadata(generated_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CommandError::with_details(
                "fs.read_failed",
                "Failed to inspect generated model directory.",
                serde_json::json!({ "path": "c4", "error": error.to_string() }),
            ));
        }
    };
    if dir_metadata.file_type().is_symlink() {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Generated model directory must not be a symlink.",
            serde_json::json!({ "path": "c4" }),
        ));
    }
    if !dir_metadata.is_dir() {
        return Err(CommandError::with_details(
            "path.invalid_target",
            "Generated model parent exists but is not a directory.",
            serde_json::json!({ "path": "c4" }),
        ));
    }

    let canonical_generated_dir = generated_dir.canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.path_denied",
            "Failed to resolve generated model directory.",
            serde_json::json!({ "path": "c4", "error": error.to_string() }),
        )
    })?;
    if !canonical_generated_dir.starts_with(repo_root) {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Generated model directory resolves outside the repository.",
            serde_json::json!({ "path": "c4" }),
        ));
    }

    let file_metadata = match fs::symlink_metadata(generated_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CommandError::with_details(
                "fs.read_failed",
                "Failed to inspect generated model.",
                serde_json::json!({ "path": GENERATED_MODEL_PATH, "error": error.to_string() }),
            ))
        }
    };
    if file_metadata.file_type().is_symlink() {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Generated model file must not be a symlink.",
            serde_json::json!({ "path": GENERATED_MODEL_PATH }),
        ));
    }
    if !file_metadata.is_file() {
        return Err(CommandError::with_details(
            "path.invalid_target",
            "Generated model path exists but is not a file.",
            serde_json::json!({ "path": GENERATED_MODEL_PATH }),
        ));
    }

    fs::read_to_string(generated_path)
        .map(Some)
        .map_err(|error| {
            CommandError::with_details(
                "fs.read_failed",
                "Failed to read generated model.",
                serde_json::json!({ "path": GENERATED_MODEL_PATH, "error": error.to_string() }),
            )
        })
}

pub fn validate_generated_overlay_paths(
    repo_root: &Path,
) -> Result<(PathBuf, PathBuf), CommandError> {
    let generated_path = repo_root.join(GENERATED_MODEL_PATH);
    let generated_dir = generated_path.parent().ok_or_else(|| {
        CommandError::new("generation.failed", "Generated model path is invalid.")
    })?;

    if let Ok(metadata) = fs::symlink_metadata(generated_dir) {
        if metadata.file_type().is_symlink() {
            return Err(CommandError::with_details(
                "repo.path_denied",
                "Generated model directory must not be a symlink.",
                serde_json::json!({ "path": "c4" }),
            ));
        }
        if !metadata.is_dir() {
            return Err(CommandError::with_details(
                "path.invalid_target",
                "Generated model parent exists but is not a directory.",
                serde_json::json!({ "path": "c4" }),
            ));
        }
    }

    fs::create_dir_all(generated_dir).map_err(|error| {
        CommandError::with_details(
            "fs.write_failed",
            "Failed to create c4 directory.",
            serde_json::json!({ "path": "c4", "error": error.to_string() }),
        )
    })?;

    let canonical_generated_dir = generated_dir.canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.path_denied",
            "Failed to resolve generated model directory.",
            serde_json::json!({ "path": "c4", "error": error.to_string() }),
        )
    })?;
    if !canonical_generated_dir.starts_with(repo_root) {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Generated model directory resolves outside the repository.",
            serde_json::json!({ "path": "c4" }),
        ));
    }

    if let Ok(metadata) = fs::symlink_metadata(&generated_path) {
        if metadata.file_type().is_symlink() {
            return Err(CommandError::with_details(
                "repo.path_denied",
                "Generated model file must not be a symlink.",
                serde_json::json!({ "path": GENERATED_MODEL_PATH }),
            ));
        }
        if !metadata.is_file() {
            return Err(CommandError::with_details(
                "path.invalid_target",
                "Generated model path exists but is not a file.",
                serde_json::json!({ "path": GENERATED_MODEL_PATH }),
            ));
        }
    }

    Ok((generated_dir.to_path_buf(), generated_path))
}

pub fn write_schema_json_if_missing(generated_dir: &Path) -> Result<(), CommandError> {
    let schema_path = generated_dir.join("schema.json");

    if schema_json_exists(&schema_path)? {
        return Ok(());
    }

    write_schema_json_to_path(&schema_path)
}

fn schema_json_exists(schema_path: &Path) -> Result<bool, CommandError> {
    match fs::symlink_metadata(schema_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(CommandError::with_details(
                    "repo.path_denied",
                    "Schema file must not be a symlink.",
                    serde_json::json!({ "path": SCHEMA_PATH }),
                ));
            }
            if !metadata.is_file() {
                return Err(CommandError::with_details(
                    "path.invalid_target",
                    "Schema path exists but is not a file.",
                    serde_json::json!({ "path": SCHEMA_PATH }),
                ));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to inspect schema file.",
            serde_json::json!({ "path": SCHEMA_PATH, "error": error.to_string() }),
        )),
    }
}

fn write_schema_json_to_path(schema_path: &Path) -> Result<(), CommandError> {
    let schema_dir = schema_path
        .parent()
        .ok_or_else(|| CommandError::new("generation.failed", "Schema file path is invalid."))?;

    let temp_path = schema_dir.join(format!(
        ".schema.json.tmp.{}.{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));

    let mut temp_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|error| {
            CommandError::with_details(
                "fs.write_failed",
                "Failed to create temporary schema file.",
                serde_json::json!({ "path": temp_path.display().to_string(), "error": error.to_string() }),
            )
        })?;

    if let Err(error) = temp_file.write_all(BUNDLED_MODEL_SCHEMA_JSON.as_bytes()) {
        let _ = fs::remove_file(&temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to write temporary schema file.",
            serde_json::json!({ "path": temp_path.display().to_string(), "error": error.to_string() }),
        ));
    }

    if let Err(error) = temp_file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to sync temporary schema file.",
            serde_json::json!({ "path": temp_path.display().to_string(), "error": error.to_string() }),
        ));
    }

    drop(temp_file);

    match fs::hard_link(&temp_path, schema_path) {
        Ok(()) => {
            let _ = fs::remove_file(&temp_path);
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temp_path);
            schema_json_exists(schema_path)?;
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            return Err(CommandError::with_details(
                "fs.write_failed",
                "Failed to create schema file.",
                serde_json::json!({ "path": SCHEMA_PATH, "error": error.to_string() }),
            ));
        }
    }

    Ok(())
}

pub fn write_generated_overlay_to_path(
    generated_path: &Path,
    generated_yaml: &str,
) -> Result<(), CommandError> {
    let generated_dir = generated_path.parent().ok_or_else(|| {
        CommandError::new("generation.failed", "Generated model path is invalid.")
    })?;
    let temp_path = write_generated_overlay_to_temp_file(generated_dir, generated_yaml)?;
    promote_generated_overlay(&temp_path, generated_path)
}

pub fn write_generated_overlay_to_temp_file(
    generated_dir: &Path,
    generated_yaml: &str,
) -> Result<PathBuf, CommandError> {
    let temp_path = generated_dir.join(format!(
        ".model.generated.yml.tmp.{}.{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));

    let mut temp_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|error| {
            CommandError::with_details(
                "fs.write_failed",
                "Failed to create temporary generated model.",
                serde_json::json!({
                    "path": temp_path.display().to_string(),
                    "error": error.to_string()
                }),
            )
        })?;

    if let Err(error) = temp_file.write_all(generated_yaml.as_bytes()) {
        let _ = fs::remove_file(&temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to write temporary generated model.",
            serde_json::json!({
                "path": temp_path.display().to_string(),
                "error": error.to_string()
            }),
        ));
    }

    if let Err(error) = temp_file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to sync temporary generated model.",
            serde_json::json!({
                "path": temp_path.display().to_string(),
                "error": error.to_string()
            }),
        ));
    }

    Ok(temp_path)
}

pub fn promote_generated_overlay(
    temp_path: &Path,
    generated_path: &Path,
) -> Result<(), CommandError> {
    if let Err(error) = fs::rename(temp_path, generated_path) {
        let _ = fs::remove_file(temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to replace generated model.",
            serde_json::json!({ "path": GENERATED_MODEL_PATH, "error": error.to_string() }),
        ));
    }

    Ok(())
}
