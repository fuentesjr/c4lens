use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use ignore::{DirEntry, WalkBuilder};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{
    acquire_repo_write_lock, load_effective_model_from_repo_recovering_generated_overlay, CodeRef,
    CommandError, RepoHandle, ScanSummary, ValidationIssue, ValidationSeverity, ValidationStage,
};

mod rust;

const MIGRATION_VERSION: i64 = 1;
const MAX_SCANNABLE_FILE_BYTES: i64 = 2 * 1024 * 1024;
const SCAN_BINARY_PREFIX_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub force: bool,
    pub index_path: Option<PathBuf>,
}

pub fn scan_repo(repo: RepoHandle, options: ScanOptions) -> Result<ScanSummary, CommandError> {
    let _write_lock = acquire_repo_write_lock(&repo)?;
    let started = Instant::now();
    let index_path = options
        .index_path
        .unwrap_or_else(|| default_index_path(&repo));
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CommandError::with_details(
                "scan.failed",
                "Failed to create SQLite index directory.",
                serde_json::json!({ "path": parent.display().to_string(), "error": error.to_string() }),
            )
        })?;
    }

    let index_exclusions = index_exclusion_paths(&index_path)?;
    let connection = Connection::open(&index_path).map_err(sqlite_error)?;
    migrate_index(&connection)?;
    scan_repo_with_connection(connection, repo, options.force, started, &index_exclusions)
}

pub fn repo_scan_token(
    repo: &RepoHandle,
    index_path: Option<PathBuf>,
) -> Result<Option<String>, CommandError> {
    let index_path = index_path.unwrap_or_else(|| default_index_path(repo));
    if !index_path.is_file() {
        return Ok(None);
    }

    let connection = Connection::open(&index_path).map_err(sqlite_error)?;
    migrate_index(&connection)?;
    connection
        .query_row(
            "SELECT scan_token FROM repos WHERE id = ?1",
            params![repo.id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(|token| token.flatten())
        .map_err(sqlite_error)
}

pub fn get_element_code(
    repo: &RepoHandle,
    index_path: &Path,
    element_slug: &str,
) -> Result<Option<CodeRef>, CommandError> {
    let connection = Connection::open(index_path).map_err(sqlite_error)?;
    migrate_index(&connection)?;

    let mut statement = connection
        .prepare(
            r#"
SELECT files.path, files.lang
FROM element_sources
JOIN files ON files.id = element_sources.file_id
WHERE element_sources.repo_id = ?1
  AND element_sources.element_slug = ?2
  AND element_sources.source = 'authored_code_path'
ORDER BY element_sources.source_key
LIMIT 1
"#,
        )
        .map_err(sqlite_error)?;
    let row = statement
        .query_row(params![repo.id, element_slug], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .optional()
        .map_err(sqlite_error)?;

    let Some((relative_path, language)) = row else {
        return Ok(None);
    };

    let repo_root = PathBuf::from(&repo.root_path);
    let absolute_path = match resolve_repo_file(&repo_root, &relative_path) {
        Ok(path) => path,
        Err(error) if error.code == "repo.path_missing" => return Ok(None),
        Err(error) => return Err(error),
    };
    if !absolute_path.is_file() {
        return Ok(None);
    }
    let metadata = match fs::metadata(&absolute_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CommandError::with_details(
                "scan.failed",
                "Failed to inspect indexed source file.",
                serde_json::json!({ "path": relative_path, "error": error.to_string() }),
            ));
        }
    };
    if metadata.len() > MAX_SCANNABLE_FILE_BYTES as u64 {
        return Ok(Some(CodeRef {
            element_slug: element_slug.to_string(),
            path: relative_path.to_string(),
            absolute_path: absolute_path.to_string_lossy().to_string(),
            language,
            snippet: None,
        }));
    }

    let bytes = match fs::read(&absolute_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CommandError::with_details(
                "scan.failed",
                "Failed to read indexed source file.",
                serde_json::json!({ "path": relative_path, "error": error.to_string() }),
            ));
        }
    };
    let snippet = if has_nul_in_prefix(&bytes, SCAN_BINARY_PREFIX_BYTES)
        || std::str::from_utf8(&bytes).is_err()
    {
        None
    } else {
        snippet_from_utf8_bytes(&bytes)
    };

    Ok(Some(CodeRef {
        element_slug: element_slug.to_string(),
        path: relative_path.to_string(),
        absolute_path: absolute_path.to_string_lossy().to_string(),
        language,
        snippet,
    }))
}

#[derive(Debug, Clone)]
pub struct RepoImportEdge {
    pub from_file: String,
    pub to_file: String,
}

pub fn list_internal_crate_import_edges(
    repo: &RepoHandle,
    index_path: Option<&Path>,
) -> Result<Vec<RepoImportEdge>, CommandError> {
    let index_path = index_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_index_path(repo));
    if !index_path.is_file() {
        return Ok(Vec::new());
    }

    let connection = Connection::open(&index_path).map_err(sqlite_error)?;
    migrate_index(&connection)?;

    let mut statement = connection
        .prepare(
            r#"
SELECT source_files.path, target_files.path
FROM imports
JOIN files AS source_files ON source_files.id = imports.file_id
JOIN files AS target_files
  ON target_files.repo_id = source_files.repo_id
 AND target_files.path = imports.target_path
WHERE source_files.repo_id = ?1
  AND imports.kind = 'internal'
  AND (
    imports.target_module LIKE 'crate::%'
    OR imports.target_module LIKE 'self::%'
    OR imports.target_module LIKE 'super::%'
  )
  AND imports.target_path IS NOT NULL
ORDER BY source_files.path, target_files.path
"#,
        )
        .map_err(sqlite_error)?;

    let rows = statement
        .query_map(params![repo.id], |row| {
            Ok(RepoImportEdge {
                from_file: row.get::<_, String>(0)?,
                to_file: row.get::<_, String>(1)?,
            })
        })
        .map_err(sqlite_error)?;

    let mut edges = Vec::new();
    for row in rows {
        edges.push(row.map_err(sqlite_error)?);
    }

    Ok(edges)
}

