use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoHandle {
    pub id: String,
    pub root_path: String,
    pub name: String,
    pub vcs: Option<String>,
    pub head_sha: Option<String>,
}

pub type Slug = String;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lifecycle {
    #[default]
    Live,
    Planned,
    Deprecated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    Actor,
    System,
    Container,
    Component,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerKind {
    #[default]
    Service,
    App,
    Store,
    Queue,
    Worker,
    Job,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    #[default]
    Authored,
    Generated,
    Merged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStage {
    Parse,
    Schema,
    Semantic,
    Scan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub stage: ValidationStage,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_sha: Option<String>,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub const fn ok() -> Self {
        Self {
            ok: true,
            source_sha: None,
            issues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub repo: RepoHandle,
    pub scan_token: String,
    pub scanned_files: usize,
    pub changed_files: usize,
    pub deleted_files: usize,
    pub symbols: usize,
    pub imports: usize,
    pub duration_ms: u128,
    pub warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRef {
    pub element_slug: Slug,
    pub path: String,
    pub absolute_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub from: Slug,
    pub to: Slug,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech: Option<String>,
    #[serde(default)]
    pub status: Lifecycle,
    #[serde(default)]
    pub generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseElement {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub slug: Slug,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech: Option<String>,
    #[serde(default)]
    pub status: Lifecycle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default)]
    pub generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    #[serde(flatten)]
    pub base: BaseElement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    #[serde(flatten)]
    pub base: BaseElement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    #[serde(flatten)]
    pub base: BaseElement,
    #[serde(default)]
    pub kind: ContainerKind,
    #[serde(default)]
    pub components: BTreeMap<Slug, Component>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System {
    #[serde(flatten)]
    pub base: BaseElement,
    #[serde(default)]
    pub external: bool,
    #[serde(default)]
    pub containers: BTreeMap<Slug, Container>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub actors: BTreeMap<Slug, Actor>,
    #[serde(default)]
    pub systems: BTreeMap<Slug, System>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementNode {
    #[serde(flatten)]
    pub base: BaseElement,
    #[serde(rename = "type")]
    pub element_type: ElementType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_slug: Option<Slug>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_slug: Option<Slug>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_slug: Option<Slug>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ContainerKind>,
    #[serde(default)]
    pub source: SourceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveModel {
    pub repo: RepoHandle,
    pub source_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_path: Option<String>,
    pub model: Model,
    pub elements_by_slug: BTreeMap<Slug, ElementNode>,
    pub relationships: Vec<Relationship>,
    pub validation: ValidationReport,
}

pub fn repo_handle_from_path(path: impl AsRef<Path>) -> Result<RepoHandle, CommandError> {
    let path = path.as_ref();
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        CommandError::with_details(
            "repo.path_missing",
            format!("Unable to open repo path {}.", path.display()),
            serde_json::json!({ "error": error.to_string() }),
        )
    })?;

    if !canonical.is_dir() {
        return Err(CommandError::new(
            "repo.path_not_directory",
            format!("Repo path is not a directory: {}", canonical.display()),
        ));
    }

    let canonical_str = canonical.to_string_lossy().to_string();
    let id = stable_repo_id_from_path(&canonical);
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo")
        .to_string();

    Ok(RepoHandle {
        id,
        root_path: canonical_str,
        name,
        vcs: None,
        head_sha: None,
    })
}

fn stable_repo_id_from_path(path: &Path) -> String {
    let mut digest = 0xcbf29ce484222325_u64;
    for byte in path.to_string_lossy().as_bytes() {
        digest ^= u64::from(*byte);
        digest = digest.wrapping_mul(0x100000001b3);
    }

    format!("repo-{digest:016x}")
}

#[cfg(test)]
mod tests {
    use super::repo_handle_from_path;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn repo_handle_accepts_existing_directory() {
        let root = fresh_test_dir("existing-directory");
        let repo_dir = root.join("repo");
        fs::create_dir(&repo_dir).expect("create repo dir");

        let repo = repo_handle_from_path(&repo_dir).expect("repo handle");

        assert_eq!(
            repo.root_path,
            fs::canonicalize(&repo_dir).unwrap().to_string_lossy()
        );
        assert_eq!(repo.name, "repo");
        assert!(repo.id.starts_with("repo-"));
        assert_eq!(repo.vcs, None);
        assert_eq!(repo.head_sha, None);

        cleanup(root);
    }

    #[test]
    fn repo_handle_rejects_missing_path() {
        let root = fresh_test_dir("missing-path");
        let missing = root.join("missing");

        let error = repo_handle_from_path(&missing).expect_err("missing path should fail");

        assert_eq!(error.code, "repo.path_missing");

        cleanup(root);
    }

    #[test]
    fn repo_handle_rejects_file_path() {
        let root = fresh_test_dir("file-path");
        let file_path = root.join("model.yml");
        fs::write(&file_path, "name: sample\n").expect("write file");

        let error = repo_handle_from_path(&file_path).expect_err("file path should fail");

        assert_eq!(error.code, "repo.path_not_directory");

        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn repo_handle_canonicalizes_symlinked_directory() {
        use std::os::unix::fs::symlink;

        let root = fresh_test_dir("symlinked-directory");
        let target = root.join("target");
        let link = root.join("repo-link");
        fs::create_dir(&target).expect("create target dir");
        symlink(&target, &link).expect("create symlink");

        let repo = repo_handle_from_path(&link).expect("repo handle");

        assert_eq!(
            repo.root_path,
            fs::canonicalize(&target).unwrap().to_string_lossy()
        );
        assert_eq!(repo.name, "target");

        cleanup(root);
    }

    #[test]
    fn repo_handle_id_hash_has_a_stable_contract() {
        let id = super::stable_repo_id_from_path(std::path::Path::new("/tmp/c4lens-demo"));

        assert_eq!(id, "repo-de397e374102b795");
    }

    fn fresh_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("c4lens-core-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir(&path).expect("create test dir");
        path
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
