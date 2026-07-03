use std::path::PathBuf;
use std::process;

use c4lens_core::{
    acquire_repo_write_lock, build_minimal_generated_model_from_authored_system,
    canonicalize_repo_root, load_effective_model_from_repo, read_generated_overlay,
    render_generated_model_yaml, repo_handle_from_path, scan_repo,
    single_authored_internal_system_for_generation, validate_generated_overlay_paths,
    validate_generated_overlay_yaml, write_generated_overlay_to_path, write_schema_json_if_missing,
    CommandError, RepoHandle, ScanOptions, GENERATED_MODEL_PATH,
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