pub fn default_index_path(repo: &RepoHandle) -> PathBuf {
    if let Ok(index_dir) = std::env::var("C4LENS_INDEX_DIR") {
        return PathBuf::from(index_dir).join(format!("{}.sqlite3", repo.id));
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let base = if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("c4lens")
            .join("indexes")
    } else {
        home.join(".local")
            .join("share")
            .join("c4lens")
            .join("indexes")
    };
    base.join(format!("{}.sqlite3", repo.id))
}

pub fn migrate_index(connection: &Connection) -> Result<(), CommandError> {
    connection
        .execute_batch(
            r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS repos (
  id TEXT PRIMARY KEY,
  root_path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  vcs TEXT NOT NULL DEFAULT 'none' CHECK (vcs IN ('git', 'none')),
  head_sha TEXT,
  scan_token TEXT,
  scanned_at TEXT
);

CREATE TABLE IF NOT EXISTS files (
  id INTEGER PRIMARY KEY,
  repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  lang TEXT,
  content_sha TEXT NOT NULL,
  mtime_ms INTEGER NOT NULL,
  size_bytes INTEGER NOT NULL,
  indexed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(repo_id, path)
);

CREATE INDEX IF NOT EXISTS idx_files_repo_lang ON files(repo_id, lang);
CREATE INDEX IF NOT EXISTS idx_files_repo_sha ON files(repo_id, content_sha);

CREATE TABLE IF NOT EXISTS symbols (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  name TEXT NOT NULL,
  qualified_name TEXT,
  start_line INTEGER NOT NULL,
  start_column INTEGER NOT NULL DEFAULT 0,
  end_line INTEGER NOT NULL,
  end_column INTEGER NOT NULL DEFAULT 0,
  parent_symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_qualified_name ON symbols(qualified_name);

CREATE TABLE IF NOT EXISTS imports (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  target_module TEXT NOT NULL,
  target_path TEXT,
  resolved_file_id INTEGER REFERENCES files(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK (kind IN ('internal', 'external', 'unknown'))
);

CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file_id);
CREATE INDEX IF NOT EXISTS idx_imports_resolved_file ON imports(resolved_file_id);
CREATE INDEX IF NOT EXISTS idx_imports_target_module ON imports(target_module);

CREATE TABLE IF NOT EXISTS element_sources (
  id INTEGER PRIMARY KEY,
  repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
  element_slug TEXT NOT NULL,
  file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
  symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
  path_glob TEXT,
  source_key TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('authored_code_path', 'generated_manifest', 'generated_component')),
  UNIQUE(repo_id, element_slug, source_key)
);

CREATE INDEX IF NOT EXISTS idx_element_sources_slug ON element_sources(repo_id, element_slug);

CREATE TABLE IF NOT EXISTS model_cache (
  repo_id TEXT PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
  source_sha TEXT NOT NULL,
  derived_json TEXT NOT NULL,
  cached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
"#,
        )
        .map_err(sqlite_error)?;

    connection
        .execute(
            "INSERT OR IGNORE INTO schema_migrations(version) VALUES (?1)",
            params![MIGRATION_VERSION],
        )
        .map_err(sqlite_error)?;

    Ok(())
}

fn scan_repo_with_connection(
    mut connection: Connection,
    repo: RepoHandle,
    force: bool,
    started: Instant,
    index_exclusions: &BTreeSet<PathBuf>,
) -> Result<ScanSummary, CommandError> {
    let repo_root = PathBuf::from(&repo.root_path);
    let mut warnings = Vec::new();
    let scanned_files = collect_scan_files(&repo_root, index_exclusions, &mut warnings)?;

    let transaction = connection.transaction().map_err(sqlite_error)?;
    let previous = if force {
        delete_indexed_repo_content(&transaction, &repo.id)?;
        BTreeMap::new()
    } else {
        previous_files_by_path(&transaction, &repo.id)?
    };

    upsert_repo(&transaction, &repo)?;

    let mut current_paths = BTreeSet::new();
    let mut file_ids_by_path = BTreeMap::new();
    let mut changed_files = 0;

    for scanned_file in &scanned_files {
        current_paths.insert(scanned_file.path.clone());
        let previous_file = previous.get(&scanned_file.path);
        let file_changed = previous_file
            .map(|previous| previous.content_sha != scanned_file.content_sha)
            .unwrap_or(true);

        let stale_ineligible_analysis = if let Some(previous_file) = previous_file {
            !scanned_file.should_extract_artifacts
                && !file_changed
                && file_has_analysis(&transaction, previous_file.id)?
        } else {
            false
        };
        let should_clear_analysis = file_changed || stale_ineligible_analysis;
        if should_clear_analysis {
            changed_files += 1;
            if let Some(previous_file) = previous_file {
                delete_file_analysis(&transaction, previous_file.id)?;
            }
        }

        upsert_file(&transaction, &repo.id, scanned_file)?;
        let file_id = file_id_for_path(&transaction, &repo.id, &scanned_file.path)?;
        file_ids_by_path.insert(scanned_file.path.clone(), file_id);

        if file_changed {
            let _ = extract_file_artifacts(&transaction, &repo_root, file_id, scanned_file)?;
        }
    }

    let mut deleted_files = 0;
    for (path, previous_file) in &previous {
        if !current_paths.contains(path) {
            deleted_files += 1;
            transaction
                .execute("DELETE FROM files WHERE id = ?1", params![previous_file.id])
                .map_err(sqlite_error)?;
        }
    }

    refresh_internal_import_resolutions(&transaction, &repo.id, &repo_root)?;
    rebuild_explicit_element_sources(&transaction, &repo, &repo_root, &file_ids_by_path)?;

    let scan_token = scan_token_for_files(scanned_files.iter());
    transaction
        .execute(
            r#"
UPDATE repos
SET scan_token = ?1,
    scanned_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = ?2
"#,
            params![scan_token, repo.id],
        )
        .map_err(sqlite_error)?;

    if changed_files > 0 || deleted_files > 0 {
        transaction
            .execute(
                "DELETE FROM model_cache WHERE repo_id = ?1",
                params![repo.id],
            )
            .map_err(sqlite_error)?;
    }

    let symbols = count_repo_symbols_for_repo(&transaction, &repo.id)? as usize;
    let imports = count_repo_imports_for_repo(&transaction, &repo.id)? as usize;

    transaction.commit().map_err(sqlite_error)?;

    Ok(ScanSummary {
        repo,
        scan_token,
        scanned_files: scanned_files.len(),
        changed_files,
        deleted_files,
        symbols,
        imports,
        duration_ms: started.elapsed().as_millis(),
        warnings,
    })
}

fn extract_file_artifacts(
    connection: &Connection,
    repo_root: &Path,
    file_id: i64,
    file: &ScannedFile,
) -> Result<(usize, usize), CommandError> {
    if !file.should_extract_artifacts {
        return Ok((0, 0));
    }

    let Some(lang) = file.lang.as_deref() else {
        return Ok((0, 0));
    };

    let absolute_path = repo_root.join(&file.path);
    let bytes = fs::read(&absolute_path).map_err(|error| {
        CommandError::with_details(
            "scan.failed",
            "Failed to re-read scanned file for symbol/import extraction.",
            serde_json::json!({ "path": file.path, "error": error.to_string() }),
        )
    })?;

    let Ok(text) = String::from_utf8(bytes) else {
        return Ok((0, 0));
    };
    let parsed = parse_file_artifacts(lang, repo_root, &file.path, &text);

    let mut inserted_symbols = 0usize;
    for symbol in &parsed.symbols {
        connection
            .execute(
                r#"
INSERT INTO symbols(file_id, kind, name, qualified_name, start_line, start_column, end_line, end_column)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
"#,
                params![
                    file_id,
                    symbol.kind,
                    symbol.name,
                    symbol.qualified_name.as_deref(),
                    symbol.start_line,
                    symbol.start_column,
                    symbol.end_line,
                    symbol.end_column,
                ],
            )
            .map_err(sqlite_error)?;
        inserted_symbols += 1;
    }

    for import in &parsed.imports {
        connection
            .execute(
                r#"
INSERT INTO imports(file_id, target_module, target_path, kind)
VALUES (?1, ?2, ?3, ?4)
"#,
                params![
                    file_id,
                    import.target_module,
                    import.target_path.as_deref(),
                    import.kind,
                ],
            )
            .map_err(sqlite_error)?;
    }

    Ok((inserted_symbols, parsed.imports.len()))
}

fn parse_file_artifacts(
    lang: &str,
    repo_root: &Path,
    source_path: &str,
    text: &str,
) -> rust::ParsedArtifacts {
    match lang {
        "rust" => rust::parse_file_artifacts(repo_root, source_path, text),
        _ => rust::ParsedArtifacts::default(),
    }
}

fn count_repo_symbols_for_repo(
    connection: &Connection,
    repo_id: &str,
) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM symbols WHERE file_id IN (SELECT id FROM files WHERE repo_id = ?1)",
            params![repo_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)
}

