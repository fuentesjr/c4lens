use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use c4lens_core::{
    acquire_repo_write_lock, build_minimal_generated_model_from_authored_system,
    canonicalize_repo_root, load_effective_model_from_repo,
    load_effective_model_from_repo_recovering_generated_overlay, read_generated_overlay,
    render_generated_model_yaml, repo_handle_from_path, scan_repo,
    single_authored_internal_system_for_generation, validate_generated_overlay_paths,
    validate_generated_overlay_yaml, write_generated_overlay_to_path, write_schema_json,
    write_schema_json_if_missing, CommandError, RepoHandle, ScanOptions, ValidationSeverity,
    GENERATED_MODEL_PATH, SCHEMA_PATH,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "c4lens")]
#[command(version)]
#[command(about = "Local C4 model validation, scanning, and generation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Doctor {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Init {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
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
    Schema {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Doctor { repo, json } => run_doctor(repo, json),
        Command::Init { repo, name, json } => run_init(repo, name, json),
        Command::Validate { repo, json } => run_validate(repo, json),
        Command::Scan { repo, force, json } => run_scan(repo, force, json),
        Command::Generate {
            repo,
            scan,
            check,
            write,
            json,
        } => run_generate(repo, scan, check, write, json),
        Command::Schema { repo, json } => run_schema(repo, json),
    };

    process::exit(exit_code);
}

fn run_doctor(repo: Option<PathBuf>, json: bool) -> i32 {
    let repo = match resolve_repo(repo) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("{}", err);
            return 3;
        }
    };

    let result = inspect_repo_health(&repo);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result.json)
                .expect("failed to serialize doctor response")
        );
    } else {
        println!("repo: {}", repo.name);
        println!("model: {}", result.model_status);
        println!("schema: {}", result.schema_status);
        println!("generated overlay: {}", result.generated_status);
        println!(
            "validation: {}",
            if result.validation_errors > 0 {
                format!("{} errors", result.validation_errors)
            } else if result.validation_warnings > 0 {
                format!("{} warnings", result.validation_warnings)
            } else {
                "ok".to_string()
            }
        );
        if result.recommendations.is_empty() {
            println!("status: ready");
        } else {
            println!("status: action needed");
            for recommendation in &result.recommendations {
                println!("- {recommendation}");
            }
        }
    }

    if result.ready {
        0
    } else {
        1
    }
}

fn run_init(repo: Option<PathBuf>, name: Option<String>, json: bool) -> i32 {
    let repo = match resolve_repo(repo) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("{}", err);
            return 3;
        }
    };

    let model_name = name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&repo.name);

    match init_repo_model(&repo, model_name) {
        Ok(result) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "repo": repo.name,
                        "modelPath": result.model_path,
                        "schemaPath": result.schema_path,
                        "modelName": model_name,
                    }))
                    .expect("failed to serialize init response")
                );
            } else {
                println!("created {}", result.model_path);
                println!("refreshed {}", result.schema_path);
            }
            0
        }
        Err(error) => {
            print_command_error(&error, json, "init");
            if error.code == "repo.write_locked" {
                3
            } else {
                1
            }
        }
    }
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

    let authored_internal_system = single_authored_internal_system_for_generation(&repo);
    let generated = build_minimal_generated_model_from_authored_system(
        &repo,
        authored_internal_system.as_ref(),
    );
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
        let before_yaml = match read_generated_overlay(&repo) {
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

fn run_schema(repo: Option<PathBuf>, json: bool) -> i32 {
    let repo = match resolve_repo(repo) {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("{}", err);
            return 3;
        }
    };

    let result = refresh_repo_schema(&repo);
    match result {
        Ok(schema_path) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "repo": repo.name,
                        "schemaPath": schema_path,
                    }))
                    .expect("failed to serialize schema response")
                );
            } else {
                println!("refreshed {schema_path}");
            }
            0
        }
        Err(error) => {
            print_command_error(&error, json, "schema");
            if error.code == "repo.write_locked" {
                3
            } else {
                4
            }
        }
    }
}

