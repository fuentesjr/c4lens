use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

#[test]
fn validate_succeeds_for_valid_authored_model() {
    let repo = fresh_test_dir("valid-authored");
    write_model(&repo, "name: Valid\n");

    Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["validate", "--repo"])
        .arg(&repo)
        .assert()
        .success()
        .stdout(predicate::str::contains("model: Valid"))
        .stdout(predicate::str::contains("status: ok"));

    cleanup(repo);
}

#[test]
fn validate_json_outputs_validation_report_for_valid_model() {
    let repo = fresh_test_dir("valid-json");
    write_model(&repo, "name: Json Valid\n");

    let assert = Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["validate", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .success();
    let output = assert.get_output();
    let report: Value = serde_json::from_slice(&output.stdout).expect("valid json");

    assert_eq!(report["ok"], true);
    assert_eq!(report["issues"].as_array().expect("issues array").len(), 0);

    cleanup(repo);
}

#[test]
fn validate_fails_when_authored_model_is_missing() {
    let repo = fresh_test_dir("missing-model");
    fs::create_dir(repo.join("c4")).expect("create c4 dir");

    Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["validate", "--repo"])
        .arg(&repo)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("No c4/model.yml exists"));

    cleanup(repo);
}

#[test]
fn validate_fails_when_authored_yaml_is_invalid() {
    let repo = fresh_test_dir("invalid-yaml");
    write_model(&repo, "name: [unterminated\n");

    Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["validate", "--repo"])
        .arg(&repo)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Failed to parse c4/model.yml"));

    cleanup(repo);
}

#[test]
fn validate_json_outputs_validation_report_for_invalid_yaml() {
    let repo = fresh_test_dir("invalid-json");
    write_model(&repo, "name: [unterminated\n");

    let assert = Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["validate", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .code(1);
    let output = assert.get_output();
    let report: Value = serde_json::from_slice(&output.stdout).expect("valid json");

    assert_eq!(report["ok"], false);
    assert_eq!(report["issues"][0]["severity"], "error");
    assert_eq!(report["issues"][0]["code"], "model.invalid");

    cleanup(repo);
}

#[test]
fn validate_json_outputs_validation_report_for_missing_model() {
    let repo = fresh_test_dir("missing-json");
    fs::create_dir(repo.join("c4")).expect("create c4 dir");

    let assert = Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["validate", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .code(1);
    let output = assert.get_output();
    let report: Value = serde_json::from_slice(&output.stdout).expect("valid json");

    assert_eq!(report["ok"], false);
    assert_eq!(report["issues"][0]["severity"], "error");
    assert_eq!(report["issues"][0]["code"], "model.not_found");

    cleanup(repo);
}

fn write_model(repo: &PathBuf, contents: &str) {
    fs::create_dir_all(repo.join("c4")).expect("create c4 dir");
    fs::write(repo.join("c4/model.yml"), contents).expect("write model");
}

fn fresh_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("c4lens-cli-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir(&path).expect("create test dir");
    path
}

fn cleanup(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}
