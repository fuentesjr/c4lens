use std::path::{Path, PathBuf};

use super::relative_posix_path;

#[derive(Clone, Debug, Default)]
pub(super) struct ParsedArtifacts {
    pub(super) symbols: Vec<ParsedSymbol>,
    pub(super) imports: Vec<ParsedImport>,
}

#[derive(Clone, Debug)]
pub(super) struct ParsedSymbol {
    pub(super) kind: &'static str,
    pub(super) name: String,
    pub(super) qualified_name: Option<String>,
    pub(super) start_line: i32,
    pub(super) start_column: i32,
    pub(super) end_line: i32,
    pub(super) end_column: i32,
}

#[derive(Clone, Debug)]
pub(super) struct ParsedImport {
    pub(super) target_module: String,
    pub(super) target_path: Option<String>,
    pub(super) kind: &'static str,
}

pub(super) fn parse_file_artifacts(
    repo_root: &Path,
    source_path: &str,
    text: &str,
) -> ParsedArtifacts {
    let mut artifacts = ParsedArtifacts::default();

    for (line_index, line) in text.lines().enumerate() {
        let line_no = (line_index + 1) as i32;
        let clean_line = strip_inline_comment(line);
        if clean_line.trim().is_empty() {
            continue;
        }

        extract_symbols(&mut artifacts.symbols, line_no, clean_line);
        extract_imports(&mut artifacts.imports, repo_root, source_path, clean_line);
    }

    artifacts
}

fn strip_inline_comment(line: &str) -> &str {
    if let Some(index) = line.find("//") {
        &line[..index]
    } else {
        line
    }
}

fn extract_symbols(symbols: &mut Vec<ParsedSymbol>, line_no: i32, line: &str) {
    let clean = line.trim_start();
    let tokens: Vec<&str> = clean.split_whitespace().collect();
    if let Some((name, kind, column)) =
        extract_named_construct(&tokens, line, ["fn", "struct", "enum", "trait"])
    {
        symbols.push(ParsedSymbol {
            kind,
            name: name.to_string(),
            qualified_name: None,
            start_line: line_no,
            start_column: column as i32,
            end_line: line_no,
            end_column: (column + name.len()) as i32,
        });
    }
}

fn extract_named_construct<'a>(
    tokens: &[&'a str],
    line: &'a str,
    keywords: impl IntoIterator<Item = &'a str>,
) -> Option<(&'a str, &'static str, usize)> {
    let mut keyword_index = None;
    let mut keyword = "";
    for k in keywords {
        if let Some(position) = tokens.iter().position(|token| *token == k) {
            keyword_index = Some(position);
            keyword = k;
            break;
        }
    }

    let position = keyword_index?;
    let candidate = tokens.get(position + 1)?;
    let name = candidate
        .split(['(', '<', '{', ':', '=', ';'])
        .next()
        .unwrap_or(candidate);
    if !is_identifier(name) || is_reserved_word(name) {
        return None;
    }

    let kind = match keyword {
        "fn" => "function",
        "def" => "function",
        "class" => "class",
        "module" => "module",
        "struct" => "struct",
        "enum" => "enum",
        "trait" => "interface",
        _ => "symbol",
    };

    let before = line.find(candidate).unwrap_or(0);
    Some((name, kind, before))
}

fn extract_imports(
    imports: &mut Vec<ParsedImport>,
    repo_root: &Path,
    source_path: &str,
    line: &str,
) {
    let clean = line.trim_start();
    if !clean.starts_with("use ") {
        return;
    }

    let target = clean
        .trim_start_matches("use ")
        .trim()
        .trim_end_matches(';');
    if target.is_empty() || target.starts_with('{') || target.starts_with('*') {
        return;
    }

    for normalized in expand_import_targets(target) {
        if normalized.is_empty() {
            continue;
        }

        let kind = if is_internal_import_path(&normalized) {
            "internal"
        } else {
            "external"
        };
        let target_path = if kind == "internal" {
            resolve_internal_import_target(repo_root, source_path, &normalized)
        } else {
            None
        };
        imports.push(ParsedImport {
            target_module: normalized,
            target_path,
            kind,
        });
    }
}