struct InitResult {
    model_path: &'static str,
    schema_path: &'static str,
}

fn init_repo_model(repo: &RepoHandle, model_name: &str) -> Result<InitResult, CommandError> {
    let _write_lock = acquire_repo_write_lock(repo)?;
    let repo_root = canonicalize_repo_root(repo)?;
    let (model_dir, _) = validate_generated_overlay_paths(&repo_root)?;
    let model_path = model_dir.join("model.yml");
    ensure_model_can_be_created(&model_path)?;
    write_schema_json(repo)?;

    let mut model_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&model_path)
        .map_err(|error| {
            CommandError::with_details(
                "fs.write_failed",
                "Failed to create authored model.",
                serde_json::json!({ "path": "c4/model.yml", "error": error.to_string() }),
            )
        })?;
    let model_yaml = initial_model_yaml(model_name);
    model_file
        .write_all(model_yaml.as_bytes())
        .map_err(|error| {
            CommandError::with_details(
                "fs.write_failed",
                "Failed to write authored model.",
                serde_json::json!({ "path": "c4/model.yml", "error": error.to_string() }),
            )
        })?;
    model_file.sync_all().map_err(|error| {
        CommandError::with_details(
            "fs.write_failed",
            "Failed to sync authored model.",
            serde_json::json!({ "path": "c4/model.yml", "error": error.to_string() }),
        )
    })?;

    Ok(InitResult {
        model_path: "c4/model.yml",
        schema_path: SCHEMA_PATH,
    })
}

fn refresh_repo_schema(repo: &RepoHandle) -> Result<&'static str, CommandError> {
    let _write_lock = acquire_repo_write_lock(repo)?;
    write_schema_json(repo)?;
    Ok(SCHEMA_PATH)
}

fn ensure_model_can_be_created(model_path: &Path) -> Result<(), CommandError> {
    match fs::symlink_metadata(model_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(CommandError::with_details(
                    "repo.path_denied",
                    "Authored model file must not be a symlink.",
                    serde_json::json!({ "path": "c4/model.yml" }),
                ));
            }
            Err(CommandError::with_details(
                "init.already_exists",
                "c4/model.yml already exists.",
                serde_json::json!({ "path": "c4/model.yml" }),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::with_details(
            "fs.read_failed",
            "Failed to inspect authored model.",
            serde_json::json!({ "path": "c4/model.yml", "error": error.to_string() }),
        )),
    }
}

fn initial_model_yaml(model_name: &str) -> String {
    format!(
        "# c4/model.yml\n# Authored C4 model for c4lens.\n# yaml-language-server: $schema=./schema.json\nname: {}\n",
        yaml_single_quoted(model_name)
    )
}

fn yaml_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

struct DoctorResult {
    ready: bool,
    model_status: &'static str,
    schema_status: &'static str,
    generated_status: &'static str,
    validation_errors: usize,
    validation_warnings: usize,
    recommendations: Vec<String>,
    json: serde_json::Value,
}

