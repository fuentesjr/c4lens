use std::fs;
use std::path::Path;

use super::artifacts::{ParsedArtifacts, ParsedImport, ParsedSymbol};
use super::relative_posix_path;

pub(super) fn parse_file_artifacts(
    repo_root: &Path,
    source_path: &str,
    text: &str,
) -> ParsedArtifacts {
    let mut artifacts = ParsedArtifacts::default();
    let module_name = go_module_name(repo_root);
    let mut in_import_block = false;
    let mut in_const_block = false;

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_no = (line_index + 1) as i32;
        let line = strip_line_comment(raw_line);
        let clean = line.trim_start();
        if clean.is_empty() {
            continue;
        }

        if in_import_block {
            if clean.starts_with(')') {
                in_import_block = false;
                continue;
            }
            if let Some(target) = quoted_import(clean) {
                push_import(
                    &mut artifacts.imports,
                    repo_root,
                    source_path,
                    module_name.as_deref(),
                    target,
                );
            }
            continue;
        }

        if in_const_block {
            if clean.starts_with(')') {
                in_const_block = false;
                continue;
            }
            if let Some(name) = parse_identifier_at_start(clean) {
                if is_exported_identifier(name) {
                    push_symbol(&mut artifacts.symbols, "constant", name, line_no, line);
                }
            }
            continue;
        }

        if clean == "import (" {
            in_import_block = true;
            continue;
        }
        if let Some(rest) = clean.strip_prefix("import ") {
            if let Some(target) = quoted_import(rest.trim_start()) {
                push_import(
                    &mut artifacts.imports,
                    repo_root,
                    source_path,
                    module_name.as_deref(),
                    target,
                );
            }
            continue;
        }

        if clean == "const (" {
            in_const_block = true;
            continue;
        }
        if let Some(rest) = clean.strip_prefix("const ") {
            if let Some(name) = parse_identifier_at_start(rest.trim_start()) {
                push_symbol(&mut artifacts.symbols, "constant", name, line_no, line);
            }
            continue;
        }

        if let Some((name, kind)) = parse_type_symbol(clean) {
            push_symbol(&mut artifacts.symbols, kind, name, line_no, line);
            continue;
        }

        if let Some(name) = parse_func_symbol(clean) {
            let kind = if clean.starts_with("func (") {
                "method"
            } else {
                "function"
            };
            push_symbol(&mut artifacts.symbols, kind, name, line_no, line);
        }
    }

    artifacts
}

fn strip_line_comment(line: &str) -> &str {
    line.find("//").map_or(line, |index| &line[..index])
}

fn parse_type_symbol(line: &str) -> Option<(&str, &'static str)> {
    let rest = line.strip_prefix("type ")?.trim_start();
    let name = parse_identifier_at_start(rest)?;
    let after_name = rest[name.len()..].trim_start();
    if after_name.starts_with("struct") {
        Some((name, "struct"))
    } else if after_name.starts_with("interface") {
        Some((name, "interface"))
    } else {
        None
    }
}

fn parse_func_symbol(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("func ")?.trim_start();
    let after_receiver = if rest.starts_with('(') {
        let close = rest.find(')')?;
        rest[close + 1..].trim_start()
    } else {
        rest
    };
    let name = parse_identifier_at_start(after_receiver)?;
    let after_name = after_receiver[name.len()..].trim_start();
    if after_name.starts_with('(') {
        Some(name)
    } else {
        None
    }
}

fn push_symbol(
    symbols: &mut Vec<ParsedSymbol>,
    kind: &'static str,
    name: &str,
    line_no: i32,
    line: &str,
) {
    if !is_identifier(name) || is_reserved_word(name) {
        return;
    }

    let column = line.find(name).unwrap_or(0) as i32;
    symbols.push(ParsedSymbol {
        kind,
        name: name.to_string(),
        qualified_name: None,
        start_line: line_no,
        start_column: column,
        end_line: line_no,
        end_column: column + name.len() as i32,
    });
}

fn push_import(
    imports: &mut Vec<ParsedImport>,
    repo_root: &Path,
    source_path: &str,
    module_name: Option<&str>,
    target: &str,
) {
    let target_path = resolve_import(repo_root, source_path, module_name, target);
    let kind = if target.starts_with('.') || target_path.is_some() {
        "internal"
    } else {
        "external"
    };
    imports.push(ParsedImport {
        target_module: target.to_string(),
        target_path,
        kind,
    });
}