fn expand_import_targets(target: &str) -> Vec<String> {
    let normalized = if target.contains('{') {
        target.trim().to_string()
    } else {
        normalize_import_target(target)
    };
    if normalized.is_empty() {
        return Vec::new();
    }

    let Some((prefix, body)) = split_group_import(&normalized) else {
        return vec![normalized];
    };

    if !is_internal_import_path(prefix) || body.contains('{') || body.contains('}') {
        return vec![normalized];
    }

    let targets = body
        .split(',')
        .filter_map(|raw_item| {
            let item = normalize_import_target(raw_item);
            if item.is_empty() || item.starts_with('*') {
                return None;
            }

            if item == "self" {
                Some(prefix.trim_end_matches("::").to_string())
            } else {
                Some(format!("{prefix}{item}"))
            }
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        vec![normalized]
    } else {
        targets
    }
}

fn normalize_import_target(target: &str) -> String {
    let without_alias = target
        .trim()
        .split_once(" as ")
        .map_or_else(|| target.trim(), |(path, _alias)| path.trim());
    without_alias.trim_end_matches("::*").trim().to_string()
}

fn split_group_import(target: &str) -> Option<(&str, &str)> {
    let open_brace = target.find('{')?;
    if !target.ends_with('}') {
        return None;
    }

    let prefix = &target[..open_brace];
    if prefix.is_empty() {
        return None;
    }

    let body = &target[open_brace + 1..target.len() - 1];
    Some((prefix, body))
}

fn is_internal_import_path(module_path: &str) -> bool {
    module_path == "crate"
        || module_path.starts_with("crate::")
        || module_path == "self"
        || module_path.starts_with("self::")
        || module_path == "super"
        || module_path.starts_with("super::")
}

pub(super) fn resolve_internal_import_target(
    repo_root: &Path,
    source_path: &str,
    module_path: &str,
) -> Option<String> {
    if module_path.contains('{') || module_path.contains('*') {
        return None;
    }

    let module_root = module_root_for_import(repo_root, source_path, module_path)?;
    let segments = import_module_segments(module_path)?;
    for length in (1..=segments.len()).rev() {
        let module_segments = &segments[..length];
        for candidate in module_file_candidates(&module_root, module_segments) {
            if candidate.is_file() {
                return relative_posix_path(repo_root, &candidate);
            }
        }
    }

    None
}

fn module_root_for_import(
    repo_root: &Path,
    source_path: &str,
    module_path: &str,
) -> Option<PathBuf> {
    if module_path.starts_with("crate::") {
        return Some(repo_root.join("src"));
    }

    let source_parent = repo_root.join(source_path).parent()?.to_path_buf();
    if module_path.starts_with("self::") {
        return Some(source_parent);
    }

    if module_path.starts_with("super::") {
        return Some(source_parent.parent()?.to_path_buf());
    }

    None
}

fn import_module_segments(module_path: &str) -> Option<Vec<&str>> {
    let trimmed = module_path
        .strip_prefix("crate::")
        .or_else(|| module_path.strip_prefix("self::"))
        .or_else(|| module_path.strip_prefix("super::"))?;
    let segments = trimmed
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

fn module_file_candidates(module_root: &Path, segments: &[&str]) -> Vec<PathBuf> {
    let mut directory = module_root.to_path_buf();
    for segment in segments.iter().take(segments.len().saturating_sub(1)) {
        directory.push(segment);
    }

    let Some(last) = segments.last() else {
        return Vec::new();
    };

    vec![
        directory.join(format!("{last}.rs")),
        directory.join(last).join("mod.rs"),
    ]
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_reserved_word(text: &str) -> bool {
    matches!(
        text,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::parse_file_artifacts;

    #[test]
    fn parses_symbols_and_external_imports_without_sqlite() {
        let artifacts = parse_file_artifacts(
            Path::new("/repo"),
            "src/main.rs",
            r#"
use std::io;
pub fn main() {}
struct Cache;
enum Mode {}
trait Worker {}
"#,
        );

        let symbols = artifacts
            .symbols
            .iter()
            .map(|symbol| format!("{}:{}", symbol.kind, symbol.name))
            .collect::<Vec<_>>();
        let imports = artifacts
            .imports
            .iter()
            .map(|import| format!("{}:{}", import.kind, import.target_module))
            .collect::<Vec<_>>();

        assert_eq!(
            symbols,
            [
                "function:main",
                "struct:Cache",
                "enum:Mode",
                "interface:Worker"
            ]
        );
        assert_eq!(imports, ["external:std::io"]);
    }

    #[test]
    fn resolves_grouped_internal_imports_without_sqlite() {
        let root = fresh_test_dir("rust-parser-grouped-imports");
        write_file(&root, "src/cache.rs", "");
        write_file(&root, "src/jobs/worker.rs", "");

        let artifacts = parse_file_artifacts(
            &root,
            "src/main.rs",
            "use crate::{cache, jobs::worker as Worker};",
        );
        let imports = artifacts
            .imports
            .iter()
            .map(|import| {
                (
                    import.target_module.as_str(),
                    import.target_path.as_deref(),
                    import.kind,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            imports,
            [
                ("crate::cache", Some("src/cache.rs"), "internal"),
                (
                    "crate::jobs::worker",
                    Some("src/jobs/worker.rs"),
                    "internal"
                )
            ]
        );

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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn cleanup(root: PathBuf) {
        fs::remove_dir_all(root).ok();
    }
}