fn count_repo_imports_for_repo(
    connection: &Connection,
    repo_id: &str,
) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM imports WHERE file_id IN (SELECT id FROM files WHERE repo_id = ?1)",
            params![repo_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)
}

fn upsert_repo(connection: &Connection, repo: &RepoHandle) -> Result<(), CommandError> {
    let vcs = repo.vcs.as_deref().unwrap_or("none");
    connection
        .execute(
            r#"
INSERT INTO repos(id, root_path, name, vcs, head_sha)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(id) DO UPDATE SET
  root_path = excluded.root_path,
  name = excluded.name,
  vcs = excluded.vcs,
  head_sha = excluded.head_sha
"#,
            params![repo.id, repo.root_path, repo.name, vcs, repo.head_sha],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn previous_files_by_path(
    connection: &Connection,
    repo_id: &str,
) -> Result<BTreeMap<String, PreviousFile>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, path, content_sha FROM files WHERE repo_id = ?1")
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![repo_id], |row| {
            Ok(PreviousFile {
                id: row.get(0)?,
                path: row.get(1)?,
                content_sha: row.get(2)?,
            })
        })
        .map_err(sqlite_error)?;

    let mut output = BTreeMap::new();
    for row in rows {
        let previous = row.map_err(sqlite_error)?;
        output.insert(previous.path.clone(), previous);
    }
    Ok(output)
}

fn delete_indexed_repo_content(connection: &Connection, repo_id: &str) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM element_sources WHERE repo_id = ?1",
            params![repo_id],
        )
        .map_err(sqlite_error)?;
    connection
        .execute("DELETE FROM files WHERE repo_id = ?1", params![repo_id])
        .map_err(sqlite_error)?;
    connection
        .execute(
            "DELETE FROM model_cache WHERE repo_id = ?1",
            params![repo_id],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn delete_file_analysis(connection: &Connection, file_id: i64) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM element_sources WHERE file_id = ?1",
            params![file_id],
        )
        .map_err(sqlite_error)?;
    connection
        .execute("DELETE FROM imports WHERE file_id = ?1", params![file_id])
        .map_err(sqlite_error)?;
    connection
        .execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])
        .map_err(sqlite_error)?;
    Ok(())
}

