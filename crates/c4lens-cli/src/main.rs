use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use c4lens_core::{
    acquire_repo_write_lock, build_minimal_generated_model, load_effective_model_from_repo,
    render_generated_model_yaml, repo_handle_from_path, scan_repo, CommandError, RepoHandle,
    ScanOptions, GENERATED_MODEL_PATH,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "c4lens")]
#[command(about = "Local C4 model validation, scanning, and generation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Validate {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Scan {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    Generate {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        scan: bool,
        #[arg(long, default_value_t = false, conflicts_with = "write")]
        check: bool,
        #[arg(long, default_value_t = false)]
        write: bool,
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Validate { repo, json } => run_validate(repo, json),
        Command::Scan { repo, force, json } => run_scan(repo, force, json),
        Command::Generate {
            repo,
            scan,
            check,
            write,
            json,
        } => run_generate(repo, scan, check, write, json),
    };

    process::exit(exit_code);
}

fn run_validate(repo: Option<PathBuf>, json: bool) -> i32 {
    let repo = match resolve_repo(repo) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("{}", err);
            return 3;
        }
    };

    let model = match load_effective_model_from_repo(repo.clone()) {
        Ok(model) => model,
        Err(error) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": false,
                        "issues": [{
                            "severity": "error",
                            "stage": validation_stage_for_error(&error.code),
                            "code": error.code,
                            "message": error.message,
                            "details": error.details,
                        }]
                    }))
                    .expect("failed to serialize validation error")
                );
            } else {
                eprintln!("{}", error.message);
            }
            return 1;
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&model.validation)
                .expect("failed to serialize validation report")
        );
    } else {
        println!("repo: {}", repo.name);
        println!("model: {}", model.model.name);
        println!("status: ok");
    }

    0
}

fn validation_stage_for_error(code: &str) -> &'static str {
    if code.starts_with("semantic.") || code == "path.invalid" {
        "semantic"
    } else if code.starts_with("schema.") {
        "schema"
    } else {
        "parse"
    }
}

fn run_scan(repo: Option<PathBuf>, force: bool, json: bool) -> i32 {
    let repo = match resolve_repo(repo) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("{}", err);
            return 3;
        }
    };

    let summary = match scan_repo(
        repo.clone(),
        ScanOptions {
            force,
            index_path: None,
        },
    ) {
        Ok(summary) => summary,
        Err(error) => {
            let exit_code = if error.code == "repo.write_locked" {
                3
            } else {
                4
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": false,
                        "issues": [{
                            "severity": "error",
                            "stage": "scan",
                            "code": error.code,
                            "message": error.message,
                            "details": error.details,
                        }]
                    }))
                    .expect("failed to serialize scan error")
                );
            } else {
                eprintln!("{}", error.message);
            }
            return exit_code;
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary).expect("failed to serialize summary")
        );
    } else {
        println!(
            "scanned={} changed={} deleted={} in {}",
            summary.scanned_files, summary.changed_files, summary.deleted_files, repo.name
        );
        println!("scan token: {}", summary.scan_token);
    }

    0
}

