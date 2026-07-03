use assert_cmd::Command;
use c4lens_core::{acquire_repo_write_lock, repo_handle_from_path};
use serde_json::Value;

mod support;

use support::{cleanup, fresh_test_dir, write_file, write_file_bytes, write_model};

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
    write_file(&repo, "src/main.rs", "use std::io;\nfn main() {}\n");
    write_file(&repo, "src/lib.rs", "pub fn run() {}\n");
    let index_dir = fresh_test_dir("scan-json-files-index");

    let assert = Command::cargo_bin("c4lens")
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
    assert_eq!(summary["symbols"], 2);
    assert_eq!(summary["imports"], 1);
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

    Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["scan", "--json", "--repo"])
        .arg(&repo)
        .env("C4LENS_INDEX_DIR", &index_dir)
        .assert()
        .success();

    let assert = Command::cargo_bin("c4lens")
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

#[test]
fn scan_reports_write_locked_when_writer_is_active() {
    let repo = fresh_test_dir("scan-write-locked");
    let lock = acquire_repo_write_lock(&repo_handle_from_path(&repo).expect("repo handle"))
        .expect("lock held for test");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["scan", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .code(3);
    let payload: Value = serde_json::from_slice(&assert.get_output().stdout).expect("json error");

    assert_eq!(payload["issues"][0]["code"], "repo.write_locked");

    drop(lock);
    cleanup(repo);
}

#[test]
fn scan_json_reports_scanner_limit_warnings() {
    let repo = fresh_test_dir("scan-json-limit-warnings");
    write_model(&repo, "name: Scan Repo\n");
    write_file_bytes(&repo, "src/binary.rs", b"fn binary()\0{}\n");
    let index_dir = fresh_test_dir("scan-json-limit-warnings-index");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["scan", "--json", "--repo"])
        .arg(&repo)
        .env("C4LENS_INDEX_DIR", &index_dir)
        .assert()
        .success();
    let output = assert.get_output();
    let summary: Value = serde_json::from_slice(&output.stdout).expect("valid json");

    let warnings = summary["warnings"].as_array().expect("warnings");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["severity"], "warning");
    assert_eq!(warnings[0]["stage"], "scan");
    assert_eq!(warnings[0]["code"], "scan.binary_file_skipped");
    assert_eq!(warnings[0]["path"], "src/binary.rs");

    cleanup(repo);
    cleanup(index_dir);
}