fn file_has_analysis(connection: &Connection, file_id: i64) -> Result<bool, CommandError> {
    let symbol_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM symbols WHERE file_id = ?1",
            params![file_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    if symbol_count > 0 {
        return Ok(true);
    }

    let import_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM imports WHERE file_id = ?1",
            params![file_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    Ok(import_count > 0)
}

fn refresh_internal_import_resolutions(
    connection: &Connection,
    repo_id: &str,
    repo_root: &Path,
) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare(
            r#"
SELECT imports.id, files.path, imports.target_module
FROM imports
JOIN files ON files.id = imports.file_id
WHERE files.repo_id = ?1
  AND imports.kind = 'internal'
ORDER BY imports.id
"#,
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![repo_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(sqlite_error)?;

    let mut imports = Vec::new();
    for row in rows {
        imports.push(row.map_err(sqlite_error)?);
    }

    for (import_id, source_path, target_module) in imports {
        let target_path =
            rust::resolve_internal_import_target(repo_root, &source_path, &target_module);
        connection
            .execute(
                r#"
UPDATE imports
SET target_path = ?2,
    resolved_file_id = (
      SELECT id FROM files WHERE repo_id = ?3 AND path = ?2
    )
WHERE id = ?1
"#,
                params![import_id, target_path.as_deref(), repo_id],
            )
            .map_err(sqlite_error)?;
    }

    Ok(())
}

fn upsert_file(
    connection: &Connection,
    repo_id: &str,
    file: &ScannedFile,
) -> Result<(), CommandError> {
    connection
        .execute(
            r#"
INSERT INTO files(repo_id, path, lang, content_sha, mtime_ms, size_bytes)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
ON CONFLICT(repo_id, path) DO UPDATE SET
  lang = excluded.lang,
  content_sha = excluded.content_sha,
  mtime_ms = excluded.mtime_ms,
  size_bytes = excluded.size_bytes,
  indexed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
"#,
            params![
                repo_id,
                file.path,
                file.lang,
                file.content_sha,
                file.mtime_ms,
                file.size_bytes
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn file_id_for_path(
    connection: &Connection,
    repo_id: &str,
    path: &str,
) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT id FROM files WHERE repo_id = ?1 AND path = ?2",
            params![repo_id, path],
            |row| row.get(0),
        )
        .map_err(sqlite_error)
}

fn rebuild_explicit_element_sources(
    connection: &Connection,
    repo: &RepoHandle,
    repo_root: &Path,
    file_ids_by_path: &BTreeMap<String, i64>,
) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM element_sources WHERE repo_id = ?1 AND source = 'authored_code_path'",
            params![repo.id],
        )
        .map_err(sqlite_error)?;

    let Ok(model) = load_effective_model_from_repo_recovering_generated_overlay(repo.clone())
    else {
        return Ok(());
    };

    for element in model.elements_by_slug.values() {
        let Some(code_path) = element.base.code.as_deref() else {
            continue;
        };
        let Some(relative_path) = normalize_repo_relative_code_file(repo_root, code_path) else {
            continue;
        };
        let Some(file_id) = file_ids_by_path.get(&relative_path) else {
            continue;
        };
        let source_key = format!("path:{relative_path}");
        connection
            .execute(
                r#"
INSERT INTO element_sources(repo_id, element_slug, file_id, source_key, source)
VALUES (?1, ?2, ?3, ?4, 'authored_code_path')
ON CONFLICT(repo_id, element_slug, source_key) DO UPDATE SET
  file_id = excluded.file_id,
  symbol_id = NULL,
  path_glob = NULL,
  source = excluded.source
"#,
                params![repo.id, element.base.slug, file_id, source_key],
            )
            .map_err(sqlite_error)?;
    }

    Ok(())
}

fn collect_scan_files(
    repo_root: &Path,
    index_exclusions: &BTreeSet<PathBuf>,
    warnings: &mut Vec<ValidationIssue>,
) -> Result<Vec<ScannedFile>, CommandError> {
    let mut files = Vec::new();
    let mut builder = WalkBuilder::new(repo_root);
    builder
        .follow_links(false)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(should_scan_entry);

    for entry in builder.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                warnings.push(scan_warning(
                    "scan.walk_failed",
                    "Failed to walk a repository entry.",
                    None,
                ));
                continue;
            }
        };

        let Some(file_type) = entry.file_type() else {
            continue;
        };

        if entry.depth() == 0 || file_type.is_dir() {
            continue;
        }

        let path = entry.path();
        if index_exclusions.contains(path) {
            continue;
        }

        let metadata = if file_type.is_symlink() {
            let resolved = path.canonicalize().map_err(|error| {
                CommandError::with_details(
                    "path.invalid",
                    "Unable to resolve scanned symlink.",
                    serde_json::json!({ "path": path.display().to_string(), "error": error.to_string() }),
                )
            })?;
            if !resolved.starts_with(repo_root) {
                warnings.push(scan_warning(
                    "scan.path_outside_repo",
                    "Skipping symlinked file that resolves outside the repository.",
                    relative_posix_path(repo_root, path),
                ));
                continue;
            }
            fs::metadata(path)
        } else {
            fs::metadata(path)
        }
        .map_err(|error| {
            CommandError::with_details(
                "scan.failed",
                "Failed to inspect scanned file.",
                serde_json::json!({ "path": path.display().to_string(), "error": error.to_string() }),
            )
        })?;

        if !metadata.is_file() {
            continue;
        }

        let Some(relative_path) = relative_posix_path(repo_root, path) else {
            continue;
        };
        let mut should_extract_artifacts = true;
        if metadata.len() > MAX_SCANNABLE_FILE_BYTES as u64 {
            should_extract_artifacts = false;
            warnings.push(scan_warning(
                "scan.file_too_large",
                "Skipping symbol and import extraction for an oversized file.",
                Some(relative_path.clone()),
            ));
        }

        let contents = fs::read(path).map_err(|error| {
            CommandError::with_details(
                "scan.failed",
                "Failed to read scanned file.",
                serde_json::json!({ "path": relative_path, "error": error.to_string() }),
            )
        })?;
        if should_extract_artifacts && has_nul_in_prefix(&contents, SCAN_BINARY_PREFIX_BYTES) {
            should_extract_artifacts = false;
            warnings.push(scan_warning(
                "scan.binary_file_skipped",
                "Skipping symbol and import extraction for a binary file.",
                Some(relative_path.clone()),
            ));
        }
        if should_extract_artifacts && std::str::from_utf8(&contents).is_err() {
            should_extract_artifacts = false;
            warnings.push(scan_warning(
                "scan.invalid_utf8",
                "Skipping symbol and import extraction for an invalid UTF-8 file.",
                Some(relative_path.clone()),
            ));
        }

        files.push(ScannedFile {
            lang: language_for_path(&relative_path).map(str::to_string),
            path: relative_path,
            content_sha: sha256_hex(&contents),
            mtime_ms: metadata_mtime_ms(&metadata),
            size_bytes: metadata.len() as i64,
            should_extract_artifacts,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn index_exclusion_paths(index_path: &Path) -> Result<BTreeSet<PathBuf>, CommandError> {
    let absolute_index_path = if index_path.is_absolute() {
        index_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                CommandError::with_details(
                    "scan.failed",
                    "Failed to resolve current directory for SQLite index path.",
                    serde_json::json!({ "error": error.to_string() }),
                )
            })?
            .join(index_path)
    };
    let parent = absolute_index_path.parent().ok_or_else(|| {
        CommandError::new(
            "scan.failed",
            "SQLite index path must include a parent directory.",
        )
    })?;
    let file_name = absolute_index_path.file_name().ok_or_else(|| {
        CommandError::new("scan.failed", "SQLite index path must include a file name.")
    })?;
    let parent = parent.canonicalize().map_err(|error| {
        CommandError::with_details(
            "scan.failed",
            "Failed to resolve SQLite index directory.",
            serde_json::json!({ "path": parent.display().to_string(), "error": error.to_string() }),
        )
    })?;
    let db_path = parent.join(file_name);
    let file_name = file_name.to_string_lossy();

    let mut paths = BTreeSet::new();
    paths.insert(db_path.clone());
    paths.insert(parent.join(format!("{file_name}-wal")));
    paths.insert(parent.join(format!("{file_name}-shm")));
    Ok(paths)
}

fn should_scan_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0
        || !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
    {
        return true;
    }

    let Some(name) = entry.file_name().to_str() else {
        return true;
    };
    if matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | "tmp"
            | "log"
            | "coverage"
    ) {
        return false;
    }

    !(name == "bundle"
        && entry
            .path()
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|parent| parent.to_str())
            == Some("vendor"))
}

