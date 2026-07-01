use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use serde_json::Value;

#[test]
fn scan_json_indexes_files_and_model_code_sources() {
    let repo = fresh_test_dir("scan-json-files");
    write_model(
        &repo,
        r#"
name: Scan Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
    );
    write_file(&repo, "src/main.rs", "fn main() {}\n");
    write_file(&repo, "src/lib.rs", "pub fn run() {}\n");
    let index_dir = fresh_test_dir("scan-json-files-index");

    let assert = Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["scan", "--json", "--repo"])
        .arg(&repo)
        .env("C4LENS_INDEX_DIR", &index_dir)
        .assert()
        .success();
    let output = assert.get_output();
    let summary: Value = serde_json::from_slice(&output.stdout).expect("valid json");

    assert_eq!(
        summary["repo"]["name"],
        repo.file_name()
            .and_then(|name| name.to_str())
            .expect("repo name")
    );
    assert_eq!(summary["scannedFiles"], 3);
    assert_eq!(summary["changedFiles"], 3);
    assert_eq!(summary["deletedFiles"], 0);
    assert_eq!(summary["symbols"], 0);
    assert_eq!(summary["imports"], 0);
    assert!(summary["scanToken"].as_str().expect("scan token").len() >= 32);
    assert_eq!(summary["warnings"].as_array().expect("warnings").len(), 0);

    cleanup(repo);
    cleanup(index_dir);
}

#[test]
fn scan_json_reports_zero_changed_files_on_unchanged_rescan() {
    let repo = fresh_test_dir("scan-json-unchanged");
    write_model(&repo, "name: Scan Repo\n");
    write_file(&repo, "src/main.rs", "fn main() {}\n");
    let index_dir = fresh_test_dir("scan-json-unchanged-index");

    Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["scan", "--json", "--repo"])
        .arg(&repo)
        .env("C4LENS_INDEX_DIR", &index_dir)
        .assert()
        .success();

    let assert = Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["scan", "--json", "--repo"])
        .arg(&repo)
        .env("C4LENS_INDEX_DIR", &index_dir)
        .assert()
        .success();
    let output = assert.get_output();
    let summary: Value = serde_json::from_slice(&output.stdout).expect("valid json");

    assert_eq!(summary["scannedFiles"], 2);
    assert_eq!(summary["changedFiles"], 0);
    assert_eq!(summary["deletedFiles"], 0);

    cleanup(repo);
    cleanup(index_dir);
}

fn fresh_test_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("c4lens-cli-{name}-{unique}"));
    fs::create_dir_all(&root).expect("create test root");
    root
}

fn write_model(repo: &PathBuf, contents: &str) {
    write_file(repo, "c4/model.yml", contents);
}

fn write_file(repo: &PathBuf, relative_path: &str, contents: &str) {
    let path = repo.join(relative_path);
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, contents).expect("write file");
}

fn cleanup(root: PathBuf) {
    fs::remove_dir_all(root).ok();
}
