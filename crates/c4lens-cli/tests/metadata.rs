use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_version_reports_release_version() {
    Command::cargo_bin("c4lens")
        .expect("binary")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "c4lens {}",
            env!("CARGO_PKG_VERSION")
        )));
}