fn normalize_repo_relative_code_file(repo_root: &Path, code_path: &str) -> Option<String> {
    let path = Path::new(code_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return None;
    }

    let joined = repo_root.join(path);
    let resolved = joined.canonicalize().ok()?;
    if !resolved.starts_with(repo_root) || !resolved.is_file() {
        return None;
    }
    relative_posix_path(repo_root, &joined)
}

fn resolve_repo_file(repo_root: &Path, relative_path: &str) -> Result<PathBuf, CommandError> {
    let path = Path::new(relative_path);
    if path.is_absolute()
        || relative_path.contains('\\')
        || relative_path.contains('\0')
        || relative_path.split('/').any(|part| part.is_empty())
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Indexed source path is not a valid repository-relative file path.",
            serde_json::json!({ "path": relative_path }),
        ));
    }

    let joined = repo_root.join(path);
    let resolved = joined.canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.path_missing",
            "Indexed source file no longer exists.",
            serde_json::json!({ "path": relative_path, "error": error.to_string() }),
        )
    })?;
    if !resolved.starts_with(repo_root) {
        return Err(CommandError::with_details(
            "repo.path_denied",
            "Indexed source path resolves outside the repository.",
            serde_json::json!({ "path": relative_path }),
        ));
    }

    Ok(resolved)
}

fn snippet_from_utf8_bytes(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut output = String::new();
    for (index, line) in text.lines().take(200).enumerate() {
        if index > 0 {
            if output.len() + 1 > 16 * 1024 {
                break;
            }
            output.push('\n');
        }

        let remaining = (16 * 1024_usize).saturating_sub(output.len());
        if remaining == 0 {
            break;
        }
        if line.len() <= remaining {
            output.push_str(line);
        } else {
            let mut end = 0;
            for (byte_index, character) in line.char_indices() {
                let next = byte_index + character.len_utf8();
                if next > remaining {
                    break;
                }
                end = next;
            }
            output.push_str(&line[..end]);
            break;
        }
    }

    if text.ends_with('\n') && output.lines().count() < 200 && output.len() < 16 * 1024 {
        output.push('\n');
    }

    Some(output)
}

fn relative_posix_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