fn run_generate(repo: Option<PathBuf>, scan: bool, check: bool, write: bool, json: bool) -> i32 {
    let repo = match resolve_repo(repo) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("{}", err);
            return 3;
        }
    };

    if scan {
        if let Err(error) = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: None,
            },
        ) {
            let exit_code = if error.code == "repo.write_locked" {
                3
            } else {
                4
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": false,
                        "issues": [{
                            "severity": "error",
                            "stage": "scan",
                            "code": error.code,
                            "message": error.message,
                            "details": error.details,
                        }]
                    }))
                    .expect("failed to serialize scan error")
                );
            } else {
                eprintln!("{}", error.message);
            }
            return exit_code;
        }
    }

    let generated = build_minimal_generated_model(&repo);
    let generated_yaml = match render_generated_model_yaml(&generated) {
        Ok(yaml) => yaml,
        Err(error) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": false,
                        "issues": [{
                            "severity": "error",
                            "stage": "generate",
                            "code": error.code,
                            "message": error.message,
                            "details": error.details,
                        }]
                    }))
                    .expect("failed to serialize generate error")
                );
            } else {
                eprintln!("{}", error.message);
            }
            return 4;
        }
    };

    if write {
        if let Err(error) = write_generated_overlay_with_lock(&repo, &generated_yaml) {
            print_generate_error(&error, json);
            return if error.code == "repo.write_locked" {
                3
            } else {
                4
            };
        }

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "repo": repo.name,
                    "scanRequested": scan,
                    "checkRequested": check,
                    "writeRequested": true,
                    "overlayPath": GENERATED_MODEL_PATH,
                }))
                .expect("failed to serialize generate response")
            );
        }
        return 0;
    }

    if check {
        let before_yaml = match read_existing_generated_overlay(&repo) {
            Ok(before_yaml) => before_yaml.unwrap_or_default(),
            Err(error) => {
                print_generate_error(&error, json);
                return 4;
            }
        };
        let changed = before_yaml != generated_yaml;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": !changed,
                    "repo": repo.name,
                    "scanRequested": scan,
                    "checkRequested": true,
                    "writeRequested": false,
                    "overlayPath": GENERATED_MODEL_PATH,
                    "beforeYaml": before_yaml,
                    "afterYaml": generated_yaml,
                    "changed": changed,
                }))
                .expect("failed to serialize generate response")
            );
        } else if changed {
            eprintln!("Generated overlay differs from candidate; use --write to apply.");
        }
        return if changed { 1 } else { 0 };
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "repo": repo.name,
                "scanRequested": scan,
                "checkRequested": false,
                "writeRequested": false,
                "overlayPath": GENERATED_MODEL_PATH,
                "generatedYaml": generated_yaml,
            }))
            .expect("failed to serialize generate response")
        );
    } else {
        print!("{}", generated_yaml);
    }

    0
}

fn write_generated_overlay_with_lock(
    repo: &RepoHandle,
    generated_yaml: &str,
) -> Result<(), CommandError> {
    let _write_lock = acquire_repo_write_lock(repo)?;
    write_generated_overlay(repo, generated_yaml)
}

fn read_existing_generated_overlay(repo: &RepoHandle) -> Result<Option<String>, CommandError> {
    let repo_root = Path::new(&repo.root_path).canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.invalid",
            "Failed to resolve repository path.",
            serde_json::json!({ "path": repo.root_path, "error": error.to_string() }),
        )
    })?;
    let generated_path = repo_root.join(GENERATED_MODEL_PATH);
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
            ))
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
    if !canonical_generated_dir.starts_with(&repo_root) {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Generated model directory resolves outside the repository.",
            serde_json::json!({ "path": "c4" }),
        ));
    }

    let file_metadata = match fs::symlink_metadata(&generated_path) {
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

    fs::read_to_string(&generated_path)
        .map(Some)
        .map_err(|error| {
            CommandError::with_details(
                "fs.read_failed",
                "Failed to read generated model.",
                serde_json::json!({ "path": GENERATED_MODEL_PATH, "error": error.to_string() }),
            )
        })
}

fn write_generated_overlay(repo: &RepoHandle, generated_yaml: &str) -> Result<(), CommandError> {
    let repo_root = Path::new(&repo.root_path).canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.invalid",
            "Failed to resolve repository path.",
            serde_json::json!({ "path": repo.root_path, "error": error.to_string() }),
        )
    })?;
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
    if !canonical_generated_dir.starts_with(&repo_root) {
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
                serde_json::json!({ "path": temp_path.display().to_string(), "error": error.to_string() }),
            )
        })?;
    if let Err(error) = temp_file.write_all(generated_yaml.as_bytes()) {
        let _ = fs::remove_file(&temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to write temporary generated model.",
            serde_json::json!({ "path": temp_path.display().to_string(), "error": error.to_string() }),
        ));
    }
    if let Err(error) = temp_file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to sync temporary generated model.",
            serde_json::json!({ "path": temp_path.display().to_string(), "error": error.to_string() }),
        ));
    }
    drop(temp_file);

    if let Err(error) = fs::rename(&temp_path, &generated_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to replace generated model.",
            serde_json::json!({ "path": GENERATED_MODEL_PATH, "error": error.to_string() }),
        ));
    }

    Ok(())
}

fn print_generate_error(error: &CommandError, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": false,
                "issues": [{
                    "severity": "error",
                    "stage": "generate",
                    "code": error.code,
                    "message": error.message,
                    "details": error.details,
                }]
            }))
            .expect("failed to serialize generation error")
        );
    } else {
        eprintln!("{}", error.message);
    }
}

fn resolve_repo(repo: Option<PathBuf>) -> Result<c4lens_core::RepoHandle, String> {
    let default_repo = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = repo.unwrap_or(default_repo);
    repo_handle_from_path(path).map_err(|error| error.message)
}
