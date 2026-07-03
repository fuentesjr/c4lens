use std::path::{Path, PathBuf};

use super::artifacts::{ParsedArtifacts, ParsedImport, ParsedSymbol};
use super::relative_posix_path;

pub(super) fn parse_file_artifacts(
    repo_root: &Path,
    source_path: &str,
    text: &str,
) -> ParsedArtifacts {
    let mut artifacts = ParsedArtifacts::default();
    let mut class_stack: Vec<(usize, String)> = Vec::new();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_no = (line_index + 1) as i32;
        let line = strip_inline_comment(raw_line);
        let clean = line.trim_start();
        if clean.is_empty() {
            continue;
        }

        let indent = line.len().saturating_sub(clean.len());
        while class_stack
            .last()
            .is_some_and(|(class_indent, _)| indent <= *class_indent)
        {
            class_stack.pop();
        }

        extract_imports(&mut artifacts.imports, repo_root, source_path, clean);

        if let Some(name) = parse_name_after_keyword(clean, "class") {
            push_symbol(&mut artifacts.symbols, "class", name, None, line_no, line);
            class_stack.push((indent, name.to_string()));
            continue;
        }

        let function_name = parse_name_after_keyword(clean, "def").or_else(|| {
            clean
                .strip_prefix("async ")
                .and_then(|rest| parse_name_after_keyword(rest.trim_start(), "def"))
        });
        if let Some(name) = function_name {
            let class_scope = class_stack
                .last()
                .filter(|(class_indent, _)| indent > *class_indent)
                .map(|(_, name)| name.as_str());
            let qualified_name = class_scope.map(|class_name| format!("{class_name}.{name}"));
            let kind = if class_scope.is_some() {
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
            continue;
        }

        if indent == 0 {
            if let Some(name) = parse_constant_assignment(clean) {
                push_symbol(
                    &mut artifacts.symbols,
                    "constant",
                    name,
                    None,
                    line_no,
                    line,
                );
            }
        }
    }

    artifacts
}

fn strip_inline_comment(line: &str) -> &str {
    line.find('#').map_or(line, |index| &line[..index])
}

fn parse_name_after_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = strip_keyword_prefix(text, keyword)?.trim_start();
    let name = take_identifier(rest)?;
    let after_name = rest[name.len()..].trim_start();
    if matches!(keyword, "class" | "def") && !after_name.starts_with(['(', ':']) {
        return None;
    }
    Some(name)
}

fn strip_keyword_prefix<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(keyword)?;
    if rest.chars().next().is_some_and(is_identifier_continue) {
        return None;
    }
    Some(rest)
}

fn parse_constant_assignment(text: &str) -> Option<&str> {
    let name = take_identifier(text)?;
    if !is_constant_name(name) {
        return None;
    }
    let rest = text[name.len()..].trim_start();
    if rest.starts_with('=') {
        Some(name)
    } else {
        None
    }
}

fn push_symbol(
    symbols: &mut Vec<ParsedSymbol>,
    kind: &'static str,
    name: &str,
    qualified_name: Option<String>,
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
    if let Some(rest) = clean.strip_prefix("import ") {
        for target in rest.split(',').filter_map(normalize_import_target) {
            push_import(imports, repo_root, source_path, &target);
        }
        return;
    }

    let Some(rest) = clean.strip_prefix("from ") else {
        return;
    };
    let Some((module, names)) = rest.split_once(" import ") else {
        return;
    };
    let module = module.trim();
    if module.is_empty() {
        return;
    }

    if module.chars().all(|character| character == '.') {
        for name in names.split(',').filter_map(normalize_import_target) {
            push_import(imports, repo_root, source_path, &format!("{module}{name}"));
        }
    } else {
        push_import(imports, repo_root, source_path, module);
    }
}

fn normalize_import_target(raw: &str) -> Option<String> {
    let module = raw
        .trim()
        .split_once(" as ")
        .map_or_else(|| raw.trim(), |(module, _)| module.trim());
    if module.is_empty() || module == "*" {
        None
    } else {
        Some(module.to_string())
    }
}

fn push_import(imports: &mut Vec<ParsedImport>, repo_root: &Path, source_path: &str, target: &str) {
    let target_path = resolve_import(repo_root, source_path, target);
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

fn resolve_import(repo_root: &Path, source_path: &str, target: &str) -> Option<String> {
    let (base, module) = if target.starts_with('.') {
        let dots = target
            .chars()
            .take_while(|character| *character == '.')
            .count();
        let mut base = repo_root.join(source_path).parent()?.to_path_buf();
        for _ in 1..dots {
            base = base.parent()?.to_path_buf();
        }
        (base, &target[dots..])
    } else {
        (repo_root.to_path_buf(), target)
    };

    let segments = module
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }

    let direct = module_candidates(&base, &segments);
    for candidate in direct {
        if candidate.is_file() {
            return relative_posix_path(repo_root, &candidate);
        }
    }

    if !target.starts_with('.') {
        let src_base = repo_root.join("src");
        for candidate in module_candidates(&src_base, &segments) {
            if candidate.is_file() {
                return relative_posix_path(repo_root, &candidate);
            }
        }
    }

    None
}

fn module_candidates(base: &Path, segments: &[&str]) -> Vec<PathBuf> {
    let mut path = base.to_path_buf();
    for segment in segments {
        path.push(segment);
    }
    vec![path.with_extension("py"), path.join("__init__.py")]
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
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
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
        "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "False"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "None"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "True"
            | "try"
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
        let root = fresh_test_dir("python-parser");
        write_file(&root, "app/tasks.py", "");

        let artifacts = parse_file_artifacts(
            &root,
            "app/service.py",
            r#"
import os, app.tasks as tasks
from .tasks import run_task
API_URL = "/api"
class Service:
    def handle(self):
        pass
async def boot():
    pass
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
                "constant:API_URL:None",
                "class:Service:None",
                "method:handle:Some(\"Service.handle\")",
                "function:boot:None"
            ]
        );
        assert_eq!(
            imports,
            [
                ("external", "os", None),
                ("internal", "app.tasks", Some("app/tasks.py")),
                ("internal", ".tasks", Some("app/tasks.py"))
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