fn metadata_mtime_ms(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn scan_token_for_files<'a>(files: impl Iterator<Item = &'a ScannedFile>) -> String {
    let mut digest = Sha256::new();
    for file in files {
        digest.update(file.path.as_bytes());
        digest.update([0]);
        digest.update(file.content_sha.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn language_for_path(path: &str) -> Option<&'static str> {
    let extension = Path::new(path).extension()?.to_str()?;
    match extension {
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        "rb" => Some("ruby"),
        "py" => Some("python"),
        "go" => Some("go"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        "cs" => Some("csharp"),
        "php" => Some("php"),
        "sql" => Some("sql"),
        "yml" | "yaml" => Some("yaml"),
        "json" => Some("json"),
        "md" => Some("markdown"),
        _ => None,
    }
}

fn scan_warning(code: &str, message: &str, path: Option<String>) -> ValidationIssue {
    ValidationIssue {
        severity: ValidationSeverity::Warning,
        stage: ValidationStage::Scan,
        code: code.to_string(),
        message: message.to_string(),
        path,
        line: None,
        column: None,
    }
}

fn sqlite_error(error: rusqlite::Error) -> CommandError {
    CommandError::with_details(
        "scan.failed",
        "SQLite index operation failed.",
        serde_json::json!({ "error": error.to_string() }),
    )
}

struct PreviousFile {
    id: i64,
    path: String,
    content_sha: String,
}

struct ScannedFile {
    path: String,
    lang: Option<String>,
    content_sha: String,
    mtime_ms: i64,
    size_bytes: i64,
    should_extract_artifacts: bool,
}

fn has_nul_in_prefix(bytes: &[u8], max: usize) -> bool {
    bytes.iter().take(max).any(|byte| *byte == b'\0')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::{params, Connection};

    use crate::{
        acquire_repo_write_lock, get_element_code, list_internal_crate_import_edges,
        repo_handle_from_path, scan_repo, ScanOptions,
    };

    use super::migrate_index;

    #[test]
    fn migrations_create_required_tables_and_indexes() {
        let root = fresh_test_dir("migrations");
        let db_path = root.join("index.sqlite3");
        let connection = Connection::open(&db_path).expect("open db");

        migrate_index(&connection).expect("migrate index");

        for table in [
            "schema_migrations",
            "repos",
            "files",
            "symbols",
            "imports",
            "element_sources",
            "model_cache",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |row| row.get(0),
                )
                .expect("query table");
            assert_eq!(count, 1, "{table} table should exist");
        }

        for index in [
            "idx_files_repo_lang",
            "idx_files_repo_sha",
            "idx_element_sources_slug",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    params![index],
                    |row| row.get(0),
                )
                .expect("query index");
            assert_eq!(count, 1, "{index} should exist");
        }

        cleanup(root);
    }

    #[test]
    fn scan_indexes_file_rows_and_explicit_element_sources() {
        let root = fresh_test_dir("scan-file-rows");
        let index_root = fresh_test_dir("scan-file-rows-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
    code: src/main.rs
  huge:
    name: Huge
    code: src/huge.rs
"#,
        );
        write_file(&root, "src/main.rs", "use std::io;\nfn main() {}\n");
        write_file(
            &root,
            "src/lib.rs",
            "pub fn run<T>() {}\npub struct Cache<K, V>;\nimpl Trait for Foo {}\n",
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let summary = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        assert_eq!(summary.scanned_files, 3);
        assert_eq!(summary.changed_files, 3);
        assert_eq!(summary.deleted_files, 0);
        assert_eq!(summary.symbols, 3);
        assert_eq!(summary.imports, 1);
        assert!(summary.scan_token.len() >= 32);

        let connection = Connection::open(&db_path).expect("open db");
        let file_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM files WHERE repo_id = ?1",
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query files");
        assert_eq!(file_count, 3);

        let source_key: String = connection
            .query_row(
                "SELECT source_key FROM element_sources WHERE repo_id = ?1 AND element_slug = 'app'",
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query source");
        assert_eq!(source_key, "path:src/main.rs");

        let mut symbol_statement = connection
            .prepare(
                r#"
SELECT symbols.kind || ':' || symbols.name
FROM symbols
JOIN files ON files.id = symbols.file_id
WHERE files.repo_id = ?1
ORDER BY lower(symbols.name)
"#,
            )
            .expect("prepare symbol query");
        let symbol_rows = symbol_statement
            .query_map(params![repo.id], |row| row.get::<_, String>(0))
            .expect("query symbols");
        let symbols = symbol_rows
            .collect::<Result<Vec<_>, _>>()
            .expect("collect symbols");
        assert_eq!(symbols, ["struct:Cache", "function:main", "function:run"]);
        drop(symbol_statement);

        let import_target: String = connection
            .query_row(
                r#"
SELECT imports.target_module
FROM imports
JOIN files ON files.id = imports.file_id
WHERE files.repo_id = ?1
"#,
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query import");
        assert_eq!(import_target, "std::io");

        let rescan = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan repo");
        assert_eq!(rescan.scanned_files, 3);
        assert_eq!(rescan.changed_files, 0);
        assert_eq!(rescan.deleted_files, 0);

        write_file(&root, "src/lib.rs", "pub struct Runner;\n");
        let changed_rescan = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan after source change");
        assert_eq!(changed_rescan.scanned_files, 3);
        assert_eq!(changed_rescan.changed_files, 1);
        assert_eq!(changed_rescan.deleted_files, 0);
        assert_eq!(changed_rescan.symbols, 2);

        let mut symbol_statement = connection
            .prepare(
                r#"
SELECT symbols.kind || ':' || symbols.name
FROM symbols
JOIN files ON files.id = symbols.file_id
WHERE files.repo_id = ?1
ORDER BY lower(symbols.name)
"#,
            )
            .expect("prepare changed symbol query");
        let symbol_rows = symbol_statement
            .query_map(params![repo.id], |row| row.get::<_, String>(0))
            .expect("query changed symbols");
        let symbols = symbol_rows
            .collect::<Result<Vec<_>, _>>()
            .expect("collect changed symbols");
        assert_eq!(symbols, ["function:main", "struct:Runner"]);
        drop(symbol_statement);

        fs::remove_file(root.join("src/lib.rs")).expect("remove file");
        let repo = repo_handle_from_path(&root).expect("repo handle");
        let delete_rescan = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan after delete");
        assert_eq!(delete_rescan.scanned_files, 2);
        assert_eq!(delete_rescan.changed_files, 0);
        assert_eq!(delete_rescan.deleted_files, 1);

        let file_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM files WHERE repo_id = ?1",
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query files after delete");
        assert_eq!(file_count, 2);

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_resolves_simple_rust_crate_import_edges_to_repo_files() {
        let root = fresh_test_dir("scan-rust-import-edges");
        let index_root = fresh_test_dir("scan-rust-import-edges-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
"#,
        );
        write_file(
            &root,
            "src/api/mod.rs",
            "use crate::domain::Thing;\npub fn handle() {}\n",
        );
        write_file(&root, "src/domain/mod.rs", "pub struct Thing;\n");
        write_file(
            &root,
            "src/unresolved/mod.rs",
            "use crate::missing::Thing;\n",
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        let edges =
            list_internal_crate_import_edges(&repo, Some(&db_path)).expect("list import edges");

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_file, "src/api/mod.rs");
        assert_eq!(edges[0].to_file, "src/domain/mod.rs");

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_resolves_grouped_rust_crate_import_edges_to_repo_files() {
        let root = fresh_test_dir("scan-rust-grouped-import-edges");
        let index_root = fresh_test_dir("scan-rust-grouped-import-edges-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
"#,
        );
        write_file(
            &root,
            "src/api/mod.rs",
            "use crate::{domain::Thing, jobs::Job, missing::Ghost};\npub fn handle() {}\n",
        );
        write_file(&root, "src/domain/mod.rs", "pub struct Thing;\n");
        write_file(&root, "src/jobs/mod.rs", "pub struct Job;\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        let edges =
            list_internal_crate_import_edges(&repo, Some(&db_path)).expect("list import edges");

        let edge_pairs = edges
            .iter()
            .map(|edge| (edge.from_file.as_str(), edge.to_file.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            edge_pairs,
            [
                ("src/api/mod.rs", "src/domain/mod.rs"),
                ("src/api/mod.rs", "src/jobs/mod.rs"),
            ]
        );

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_resolves_rust_import_edges_when_target_file_is_added_later() {
        let root = fresh_test_dir("scan-rust-import-edges-incremental-target");
        let index_root = fresh_test_dir("scan-rust-import-edges-incremental-target-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
"#,
        );
        write_file(
            &root,
            "src/api/mod.rs",
            "use crate::domain::Thing;\npub fn handle() {}\n",
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan unresolved importer");
        let unresolved_edges =
            list_internal_crate_import_edges(&repo, Some(&db_path)).expect("list unresolved edges");
        assert!(unresolved_edges.is_empty());

        write_file(&root, "src/domain/mod.rs", "pub struct Thing;\n");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan after adding target");

        let edges =
            list_internal_crate_import_edges(&repo, Some(&db_path)).expect("list resolved edges");

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_file, "src/api/mod.rs");
        assert_eq!(edges[0].to_file, "src/domain/mod.rs");

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_clears_rust_import_edges_when_target_file_is_deleted() {
        let root = fresh_test_dir("scan-rust-import-edges-deleted-target");
        let index_root = fresh_test_dir("scan-rust-import-edges-deleted-target-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
"#,
        );
        write_file(
            &root,
            "src/api/mod.rs",
            "use crate::domain::Thing;\npub fn handle() {}\n",
        );
        write_file(&root, "src/domain/mod.rs", "pub struct Thing;\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan resolved import");
        let resolved_edges =
            list_internal_crate_import_edges(&repo, Some(&db_path)).expect("list resolved edges");
        assert_eq!(resolved_edges.len(), 1);

        fs::remove_file(root.join("src/domain/mod.rs")).expect("remove target file");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan after deleting target");

        let edges =
            list_internal_crate_import_edges(&repo, Some(&db_path)).expect("list cleared edges");

        assert!(edges.is_empty());

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_rejects_when_write_lock_is_held() {
        let root = fresh_test_dir("scan-write-locked");
        let index_root = fresh_test_dir("scan-write-locked-index");
        let db_path = index_root.join("index.sqlite3");
        let repo = repo_handle_from_path(&root).expect("repo handle");

        let _lock = acquire_repo_write_lock(&repo).expect("acquire external lock");
        let error = scan_repo(
            repo,
            ScanOptions {
                force: false,
                index_path: Some(db_path),
            },
        )
        .expect_err("scan blocked by lock");

        assert_eq!(error.code, "repo.write_locked");
        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_respects_gitignore_and_always_excluded_directories() {
        let root = fresh_test_dir("scan-ignore-rules");
        let index_root = fresh_test_dir("scan-ignore-rules-index");
        let db_path = index_root.join("index.sqlite3");
        fs::create_dir(root.join(".git")).expect("create git dir");
        write_file(&root, ".gitignore", "*.log\n");
        write_file(&root, "src/main.rs", "fn main() {}\n");
        write_file(&root, "debug.log", "ignored\n");
        write_file(&root, "node_modules/pkg/index.js", "ignored\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let summary = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        assert_eq!(summary.scanned_files, 2);

        let connection = Connection::open(&db_path).expect("open db");
        let ignored_count: i64 = connection
            .query_row(
                r#"
SELECT COUNT(*) FROM files
WHERE repo_id = ?1 AND path IN ('debug.log', 'node_modules/pkg/index.js')
"#,
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query ignored files");
        assert_eq!(ignored_count, 0);

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_excludes_custom_index_path_inside_repo() {
        let root = fresh_test_dir("scan-in-repo-index");
        let db_path = root.join(".c4lens-index/index.sqlite3");
        write_file(&root, "c4/model.yml", "name: Scan Repo\n");
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let first_scan = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");
        assert_eq!(first_scan.scanned_files, 2);
        assert_eq!(first_scan.changed_files, 2);

        let second_scan = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan repo");
        assert_eq!(second_scan.scanned_files, 2);
        assert_eq!(second_scan.changed_files, 0);
        assert_eq!(second_scan.scan_token, first_scan.scan_token);

        let connection = Connection::open(&db_path).expect("open db");
        let index_file_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM files WHERE repo_id = ?1 AND path LIKE '.c4lens-index/%'",
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query indexed index files");
        assert_eq!(index_file_count, 0);

        cleanup(root);
    }

    #[test]
    fn scan_skips_oversized_file_from_symbol_and_import_extraction() {
        let root = fresh_test_dir("scan-skip-oversized");
        let index_root = fresh_test_dir("scan-skip-oversized-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
    code: src/main.rs
  huge:
    name: Huge
    code: src/huge.rs
"#,
        );
        write_file(&root, "src/main.rs", "use std::io;\nfn main() {}\n");
        let oversized = "a".repeat((super::MAX_SCANNABLE_FILE_BYTES + 1) as usize);
        write_file(&root, "src/huge.rs", &oversized);

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let summary = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        assert_eq!(summary.scanned_files, 3);
        assert_eq!(summary.changed_files, 3);
        assert_eq!(summary.symbols, 1);
        assert_eq!(summary.imports, 1);
        assert!(summary
            .warnings
            .iter()
            .any(|issue| issue.code == "scan.file_too_large"));

        let connection = Connection::open(&db_path).expect("open db");
        let huge_symbol_count: i64 = connection
            .query_row(
                r#"
SELECT COUNT(*)
FROM symbols
JOIN files ON files.id = symbols.file_id
WHERE files.repo_id = ?1 AND files.path = 'src/huge.rs'
"#,
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query huge file symbols");
        assert_eq!(huge_symbol_count, 0);

        let code = get_element_code(&repo, &db_path, "huge")
            .expect("resolve huge code")
            .expect("huge code ref");
        assert_eq!(code.path, "src/huge.rs");
        assert!(code.snippet.is_none());

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_binary_files_emit_warnings_and_skip_analysis() {
        let root = fresh_test_dir("scan-skip-binary");
        let index_root = fresh_test_dir("scan-skip-binary-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
    code: src/main.rs
  binary:
    name: Binary
    code: src/binary.rs
"#,
        );
        write_file(&root, "src/main.rs", "use std::io;\nfn main() {}\n");
        write_file_bytes(&root, "src/binary.rs", b"fn binary()\0{}\nuse std::io;\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let summary = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        assert_eq!(summary.scanned_files, 3);
        assert_eq!(summary.changed_files, 3);
        assert_eq!(summary.symbols, 1);
        assert_eq!(summary.imports, 1);
        assert!(summary
            .warnings
            .iter()
            .any(|issue| issue.code == "scan.binary_file_skipped"));

        let connection = Connection::open(&db_path).expect("open db");
        let binary_symbol_count: i64 = connection
            .query_row(
                r#"
SELECT COUNT(*)
FROM symbols
JOIN files ON files.id = symbols.file_id
WHERE files.repo_id = ?1 AND files.path = 'src/binary.rs'
"#,
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query binary file symbols");
        assert_eq!(binary_symbol_count, 0);

        let binary_file_id: i64 = connection
            .query_row(
                "SELECT id FROM files WHERE repo_id = ?1 AND path = 'src/binary.rs'",
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query binary file id");
        connection
            .execute(
                r#"
INSERT INTO symbols(file_id, kind, name, qualified_name, start_line, start_column, end_line, end_column)
VALUES (?1, 'function', 'stale_binary', NULL, 1, 0, 1, 12)
"#,
                params![binary_file_id],
            )
            .expect("seed stale binary symbol");
        connection
            .execute(
                r#"
INSERT INTO imports(file_id, target_module, target_path, kind)
VALUES (?1, 'stale::binary', NULL, 'external')
"#,
                params![binary_file_id],
            )
            .expect("seed stale binary import");

        let cleanup_rescan = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan binary stale analysis");
        assert_eq!(cleanup_rescan.changed_files, 1);
        assert_eq!(cleanup_rescan.symbols, 1);
        assert_eq!(cleanup_rescan.imports, 1);

        let stable_rescan = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan binary after cleanup");
        assert_eq!(stable_rescan.changed_files, 0);
        assert_eq!(stable_rescan.symbols, 1);
        assert_eq!(stable_rescan.imports, 1);

        let code = get_element_code(&repo, &db_path, "binary")
            .expect("resolve binary code")
            .expect("binary code ref");
        assert_eq!(code.path, "src/binary.rs");
        assert!(code.snippet.is_none());

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn scan_invalid_utf8_files_emit_warnings_and_skip_analysis() {
        let root = fresh_test_dir("scan-skip-invalid-utf8");
        let index_root = fresh_test_dir("scan-skip-invalid-utf8-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Scan Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        write_file(&root, "src/main.rs", "use std::io;\nfn main() {}\n");
        write_file_bytes(
            &root,
            "src/invalid.rs",
            &[0x66, 0x6e, 0x20, 0x62, 0xa, 0xff, 0x61, 0x64],
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let summary = scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        assert_eq!(summary.scanned_files, 3);
        assert_eq!(summary.changed_files, 3);
        assert_eq!(summary.symbols, 1);
        assert_eq!(summary.imports, 1);
        assert!(summary
            .warnings
            .iter()
            .any(|issue| issue.code == "scan.invalid_utf8"));

        let connection = Connection::open(&db_path).expect("open db");
        let invalid_symbol_count: i64 = connection
            .query_row(
                r#"
SELECT COUNT(*)
FROM symbols
JOIN files ON files.id = symbols.file_id
WHERE files.repo_id = ?1 AND files.path = 'src/invalid.rs'
"#,
                params![repo.id],
                |row| row.get(0),
            )
            .expect("query invalid utf8 file symbols");
        assert_eq!(invalid_symbol_count, 0);

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn element_code_resolves_indexed_file_source_with_capped_snippet() {
        let root = fresh_test_dir("code-ref-file-source");
        let index_root = fresh_test_dir("code-ref-file-source-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Code Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        write_file(
            &root,
            "src/main.rs",
            "fn main() {\n    println!(\"hello\");\n}\n",
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        let code = get_element_code(&repo, &db_path, "app")
            .expect("resolve code")
            .expect("code ref");

        assert_eq!(code.element_slug, "app");
        assert_eq!(code.path, "src/main.rs");
        assert_eq!(code.language.as_deref(), Some("rust"));
        assert_eq!(
            code.absolute_path,
            fs::canonicalize(root.join("src/main.rs"))
                .expect("canonical source")
                .to_string_lossy()
        );
        assert_eq!(
            code.snippet.as_deref(),
            Some("fn main() {\n    println!(\"hello\");\n}\n")
        );

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn element_code_returns_none_after_source_file_is_deleted_and_rescanned() {
        let root = fresh_test_dir("code-ref-deleted-source");
        let index_root = fresh_test_dir("code-ref-deleted-source-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Code Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");
        fs::remove_file(root.join("src/main.rs")).expect("remove source");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("rescan repo");

        let code = get_element_code(&repo, &db_path, "app").expect("resolve code");

        assert!(code.is_none());

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn element_code_returns_none_for_stale_deleted_source_file() {
        let root = fresh_test_dir("code-ref-stale-deleted-source");
        let index_root = fresh_test_dir("code-ref-stale-deleted-source-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Code Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        write_file(&root, "src/main.rs", "fn main() {}\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");
        fs::remove_file(root.join("src/main.rs")).expect("remove source");

        let code = get_element_code(&repo, &db_path, "app").expect("resolve code");

        assert!(code.is_none());

        cleanup(index_root);
        cleanup(root);
    }

    #[test]
    fn element_code_caps_snippet_to_200_lines_and_16_kib() {
        let root = fresh_test_dir("code-ref-snippet-cap");
        let index_root = fresh_test_dir("code-ref-snippet-cap-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Code Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        let source = (0..260)
            .map(|index| format!("println!(\"line {index:03}\");"))
            .collect::<Vec<_>>()
            .join("\n");
        write_file(&root, "src/main.rs", &source);

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        let snippet = get_element_code(&repo, &db_path, "app")
            .expect("resolve code")
            .expect("code ref")
            .snippet
            .expect("snippet");

        assert_eq!(snippet.lines().count(), 200);
        assert!(snippet.len() <= 16 * 1024);
        assert!(snippet.contains("line 199"));
        assert!(!snippet.contains("line 200"));

        cleanup(index_root);
        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn element_code_does_not_read_symlinked_file_outside_repo() {
        use std::os::unix::fs::symlink;

        let root = fresh_test_dir("code-ref-outside-symlink");
        let outside = fresh_test_dir("code-ref-outside-target");
        let index_root = fresh_test_dir("code-ref-outside-symlink-index");
        let db_path = index_root.join("index.sqlite3");
        write_file(
            &root,
            "c4/model.yml",
            r#"
name: Code Repo
systems:
  app:
    name: App
    code: src/main.rs
"#,
        );
        write_file(&outside, "main.rs", "fn outside() {}\n");
        fs::create_dir_all(root.join("src")).expect("create src");
        symlink(outside.join("main.rs"), root.join("src/main.rs")).expect("create symlink");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        scan_repo(
            repo.clone(),
            ScanOptions {
                force: false,
                index_path: Some(db_path.clone()),
            },
        )
        .expect("scan repo");

        let code = get_element_code(&repo, &db_path, "app").expect("resolve code");

        assert!(code.is_none());

        cleanup(index_root);
        cleanup(outside);
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

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write file");
    }

    fn write_file_bytes(root: &Path, relative_path: &str, contents: &[u8]) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write file bytes");
    }

    fn cleanup(root: PathBuf) {
        fs::remove_dir_all(root).ok();
    }
}
