use std::fs;

use assert_cmd::Command;
use c4lens_core::{acquire_repo_write_lock, repo_handle_from_path, BUNDLED_MODEL_SCHEMA_JSON};
use predicates::prelude::*;
use serde_json::Value;

mod support;

use support::{cleanup, fresh_test_dir, write_model};

#[test]
fn init_creates_authored_model_and_schema() {
    let repo = fresh_test_dir("init-create");

    Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["init", "--name", "Billing API", "--repo"])
        .arg(&repo)
        .assert()
        .success()
        .stdout(predicate::str::contains("created c4/model.yml"))
        .stdout(predicate::str::contains("refreshed c4/schema.json"));

    let model = fs::read_to_string(repo.join("c4/model.yml")).expect("model");
    assert_eq!(
        model,
        "# c4/model.yml\n# Authored C4 model for c4lens.\n# yaml-language-server: $schema=./schema.json\nname: 'Billing API'\n"
    );
    assert_eq!(
        fs::read_to_string(repo.join("c4/schema.json")).expect("schema"),
        BUNDLED_MODEL_SCHEMA_JSON
    );

    cleanup(repo);
}

#[test]
fn init_json_reports_created_paths() {
    let repo = fresh_test_dir("init-json");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["init", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .success();
    let payload: Value = serde_json::from_slice(&assert.get_output().stdout).expect("json");

    assert_eq!(payload["ok"], true);
    assert_eq!(payload["modelPath"], "c4/model.yml");
    assert_eq!(payload["schemaPath"], "c4/schema.json");
    assert_eq!(
        payload["modelName"],
        repo.file_name().unwrap().to_string_lossy().as_ref()
    );

    cleanup(repo);
}

#[test]
fn init_escapes_yaml_model_name() {
    let repo = fresh_test_dir("init-escape");

    Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["init", "--name", "Sal's API", "--repo"])
        .arg(&repo)
        .assert()
        .success();

    let model = fs::read_to_string(repo.join("c4/model.yml")).expect("model");
    assert!(model.contains("name: 'Sal''s API'\n"));

    cleanup(repo);
}

#[test]
fn init_refuses_to_overwrite_authored_model() {
    let repo = fresh_test_dir("init-existing");
    write_model(&repo, "name: Existing\n");

    Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["init", "--repo"])
        .arg(&repo)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("c4/model.yml already exists."));

    assert_eq!(
        fs::read_to_string(repo.join("c4/model.yml")).expect("model"),
        "name: Existing\n"
    );

    cleanup(repo);
}

#[test]
fn init_reports_write_lock_in_json() {
    let repo = fresh_test_dir("init-locked");
    let repo_handle = repo_handle_from_path(&repo).expect("repo handle");
    let _lock = acquire_repo_write_lock(&repo_handle).expect("lock");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["init", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .code(3);
    let payload: Value = serde_json::from_slice(&assert.get_output().stdout).expect("json");

    assert_eq!(payload["ok"], false);
    assert_eq!(payload["issues"][0]["stage"], "init");
    assert_eq!(payload["issues"][0]["code"], "repo.write_locked");
    assert!(!repo.join("c4/model.yml").exists());

    cleanup(repo);
}

#[test]
fn schema_refreshes_existing_schema() {
    let repo = fresh_test_dir("schema-refresh");
    fs::create_dir_all(repo.join("c4")).expect("create c4");
    fs::write(repo.join("c4/schema.json"), "{\"title\":\"stale\"}\n").expect("stale schema");

    Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["schema", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"schemaPath\": \"c4/schema.json\"",
        ));

    assert_eq!(
        fs::read_to_string(repo.join("c4/schema.json")).expect("schema"),
        BUNDLED_MODEL_SCHEMA_JSON
    );

    cleanup(repo);
}

#[test]
fn schema_reports_write_lock_in_json() {
    let repo = fresh_test_dir("schema-locked");
    let repo_handle = repo_handle_from_path(&repo).expect("repo handle");
    let _lock = acquire_repo_write_lock(&repo_handle).expect("lock");

    let assert = Command::cargo_bin("c4lens")
        .expect("binary")
        .args(["schema", "--json", "--repo"])
        .arg(&repo)
        .assert()
        .code(3);
    let payload: Value = serde_json::from_slice(&assert.get_output().stdout).expect("json");

    assert_eq!(payload["ok"], false);
    assert_eq!(payload["issues"][0]["stage"], "schema");
    assert_eq!(payload["issues"][0]["code"], "repo.write_locked");
    assert!(!repo.join("c4/schema.json").exists());

    cleanup(repo);
}