fn quoted_import(text: &str) -> Option<&str> {
    let quote_index = text.find('"')?;
    let rest = &text[quote_index + 1..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn resolve_import(
    repo_root: &Path,
    source_path: &str,
    module_name: Option<&str>,
    target: &str,
) -> Option<String> {
    if target.starts_with('.') {
        let source_dir = repo_root.join(source_path).parent()?.to_path_buf();
        return first_go_file_in_dir(repo_root, &source_dir.join(target));
    }

    let module_name = module_name?;
    let relative = target
        .strip_prefix(&format!("{module_name}/"))
        .or_else(|| (target == module_name).then_some(""))?;
    first_go_file_in_dir(repo_root, &repo_root.join(relative))
}

fn first_go_file_in_dir(repo_root: &Path, path: &Path) -> Option<String> {
    let repo_root = repo_root.canonicalize().ok()?;
    let directory = if path.is_dir() { path } else { path.parent()? };
    let directory = directory.canonicalize().ok()?;
    if !directory.starts_with(&repo_root) {
        return None;
    }
    let mut candidates = fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("go")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with("_test.go"))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates
        .into_iter()
        .find_map(|candidate| relative_posix_path(&repo_root, &candidate))
}

fn go_module_name(repo_root: &Path) -> Option<String> {
    let contents = fs::read_to_string(repo_root.join("go.mod")).ok()?;
    contents.lines().find_map(|line| {
        let clean = strip_line_comment(line).trim();
        let rest = clean.strip_prefix("module ")?;
        let name = rest.trim();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    })
}

fn parse_identifier_at_start(text: &str) -> Option<&str> {
    let mut end = 0;
    for (index, character) in text.char_indices() {
        if index == 0 {
            if !is_identifier_start(character) {
                return None;
            }
            end = character.len_utf8();
            continue;
        }
        if !is_identifier_continue(character) {
            break;
        }
        end = index + character.len_utf8();
    }

    if end == 0 {
        None
    } else {
        Some(&text[..end])
    }
}

fn is_identifier(text: &str) -> bool {
    parse_identifier_at_start(text).is_some_and(|identifier| identifier == text)
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn is_exported_identifier(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
}

fn is_reserved_word(text: &str) -> bool {
    matches!(
        text,
        "break"
            | "case"
            | "chan"
            | "const"
            | "continue"
            | "default"
            | "defer"
            | "else"
            | "fallthrough"
            | "for"
            | "func"
            | "go"
            | "goto"
            | "if"
            | "import"
            | "interface"
            | "map"
            | "package"
            | "range"
            | "return"
            | "select"
            | "struct"
            | "switch"
            | "type"
            | "var"
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::parse_file_artifacts;

    #[test]
    fn parses_symbols_and_imports() {
        let root = fresh_test_dir("go-parser");
        write_file(&root, "go.mod", "module example.com/app\n");
        write_file(&root, "internal/jobs/jobs.go", "package jobs\n");

        let artifacts = parse_file_artifacts(
            &root,
            "cmd/api/main.go",
            r#"
package main

import (
  "fmt"
  jobs "example.com/app/internal/jobs"
)

const Version = "1"
type Server struct {}
type Handler interface {}
func NewServer() {}
func (s *Server) Serve() {}
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
            .map(|import| {
                (
                    import.kind,
                    import.target_module.as_str(),
                    import.target_path.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            symbols,
            [
                "constant:Version",
                "struct:Server",
                "interface:Handler",
                "function:NewServer",
                "method:Serve"
            ]
        );
        assert_eq!(
            imports,
            [
                ("external", "fmt", None),
                (
                    "internal",
                    "example.com/app/internal/jobs",
                    Some("internal/jobs/jobs.go")
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
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(root: &Path, path: &str, contents: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("file parent")).expect("create parent");
        fs::write(path, contents).expect("write file");
    }

    fn cleanup(path: PathBuf) {
        if path.exists() {
            fs::remove_dir_all(path).expect("cleanup temp root");
        }
    }
}
