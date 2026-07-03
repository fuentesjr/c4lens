use std::path::{Path, PathBuf};

use super::artifacts::{ParsedArtifacts, ParsedImport, ParsedSymbol};
use super::relative_posix_path;

pub(super) fn parse_file_artifacts(
    repo_root: &Path,
    source_path: &str,
    text: &str,
) -> ParsedArtifacts {
    let mut artifacts = ParsedArtifacts::default();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_no = (line_index + 1) as i32;
        let line = strip_line_comment(raw_line);
        let clean = line.trim_start();
        if clean.is_empty() {
            continue;
        }

        extract_imports(&mut artifacts.imports, repo_root, source_path, clean);
        extract_symbols(&mut artifacts.symbols, line_no, line, clean);
    }

    artifacts
}

fn strip_line_comment(line: &str) -> &str {
    line.find("//").map_or(line, |index| &line[..index])
}

fn extract_symbols(symbols: &mut Vec<ParsedSymbol>, line_no: i32, line: &str, clean: &str) {
    let (without_export, exported) = consume_prefix_keywords(clean, &["export"]);
    let (candidate, _) = consume_prefix_keywords(
        without_export,
        &[
            "default",
            "declare",
            "abstract",
            "async",
            "public",
            "private",
            "protected",
        ],
    );

    for (keyword, kind) in [
        ("class", "class"),
        ("interface", "interface"),
        ("enum", "enum"),
        ("function", "function"),
    ] {
        if let Some(name) = parse_name_after_keyword(candidate, keyword) {
            push_symbol(symbols, kind, name, line_no, line);
            return;
        }
    }

    if let Some((name, kind)) = parse_variable_symbol(candidate, exported) {
        push_symbol(symbols, kind, name, line_no, line);
    }
}

fn consume_prefix_keywords<'a>(mut text: &'a str, keywords: &[&str]) -> (&'a str, bool) {
    let mut consumed = false;
    loop {
        let mut changed = false;
        for keyword in keywords {
            if let Some(rest) = strip_keyword_prefix(text, keyword) {
                text = rest.trim_start();
                consumed = true;
                changed = true;
                break;
            }
        }
        if !changed {
            return (text, consumed);
        }
    }
}

fn strip_keyword_prefix<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(keyword)?;
    if rest.chars().next().is_some_and(is_identifier_continue) {
        return None;
    }
    Some(rest)
}

fn parse_name_after_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = strip_keyword_prefix(text, keyword)?.trim_start();
    take_identifier(rest)
}

fn parse_variable_symbol(text: &str, exported: bool) -> Option<(&str, &'static str)> {
    let mut keyword = None;
    for candidate in ["const", "let", "var"] {
        if let Some(rest) = strip_keyword_prefix(text, candidate) {
            keyword = Some((candidate, rest.trim_start()));
            break;
        }
    }

    let (keyword, rest) = keyword?;
    let name = take_identifier(rest)?;
    let after_name = rest[name.len()..].trim_start();
    let assignment = after_name
        .split_once('=')
        .map(|(_, right)| right.trim_start())
        .unwrap_or_default();
    if assignment.contains("=>") || assignment.starts_with("function") {
        return Some((name, "function"));
    }
    if exported || (keyword == "const" && is_constant_name(name)) {
        return Some((name, "constant"));
    }
    None
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

fn extract_imports(
    imports: &mut Vec<ParsedImport>,
    repo_root: &Path,
    source_path: &str,
    clean: &str,
) {
    let mut targets = Vec::new();
    if clean.starts_with("import ") {
        if let Some(target) = module_specifier_from_import(clean) {
            targets.push(target);
        }
    }
    if clean.starts_with("export ") {
        if let Some(target) = module_specifier_after_from(clean) {
            targets.push(target);
        }
    }
    if let Some(target) = module_specifier_from_require(clean) {
        targets.push(target);
    }

    for target in targets {
        if target.is_empty() {
            continue;
        }

        let target_path = resolve_module_specifier(repo_root, source_path, target);
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
}

fn module_specifier_from_import(line: &str) -> Option<&str> {
    if let Some(target) = module_specifier_after_from(line) {
        return Some(target);
    }

    let rest = line.strip_prefix("import ")?.trim_start();
    quoted_string(rest)
}

fn module_specifier_after_from(line: &str) -> Option<&str> {
    let (_, rest) = line.rsplit_once(" from ")?;
    quoted_string(rest.trim_start())
}

fn module_specifier_from_require(line: &str) -> Option<&str> {
    let require_index = line.find("require(")?;
    quoted_string(line[require_index + "require(".len()..].trim_start())
}

fn quoted_string(text: &str) -> Option<&str> {
    let mut chars = text.char_indices();
    let (_, quote) = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    for (index, character) in chars {
        if character == quote {
            return Some(&text[quote.len_utf8()..index]);
        }
    }
    None
}

fn resolve_module_specifier(repo_root: &Path, source_path: &str, target: &str) -> Option<String> {
    if !target.starts_with('.') {
        return None;
    }

    let source_dir = repo_root.join(source_path).parent()?.to_path_buf();
    let base = source_dir.join(target);
    for candidate in module_candidates(&base) {
        if candidate.is_file() {
            return relative_posix_path(repo_root, &candidate);
        }
    }

    None
}

fn module_candidates(base: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![base.to_path_buf()];
    for extension in ["ts", "tsx", "js", "jsx", "mjs", "cjs", "json"] {
        candidates.push(base.with_extension(extension));
    }
    for extension in ["ts", "tsx", "js", "jsx", "mjs", "cjs"] {
        candidates.push(base.join(format!("index.{extension}")));
    }
    candidates
}

fn take_identifier(text: &str) -> Option<&str> {
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
    take_identifier(text).is_some_and(|identifier| identifier == text)
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_' || character == '$'
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '$'
}

fn is_constant_name(name: &str) -> bool {
    name.chars().any(|character| character.is_ascii_uppercase())
        && name.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_reserved_word(text: &str) -> bool {
    matches!(
        text,
        "as" | "async"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "from"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "undefined"
            | "var"
            | "void"
            | "while"
            | "with"
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
    fn parses_symbols_and_imports() {
        let root = fresh_test_dir("ts-parser");
        write_file(&root, "src/http.ts", "");

        let artifacts = parse_file_artifacts(
            &root,
            "src/index.ts",
            r#"
import { request } from "./http";
import React from "react";
export class ApiClient {}
export interface Handler {}
export const API_URL = "/api";
const boot = () => {};
function run() {}
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
                "class:ApiClient",
                "interface:Handler",
                "constant:API_URL",
                "function:boot",
                "function:run"
            ]
        );
        assert_eq!(
            imports,
            [
                ("internal", "./http", Some("src/http.ts")),
                ("external", "react", None)
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
