use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::{CommandError, RepoHandle};

#[derive(Debug)]
pub struct RepoWriteLock {
    path: PathBuf,
}

impl Drop for RepoWriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn acquire_repo_write_lock(repo: &RepoHandle) -> Result<RepoWriteLock, CommandError> {
    let lock_path = repo_write_lock_path(repo)?;

    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(CommandError::new(
                "repo.write_locked",
                "Another c4lens writer is already updating this repository.",
            ));
        }
        Err(error) => {
            return Err(CommandError::with_details(
                "fs.write_failed",
                "Failed to acquire repository write lock.",
                serde_json::json!({
                    "path": lock_path.to_string_lossy(),
                    "error": error.to_string()
                }),
            ))
        }
    };

    if let Err(error) = file.write_all(std::process::id().to_string().as_bytes()) {
        let _ = fs::remove_file(&lock_path);
        return Err(CommandError::with_details(
            "fs.write_failed",
            "Failed to initialize repository write lock.",
            serde_json::json!({
                "path": lock_path.to_string_lossy(),
                "error": error.to_string()
            }),
        ));
    }

    Ok(RepoWriteLock { path: lock_path })
}

fn repo_write_lock_path(repo: &RepoHandle) -> Result<PathBuf, CommandError> {
    let parent = app_support_dir().join("locks");

    fs::create_dir_all(&parent).map_err(|error| {
        CommandError::with_details(
            "fs.write_failed",
            "Failed to create repository lock directory.",
            serde_json::json!({
                "path": parent.to_string_lossy(),
                "error": error.to_string()
            }),
        )
    })?;

    Ok(parent.join(format!("{}.write.lock", repo.id)))
}

fn app_support_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("c4lens")
    } else {
        home.join(".local").join("share").join("c4lens")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::repo_handle_from_path;

    use super::acquire_repo_write_lock;

    #[test]
    fn write_lock_blocks_second_holder_until_released() {
        let root = fresh_test_dir("write-lock-release");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let first_lock = acquire_repo_write_lock(&repo).expect("first lock");

        let error = acquire_repo_write_lock(&repo).expect_err("second lock blocked");
        assert_eq!(error.code, "repo.write_locked");

        drop(first_lock);
        let second_lock = acquire_repo_write_lock(&repo).expect("lock released");
        drop(second_lock);

        cleanup(root);
    }

    fn fresh_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("c4lens-core-{name}-{unique}"));
        fs::create_dir_all(&root).expect("create test root");
        root
    }

    fn cleanup(root: PathBuf) {
        fs::remove_dir_all(root).ok();
    }
}
