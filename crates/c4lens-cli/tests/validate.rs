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
    assert_eq!(report["issues"][0]["code"], "parse.invalid_yaml");

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

#[test]
fn validate_json_outputs_validation_report_for_top_level_schema_error() {
    let repo = fresh_test_dir("schema-error-json");
    write_model(
        &repo,
        r#"
name: Schema Error
unexpected: true
"#,
    );

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
    assert_eq!(report["issues"][0]["stage"], "schema");
    assert_eq!(report["issues"][0]["code"], "schema.additional_property");

    cleanup(repo);
}

#[test]
fn validate_json_outputs_validation_report_for_relationship_schema_error() {
    let repo = fresh_test_dir("relationship-schema-error-json");
    write_model(
        &repo,
        r#"
name: Relationship Schema Error
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: customer
    to: banking
    description: Uses
    protocol: HTTPS
"#,
    );

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
    assert_eq!(report["issues"][0]["stage"], "schema");
    assert_eq!(report["issues"][0]["code"], "schema.additional_property");

    cleanup(repo);
}

#[test]
fn validate_json_outputs_validation_report_for_unresolved_relationship() {
    let repo = fresh_test_dir("unresolved-relationship-json");
    write_model(
        &repo,
        r#"
name: Unresolved Relationship
actors:
  customer:
    name: Customer
relationships:
  - from: customer
    to: missing
    description: Uses
"#,
    );

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
    assert_eq!(report["issues"][0]["stage"], "semantic");
    assert_eq!(
        report["issues"][0]["code"],
        "semantic.unresolved_relationship_target"
    );

    cleanup(repo);
}

#[test]
fn validate_json_outputs_validation_report_for_unresolved_relationship_source() {
    let repo = fresh_test_dir("unresolved-relationship-source-json");
    write_model(
        &repo,
        r#"
name: Unresolved Relationship Source
systems:
  banking:
    name: Banking
relationships:
  - from: missing
    to: banking
    description: Uses
"#,
    );

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
    assert_eq!(report["issues"][0]["stage"], "semantic");
    assert_eq!(
        report["issues"][0]["code"],
        "semantic.unresolved_relationship_source"
    );

    cleanup(repo);
}

#[test]
fn validate_json_outputs_warning_for_missing_code_path() {
    let repo = fresh_test_dir("missing-code-path-json");
    write_model(
        &repo,
        r#"
name: Missing Code Path
systems:
  banking:
    name: Banking
    code: src/banking
"#,
    );

    let assert = Command::cargo_bin("c4lens-cli")
        .expect("binary")
        .args(["validate", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .success();
    let output = assert.get_output();
    let report: Value = serde_json::from_slice(&output.stdout).expect("valid json");

    assert_eq!(report["ok"], true);
    assert_eq!(report["issues"][0]["severity"], "warning");
    assert_eq!(report["issues"][0]["stage"], "semantic");
    assert_eq!(report["issues"][0]["code"], "semantic.code_path_missing");

    cleanup(repo);
}

#[test]
fn validate_json_outputs_validation_report_for_duplicate_slug() {
    let repo = fresh_test_dir("duplicate-slug-json");
    write_model(
        &repo,
        r#"
name: Duplicate Slug
actors:
  customer:
    name: Customer
systems:
  customer:
    name: Customer Portal
"#,
    );

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
    assert_eq!(report["issues"][0]["stage"], "semantic");
    assert_eq!(report["issues"][0]["code"], "semantic.duplicate_slug");
    assert_eq!(report["issues"][0]["details"]["slug"], "customer");
    assert_eq!(
        report["issues"][0]["details"]["firstPath"],
        "/actors/customer"
    );
    assert_eq!(report["issues"][0]["details"]["path"], "/systems/customer");

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