fn inspect_repo_health(repo: &RepoHandle) -> DoctorResult {
    let repo_root = match canonicalize_repo_root(repo) {
        Ok(root) => root,
        Err(error) => {
            let issue = serde_json::json!({
                "severity": "error",
                "stage": "doctor",
                "code": error.code,
                "message": error.message,
                "details": error.details,
            });
            return DoctorResult {
                ready: false,
                model_status: "unavailable",
                schema_status: "unavailable",
                generated_status: "unavailable",
                validation_errors: 1,
                validation_warnings: 0,
                recommendations: vec!["Open an existing repository path.".to_string()],
                json: serde_json::json!({
                    "ok": false,
                    "repo": repo.name,
                    "rootPath": repo.root_path,
                    "model": { "path": "c4/model.yml", "exists": false },
                    "schema": { "path": SCHEMA_PATH, "exists": false },
                    "generatedOverlay": { "path": GENERATED_MODEL_PATH, "exists": false },
                    "validation": { "ok": false, "issues": [issue] },
                    "recommendations": ["Open an existing repository path."],
                }),
            };
        }
    };

    let model_exists = repo_root.join("c4/model.yml").is_file();
    let schema_exists = repo_root.join(SCHEMA_PATH).is_file();
    let generated_exists = repo_root.join(GENERATED_MODEL_PATH).is_file();
    let mut recommendations = Vec::new();
    if !model_exists {
        recommendations.push("Run c4lens init --repo <repo> --name \"My System\".".to_string());
    }
    if !schema_exists {
        recommendations
            .push("Run c4lens schema --repo <repo> to refresh editor schema.".to_string());
    }

    let validation_json =
        match load_effective_model_from_repo_recovering_generated_overlay(repo.clone()) {
            Ok(effective) => {
                let validation_errors = effective
                    .validation
                    .issues
                    .iter()
                    .filter(|issue| issue.severity == ValidationSeverity::Error)
                    .count();
                let validation_warnings = effective
                    .validation
                    .issues
                    .iter()
                    .filter(|issue| issue.severity == ValidationSeverity::Warning)
                    .count();
                if validation_errors > 0 {
                    recommendations.push("Fix validation errors in c4/model.yml.".to_string());
                }
                DoctorValidation {
                    ok: validation_errors == 0,
                    errors: validation_errors,
                    warnings: validation_warnings,
                    json: serde_json::to_value(effective.validation)
                        .expect("failed to serialize validation report"),
                }
            }
            Err(error) => {
                let code = error.code.clone();
                if code != "model.not_found" {
                    recommendations.push(
                        "Fix model loading errors before opening the repo in c4lens.".to_string(),
                    );
                }
                DoctorValidation {
                    ok: false,
                    errors: 1,
                    warnings: 0,
                    json: serde_json::json!({
                        "ok": false,
                        "issues": [{
                            "severity": "error",
                            "stage": validation_stage_for_error(&error.code),
                            "code": error.code,
                            "message": error.message,
                            "details": error.details,
                        }],
                    }),
                }
            }
        };

    let ready = model_exists && schema_exists && validation_json.ok;
    let model_status = if model_exists { "present" } else { "missing" };
    let schema_status = if schema_exists { "present" } else { "missing" };
    let generated_status = if generated_exists {
        "present"
    } else {
        "missing"
    };
    let json_recommendations = recommendations.clone();

    DoctorResult {
        ready,
        model_status,
        schema_status,
        generated_status,
        validation_errors: validation_json.errors,
        validation_warnings: validation_json.warnings,
        recommendations,
        json: serde_json::json!({
            "ok": ready,
            "repo": repo.name,
            "rootPath": repo.root_path,
            "model": { "path": "c4/model.yml", "exists": model_exists },
            "schema": { "path": SCHEMA_PATH, "exists": schema_exists },
            "generatedOverlay": { "path": GENERATED_MODEL_PATH, "exists": generated_exists },
            "validation": validation_json.json,
            "recommendations": json_recommendations,
        }),
    }
}

struct DoctorValidation {
    ok: bool,
    errors: usize,
    warnings: usize,
    json: serde_json::Value,
}

fn write_generated_overlay_with_lock(
    repo: &RepoHandle,
    generated_yaml: &str,
) -> Result<(), CommandError> {
    let _write_lock = acquire_repo_write_lock(repo)?;

    let repo_root = canonicalize_repo_root(repo)?;
    let (generated_dir, generated_path) = validate_generated_overlay_paths(&repo_root)?;
    validate_generated_overlay_yaml(repo.clone(), generated_yaml)?;

    write_schema_json_if_missing(&generated_dir)?;
    write_generated_overlay_to_path(&generated_path, generated_yaml)
}

fn print_generate_error(error: &CommandError, json: bool) {
    print_command_error(error, json, "generate");
}

fn print_command_error(error: &CommandError, json: bool, stage: &str) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": false,
                "issues": [{
                    "severity": "error",
                    "stage": stage,
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
