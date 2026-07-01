use std::path::PathBuf;
use std::process;

use c4lens_core::{load_effective_model_from_repo, repo_handle_from_path, scan_repo, ScanOptions};
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
        #[arg(long, default_value_t = false)]
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
            return 4;
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

    let response = serde_json::json!({
        "repo": repo.name,
        "scanRequested": scan,
        "checkRequested": check,
        "writeRequested": write,
        "message": "Phase-0 stub: generation is intentionally not implemented.",
    });

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&response).expect("failed to serialize generate response")
        );
    } else {
        println!("generate (Phase-0 stub) for {}", repo.name);
        println!("Generation workflow is intentionally deferred to Phase 1.");
    }

    if check && write {
        0
    } else {
        0
    }
}

fn resolve_repo(repo: Option<PathBuf>) -> Result<c4lens_core::RepoHandle, String> {
    let default_repo = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = repo.unwrap_or(default_repo);
    repo_handle_from_path(path).map_err(|error| error.message)
}
