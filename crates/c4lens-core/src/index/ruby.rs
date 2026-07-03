use std::path::Path;

use super::artifacts::{ParsedArtifacts, ParsedImport, ParsedSymbol};
use super::relative_posix_path;

#[derive(Clone, Debug)]
enum RubyBlock {
    Scope(String),
    Def,
    Other,
}

pub(super) fn parse_file_artifacts(
    repo_root: &Path,
    source_path: &str,
    text: &str,
) -> ParsedArtifacts {
    let mut artifacts = ParsedArtifacts::default();
    let mut blocks = Vec::new();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_no = (line_index + 1) as i32;
        let line = strip_inline_comment(raw_line);
        let clean = line.trim_start();
        if clean.is_empty() {
            continue;
        }

        if clean == "end" {
            blocks.pop();
            continue;
        }

        extract_imports(&mut artifacts.imports, repo_root, source_path, clean);

        if let Some(name) = parse_scope_name(clean, "class") {
            push_symbol(
                &mut artifacts.symbols,
                "class",
                name,
                Some(qualified_scope_name(&blocks, name)),
                line_no,
                line,
            );
            blocks.push(RubyBlock::Scope(name.to_string()));
            continue;
        }

        if let Some(name) = parse_scope_name(clean, "module") {
            push_symbol(
                &mut artifacts.symbols,
                "module",
                name,
                Some(qualified_scope_name(&blocks, name)),
                line_no,
                line,
            );
            blocks.push(RubyBlock::Scope(name.to_string()));
            continue;
        }

        if let Some(raw_name) = parse_method_name(clean) {
            let name = raw_name.rsplit('.').next().unwrap_or(raw_name);
            let scope = current_scope_name(&blocks);
            let qualified_name = scope
                .as_deref()
                .map(|scope| format!("{scope}.{name}"))
                .or_else(|| Some(name.to_string()));
            let kind = if scope.is_some() {
                "method"
            } else {
                "function"
            };
            push_symbol(
                &mut artifacts.symbols,
                kind,
                name,
                qualified_name,
                line_no,
                line,
            );
            blocks.push(RubyBlock::Def);
            continue;
        }

        if let Some(name) = parse_constant_assignment(clean) {
            push_symbol(
                &mut artifacts.symbols,
                "constant",
                name,
                current_scope_name(&blocks).map(|scope| format!("{scope}::{name}")),
                line_no,
                line,
            );
            continue;
        }

        if starts_block(clean) {
            blocks.push(RubyBlock::Other);
        }
    }

    artifacts
}

fn strip_inline_comment(line: &str) -> &str {
    line.find('#').map_or(line, |index| &line[..index])
}

fn parse_scope_name<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = strip_keyword_prefix(line, keyword)?.trim_start();
    let name = rest.split([' ', '<']).next()?.trim();
    if is_constant_path(name) {
        Some(name)
    } else {
        None
    }
}

fn parse_method_name(line: &str) -> Option<&str> {
    let rest = strip_keyword_prefix(line, "def")?.trim_start();
    let name = rest.split(['(', ' ', ';']).next()?.trim();
    if name.is_empty() {
        return None;
    }
    let short_name = name.rsplit('.').next().unwrap_or(name);
    if is_method_name(short_name) {
        Some(name)
    } else {
        None
    }
}

fn parse_constant_assignment(line: &str) -> Option<&str> {
    let name = line.split([' ', '=']).next()?.trim();
    if !is_constant_name(name) {
        return None;
    }
    let rest = line[name.len()..].trim_start();
    if rest.starts_with('=') {
        Some(name)
    } else {
        None
    }
}

fn strip_keyword_prefix<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(keyword)?;
    if rest
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    Some(rest)
}

fn qualified_scope_name(blocks: &[RubyBlock], name: &str) -> String {
    current_scope_name(blocks).map_or_else(|| name.to_string(), |scope| format!("{scope}::{name}"))
}

fn current_scope_name(blocks: &[RubyBlock]) -> Option<String> {
    let names = blocks
        .iter()
        .filter_map(|block| match block {
            RubyBlock::Scope(name) => Some(name.as_str()),
            RubyBlock::Def | RubyBlock::Other => None,
        })
        .collect::<Vec<_>>();
    if names.is_empty() {
        None
    } else {
        Some(names.join("::"))
    }
}

fn starts_block(line: &str) -> bool {
    line.ends_with(" do")
        || [
            "if ", "unless ", "case ", "begin", "while ", "until ", "for ",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

fn push_symbol(
    symbols: &mut Vec<ParsedSymbol>,
    kind: &'static str,
    name: &str,
    qualified_name: Option<String>,
    line_no: i32,
    line: &str,
) {
    let column = line.find(name).unwrap_or(0) as i32;
    symbols.push(ParsedSymbol {
        kind,
        name: name.to_string(),
        qualified_name,
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
    if let Some(target) = quoted_argument_after(clean, "require_relative") {
        imports.push(ParsedImport {
            target_module: target.to_string(),
            target_path: resolve_require_relative(repo_root, source_path, target),
            kind: "internal",
        });
        return;
    }

    if let Some(target) = quoted_argument_after(clean, "require") {
        let target_path = resolve_require(repo_root, target);
        let kind = if target_path.is_some() {
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

fn quoted_argument_after<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = strip_keyword_prefix(line, keyword)?.trim_start();
    quoted_string(rest)
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

fn resolve_require_relative(repo_root: &Path, source_path: &str, target: &str) -> Option<String> {
    let source_dir = repo_root.join(source_path).parent()?.to_path_buf();
    resolve_ruby_path(repo_root, &source_dir.join(target))
}

fn resolve_require(repo_root: &Path, target: &str) -> Option<String> {
    resolve_ruby_path(repo_root, &repo_root.join(target))
        .or_else(|| resolve_ruby_path(repo_root, &repo_root.join("lib").join(target)))
}

fn resolve_ruby_path(repo_root: &Path, base: &Path) -> Option<String> {
    for candidate in [base.to_path_buf(), base.with_extension("rb")] {
        if candidate.is_file() {
            return relative_posix_path(repo_root, &candidate);
        }
    }
    None
}

fn is_constant_path(text: &str) -> bool {
    !text.is_empty() && text.split("::").all(is_constant_name)
}

fn is_constant_name(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_uppercase())
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn is_method_name(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '?' | '!' | '=')
        })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::parse_file_artifacts;

    #[test]
    fn parses_symbols_and_imports() {
        let root = fresh_test_dir("ruby-parser");
        write_file(&root, "lib/app/task.rb", "");

        let artifacts = parse_file_artifacts(
            &root,
            "lib/app/service.rb",
            r#"
require "json"
require_relative "task"
module App
  class Service
    API_URL = "/api"
    def call
    end
  end
end
"#,
        );

        let symbols = artifacts
            .symbols
            .iter()
            .map(|symbol| {
                format!(
                    "{}:{}:{:?}",
                    symbol.kind, symbol.name, symbol.qualified_name
                )
            })
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
                "module:App:Some(\"App\")",
                "class:Service:Some(\"App::Service\")",
                "constant:API_URL:Some(\"App::Service::API_URL\")",
                "method:call:Some(\"App::Service.call\")"
            ]
        );
        assert_eq!(
            imports,
            [
                ("external", "json", None),
                ("internal", "task", Some("lib/app/task.rb"))
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
