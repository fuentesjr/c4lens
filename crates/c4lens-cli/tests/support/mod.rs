// Each integration test compiles this support module separately, so a helper
// can be intentionally unused in one test crate while used in another.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn fresh_test_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("c4lens-cli-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&root).expect("create test root");
    root
}

pub fn write_model(repo: &Path, contents: &str) {
    write_file(repo, "c4/model.yml", contents);
}

pub fn write_generated_model(repo: &Path, contents: &str) {
    write_file(repo, "c4/model.generated.yml", contents);
}

pub fn write_file(repo: &Path, relative_path: &str, contents: &str) {
    write_file_bytes(repo, relative_path, contents.as_bytes());
}

pub fn write_file_bytes(repo: &Path, relative_path: &str, contents: &[u8]) {
    let path = repo.join(relative_path);
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, contents).expect("write file");
}

pub fn cleanup(root: PathBuf) {
    fs::remove_dir_all(root).ok();
}
