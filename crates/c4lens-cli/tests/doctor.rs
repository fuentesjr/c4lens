use std::fs;

use assert_cmd::Command;
use c4lens_core::BUNDLED_MODEL_SCHEMA_JSON;
use predicates::prelude::*;
use serde_json::Value;

mod support;

use support::{cleanup, fresh_test_dir, write_model};

#[test]
fn doctor_reports_ready_repo() {
    let repo = fresh_test_dir("doctor-ready");
    write_model(&repo, "name: Ready\n");
    fs::write(repo.join("c4/schema.json"), BUNDLED_MODEL_SCHEMA_JSON).expect("schema");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["doctor", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .success();
    let payload: Value = serde_json::from_slice(&assert.get_output().stdout).expect("json");

    assert_eq!(payload["ok"], true);
    assert_eq!(payload["model"]["exists"], true);
    assert_eq!(payload["schema"]["exists"], true);
    assert_eq!(payload["generatedOverlay"]["exists"], false);
    assert_eq!(payload["validation"]["ok"], true);
    assert_eq!(
        payload["recommendations"]
            .as_array()
            .expect("recommendations")
            .len(),
        0
    );

    cleanup(repo);
}

#[test]
fn doctor_recommends_init_for_uninitialized_repo() {
    let repo = fresh_test_dir("doctor-uninitialized");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["doctor", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .code(1);
    let payload: Value = serde_json::from_slice(&assert.get_output().stdout).expect("json");

    assert_eq!(payload["ok"], false);
    assert_eq!(payload["model"]["exists"], false);
    assert_eq!(payload["schema"]["exists"], false);
    assert_eq!(
        payload["validation"]["issues"][0]["code"],
        "model.not_found"
    );
    let recommendations = payload["recommendations"]
        .as_array()
        .expect("recommendations")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(recommendations
        .iter()
        .any(|item| item.contains("c4lens init")));
    assert!(recommendations
        .iter()
        .any(|item| item.contains("c4lens schema")));

    cleanup(repo);
}

#[test]
fn doctor_reports_validation_errors() {
    let repo = fresh_test_dir("doctor-invalid");
    write_model(&repo, "name: [unterminated\n");
    fs::write(repo.join("c4/schema.json"), BUNDLED_MODEL_SCHEMA_JSON).expect("schema");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["doctor", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .code(1);
    let payload: Value = serde_json::from_slice(&assert.get_output().stdout).expect("json");

    assert_eq!(payload["ok"], false);
    assert_eq!(
        payload["validation"]["issues"][0]["code"],
        "parse.invalid_yaml"
    );
    assert!(payload["recommendations"]
        .as_array()
        .expect("recommendations")
        .iter()
        .any(|item| item
            .as_str()
            .unwrap_or_default()
            .contains("Fix model loading errors")));

    cleanup(repo);
}

#[test]
fn doctor_reports_schema_drift_as_warning() {
    let repo = fresh_test_dir("doctor-schema-drift");
    write_model(&repo, "name: Drift Warning\n");
    fs::write(repo.join("c4/schema.json"), "{\"title\":\"stale\"}\n").expect("schema");

    Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["doctor", "--repo"])
        .arg(&repo)
        .assert()
        .success()
        .stdout(predicate::str::contains("validation: 1 warnings"))
        .stdout(predicate::str::contains("status: ready"));

    cleanup(repo);
}
