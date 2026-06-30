use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{
    BaseElement, CommandError, EffectiveModel, ElementNode, ElementType, Model, RepoHandle,
    SourceKind, ValidationReport,
};

pub fn load_effective_model_from_repo(repo: RepoHandle) -> Result<EffectiveModel, CommandError> {
    let repo_root = Path::new(&repo.root_path).canonicalize().map_err(|error| {
        CommandError::with_details(
            "repo.path_missing",
            "Unable to resolve repository root.",
            serde_json::json!({ "error": error.to_string() }),
        )
    })?;
    let authored_path = repo_root.join("c4/model.yml");

    if !authored_path.exists() {
        return Err(CommandError::new(
            "model.not_found",
            "No c4/model.yml exists in this repository.",
        ));
    }
    let authored_path = authored_path.canonicalize().map_err(|error| {
        CommandError::with_details(
            "path.invalid",
            "Unable to resolve c4/model.yml.",
            serde_json::json!({ "error": error.to_string() }),
        )
    })?;
    if !authored_path.starts_with(&repo_root) {
        return Err(CommandError::new(
            "path.invalid",
            "c4/model.yml resolves outside the selected repository.",
        ));
    }

    let contents = fs::read_to_string(&authored_path).map_err(|error| {
        CommandError::with_details(
            "model.invalid",
            "Failed to read c4/model.yml.",
            serde_json::json!({ "error": error.to_string() }),
        )
    })?;
    let value = parse_plain_yaml_value(&contents)?;
    let mut model: Model = serde_yaml_ng::from_value(value).map_err(|error| {
        CommandError::with_details(
            "model.invalid",
            "Failed to parse c4/model.yml.",
            serde_json::json!({ "error": error.to_string() }),
        )
    })?;
    normalize_model(&mut model, false);

    let relationships = model.relationships.clone();
    let elements_by_slug = flatten_elements(&model);
    let source_sha = stable_source_sha(&contents);

    Ok(EffectiveModel {
        repo,
        source_sha: source_sha.clone(),
        authored_path: Some("c4/model.yml".to_string()),
        generated_path: None,
        model,
        elements_by_slug,
        relationships,
        validation: ValidationReport {
            ok: true,
            source_sha: Some(source_sha),
            issues: Vec::new(),
        },
    })
}

fn normalize_model(model: &mut Model, generated: bool) {
    model.generated = generated;

    for (slug, actor) in model.actors.iter_mut() {
        actor.base.slug = slug.clone();
        actor.base.generated = generated;
    }

    for (system_slug, system) in model.systems.iter_mut() {
        system.base.slug = system_slug.clone();
        system.base.generated = generated;
        for (container_slug, container) in system.containers.iter_mut() {
            container.base.slug = container_slug.clone();
            container.base.generated = generated;
            for (component_slug, component) in container.components.iter_mut() {
                component.base.slug = component_slug.clone();
                component.base.generated = generated;
            }
        }
    }

    for relationship in &mut model.relationships {
        relationship.generated = generated;
    }
}

fn flatten_elements(model: &Model) -> BTreeMap<String, ElementNode> {
    let mut output = BTreeMap::new();

    for (slug, actor) in &model.actors {
        output.insert(
            slug.clone(),
            ElementNode {
                base: base_with_slug(&actor.base, slug),
                element_type: ElementType::Actor,
                parent_slug: None,
                system_slug: None,
                container_slug: None,
                external: None,
                kind: None,
                source: SourceKind::Authored,
            },
        );
    }

    for (system_slug, system) in &model.systems {
        output.insert(
            system_slug.clone(),
            ElementNode {
                base: base_with_slug(&system.base, system_slug),
                element_type: ElementType::System,
                parent_slug: None,
                system_slug: Some(system_slug.clone()),
                container_slug: None,
                external: Some(system.external),
                kind: None,
                source: SourceKind::Authored,
            },
        );

        for (container_slug, container) in &system.containers {
            output.insert(
                container_slug.clone(),
                ElementNode {
                    base: base_with_slug(&container.base, container_slug),
                    element_type: ElementType::Container,
                    parent_slug: Some(system_slug.clone()),
                    system_slug: Some(system_slug.clone()),
                    container_slug: None,
                    external: Some(system.external),
                    kind: Some(container.kind.clone()),
                    source: SourceKind::Authored,
                },
            );

            for (component_slug, component) in &container.components {
                output.insert(
                    component_slug.clone(),
                    ElementNode {
                        base: base_with_slug(&component.base, component_slug),
                        element_type: ElementType::Component,
                        parent_slug: Some(container_slug.clone()),
                        system_slug: Some(system_slug.clone()),
                        container_slug: Some(container_slug.clone()),
                        external: Some(system.external),
                        kind: None,
                        source: SourceKind::Authored,
                    },
                );
            }
        }
    }

    output
}

fn base_with_slug(base: &BaseElement, slug: &str) -> BaseElement {
    let mut base = base.clone();
    base.slug = slug.to_string();
    base
}

fn stable_source_sha(contents: &str) -> String {
    format!("{:x}", Sha256::digest(contents.as_bytes()))
}

fn parse_plain_yaml_value(contents: &str) -> Result<serde_yaml_ng::Value, CommandError> {
    reject_anchor_and_alias_tokens(contents)?;

    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(contents).map_err(|error| {
        let message = error.to_string();
        if message.contains("duplicate entry") {
            return CommandError::with_details(
                "parse.duplicate_key",
                "YAML duplicate keys are unsupported.",
                serde_json::json!({ "error": message }),
            );
        }

        CommandError::with_details(
            "parse.invalid_yaml",
            "Failed to parse c4/model.yml.",
            serde_json::json!({ "error": message }),
        )
    })?;
    reject_merge_keys(&value)?;
    Ok(value)
}

fn reject_merge_keys(value: &serde_yaml_ng::Value) -> Result<(), CommandError> {
    match value {
        serde_yaml_ng::Value::Sequence(items) => {
            for item in items {
                reject_merge_keys(item)?;
            }
        }
        serde_yaml_ng::Value::Mapping(mapping) => {
            for (key, value) in mapping {
                if matches!(key, serde_yaml_ng::Value::String(key) if key == "<<") {
                    return Err(CommandError::new(
                        "parse.unsupported_yaml_feature",
                        "YAML merge keys are unsupported.",
                    ));
                }
                reject_merge_keys(key)?;
                reject_merge_keys(value)?;
            }
        }
        serde_yaml_ng::Value::Tagged(tagged) => {
            reject_merge_keys(&tagged.value)?;
        }
        serde_yaml_ng::Value::Null
        | serde_yaml_ng::Value::Bool(_)
        | serde_yaml_ng::Value::Number(_)
        | serde_yaml_ng::Value::String(_) => {}
    }

    Ok(())
}

fn reject_anchor_and_alias_tokens(contents: &str) -> Result<(), CommandError> {
    let mut block_scalar_parent_indent = None;
    let mut plain_scalar_parent_indent = None;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for (line_index, line) in contents.lines().enumerate() {
        let line_indent = leading_space_count(line);
        if let Some(parent_indent) = block_scalar_parent_indent {
            if line.trim().is_empty() || line_indent > parent_indent {
                continue;
            }
            block_scalar_parent_indent = None;
        }
        if let Some(parent_indent) = plain_scalar_parent_indent {
            if line.trim().is_empty() || line_indent > parent_indent {
                continue;
            }
            plain_scalar_parent_indent = None;
        }

        let mut escaped = false;
        let mut starts_block_scalar = None;
        let chars: Vec<(usize, char)> = line.char_indices().collect();

        for (char_index, (byte_index, character)) in chars.iter().enumerate() {
            if in_double_quote && escaped {
                escaped = false;
                continue;
            }

            match character {
                '\\' if in_double_quote => {
                    escaped = true;
                }
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                }
                '"' if !in_single_quote => {
                    in_double_quote = !in_double_quote;
                }
                '#' if !in_single_quote
                    && !in_double_quote
                    && is_yaml_comment_start(&chars, char_index) =>
                {
                    break;
                }
                '&' | '*' if !in_single_quote && !in_double_quote => {
                    if is_yaml_anchor_or_alias_token(line, &chars, char_index) {
                        return Err(CommandError::with_details(
                            "parse.unsupported_yaml_feature",
                            "YAML anchors and aliases are unsupported.",
                            serde_json::json!({
                                "line": line_index + 1,
                                "column": byte_index + 1,
                            }),
                        ));
                    }
                }
                '|' | '>' if !in_single_quote && !in_double_quote => {
                    if is_yaml_block_scalar_token(line, &chars, char_index) {
                        starts_block_scalar = Some(block_scalar_parent_indent_for_line(
                            line, &chars, char_index,
                        ));
                    }
                }
                _ => {}
            }
        }

        if let Some(parent_indent) = starts_block_scalar {
            block_scalar_parent_indent = Some(parent_indent);
            plain_scalar_parent_indent = None;
        } else if !in_single_quote && !in_double_quote {
            plain_scalar_parent_indent = plain_scalar_parent_indent_for_line(line, &chars);
        }
    }

    Ok(())
}

fn leading_space_count(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn block_scalar_parent_indent_for_line(
    line: &str,
    chars: &[(usize, char)],
    marker_index: usize,
) -> usize {
    let line_indent = leading_space_count(line);
    let Some((byte_index, _)) = chars.get(marker_index) else {
        return line_indent;
    };

    let prefix_after_indent = line[line_indent..*byte_index].trim_end();
    let Some(after_dash) = prefix_after_indent.strip_prefix('-') else {
        return line_indent;
    };
    let separation_spaces = after_dash
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    if separation_spaces == 0 {
        return line_indent;
    }

    if after_dash[separation_spaces..].contains(':') {
        line_indent + 1 + separation_spaces
    } else {
        line_indent
    }
}

fn plain_scalar_parent_indent_for_line(line: &str, chars: &[(usize, char)]) -> Option<usize> {
    let line_indent = leading_space_count(line);
    let comment_start = comment_start_byte(line, chars);
    let significant = line[..comment_start].trim_end();
    if significant.trim().is_empty() {
        return None;
    }

    if let Some(colon_byte) = mapping_value_colon(chars, comment_start) {
        let value = line[colon_byte + 1..comment_start].trim_start();
        if is_plain_scalar_value(value) {
            return Some(mapping_parent_indent_for_colon(line, colon_byte));
        }
        return None;
    }

    let after_indent = &significant[line_indent..];
    let Some(after_dash) = after_indent.strip_prefix('-') else {
        return None;
    };
    let separation_spaces = after_dash
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    if separation_spaces == 0 {
        return None;
    }

    is_plain_scalar_value(&after_dash[separation_spaces..]).then_some(line_indent)
}

fn comment_start_byte(line: &str, chars: &[(usize, char)]) -> usize {
    for (char_index, (byte_index, character)) in chars.iter().enumerate() {
        if *character == '#' && is_yaml_comment_start(chars, char_index) {
            return *byte_index;
        }
    }

    line.len()
}

fn mapping_value_colon(chars: &[(usize, char)], comment_start: usize) -> Option<usize> {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for (char_index, (byte_index, character)) in chars.iter().enumerate() {
        if *byte_index >= comment_start {
            break;
        }
        if in_double_quote && escaped {
            escaped = false;
            continue;
        }

        match character {
            '\\' if in_double_quote => {
                escaped = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ':' if !in_single_quote && !in_double_quote => {
                let next = chars.get(char_index + 1).map(|(_, character)| *character);
                if next.is_none_or(|character| character.is_whitespace()) {
                    return Some(*byte_index);
                }
            }
            _ => {}
        }
    }

    None
}

fn mapping_parent_indent_for_colon(line: &str, colon_byte: usize) -> usize {
    let line_indent = leading_space_count(line);
    let prefix_after_indent = line[line_indent..colon_byte].trim_end();
    let Some(after_dash) = prefix_after_indent.strip_prefix('-') else {
        return line_indent;
    };
    let separation_spaces = after_dash
        .chars()
        .take_while(|character| *character == ' ')
        .count();

    if separation_spaces > 0 && !after_dash[separation_spaces..].trim().is_empty() {
        line_indent + 1 + separation_spaces
    } else {
        line_indent
    }
}

fn is_plain_scalar_value(value: &str) -> bool {
    let mut value = value.trim_start();
    while value.starts_with('!') {
        let Some(tag_end) = value.find(char::is_whitespace) else {
            return false;
        };
        value = value[tag_end..].trim_start();
    }

    !value.is_empty()
        && !matches!(
            value.chars().next(),
            Some('\'' | '"' | '|' | '>' | '[' | '{' | '&' | '*')
        )
}

fn is_yaml_anchor_or_alias_token(line: &str, chars: &[(usize, char)], marker_index: usize) -> bool {
    let Some((byte_index, marker)) = chars.get(marker_index) else {
        return false;
    };
    if *marker != '&' && *marker != '*' {
        return false;
    }

    let next = line[*byte_index + marker.len_utf8()..].chars().next();
    if !matches!(next, Some(character) if character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        return false;
    }

    if let Some((_, previous)) = marker_index
        .checked_sub(1)
        .and_then(|index| chars.get(index))
    {
        if !previous.is_whitespace() && !is_yaml_token_boundary(*previous) {
            return false;
        }
    }

    let previous = chars[..marker_index]
        .iter()
        .rev()
        .find_map(|(_, character)| (!character.is_whitespace()).then_some(*character));

    previous.is_none()
        || matches!(previous, Some(':' | '[' | '{' | ',' | '-' | '?'))
        || previous_token_is_yaml_tag(line, chars, marker_index)
}

fn is_yaml_block_scalar_token(line: &str, chars: &[(usize, char)], marker_index: usize) -> bool {
    let Some((byte_index, marker)) = chars.get(marker_index) else {
        return false;
    };
    if *marker != '|' && *marker != '>' {
        return false;
    }

    if let Some((_, previous)) = marker_index
        .checked_sub(1)
        .and_then(|index| chars.get(index))
    {
        if !previous.is_whitespace() && !is_yaml_token_boundary(*previous) {
            return false;
        }
    }

    let previous = chars[..marker_index]
        .iter()
        .rev()
        .find_map(|(_, character)| (!character.is_whitespace()).then_some(*character));
    if previous.is_some()
        && !matches!(previous, Some(':' | '[' | '{' | ',' | '-' | '?'))
        && !previous_token_is_yaml_tag(line, chars, marker_index)
    {
        return false;
    }

    let mut rest = line[*byte_index + marker.len_utf8()..].chars().peekable();
    if matches!(rest.peek(), Some('+' | '-')) {
        rest.next();
    }
    while matches!(rest.peek(), Some(character) if character.is_ascii_digit()) {
        rest.next();
    }
    let remainder = rest.collect::<String>();
    let remainder = remainder.trim_start();

    remainder.is_empty() || remainder.starts_with('#')
}

fn is_yaml_comment_start(chars: &[(usize, char)], marker_index: usize) -> bool {
    chars[..marker_index]
        .last()
        .is_none_or(|(_, character)| character.is_whitespace())
}

fn is_yaml_token_boundary(character: char) -> bool {
    matches!(character, ':' | '[' | '{' | ',' | '-' | '?')
}

fn previous_token_is_yaml_tag(line: &str, chars: &[(usize, char)], marker_index: usize) -> bool {
    let Some((byte_index, _)) = chars.get(marker_index) else {
        return false;
    };
    let before = line[..*byte_index].trim_end();
    let token_start = before
        .rfind(char::is_whitespace)
        .map(|index| index + 1)
        .unwrap_or(0);

    before[token_start..].starts_with('!')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{load_effective_model_from_repo, repo_handle_from_path};

    #[test]
    fn loads_authored_model_yml_from_repo_c4_directory() {
        let root = fresh_test_dir("authored-model");
        fs::create_dir(root.join("c4")).expect("create c4 dir");
        fs::write(
            root.join("c4/model.yml"),
            r#"
name: Internet Banking
description: Lets customers view accounts and make payments.
actors:
  customer:
    name: Personal Banking Customer
systems:
  internet_banking:
    name: Internet Banking System
    containers:
      api:
        name: API Application
        tech: Rust
        kind: service
        components:
          signin:
            name: Sign In Controller
relationships:
  - from: customer
    to: api
    description: Uses
"#,
        )
        .expect("write model");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("effective model");

        assert_eq!(effective.model.name, "Internet Banking");
        assert_eq!(effective.model.actors["customer"].base.slug, "customer");
        assert_eq!(
            effective.model.systems["internet_banking"].base.slug,
            "internet_banking"
        );
        assert_eq!(
            effective.model.systems["internet_banking"].containers["api"]
                .base
                .slug,
            "api"
        );
        assert_eq!(effective.authored_path.as_deref(), Some("c4/model.yml"));
        assert_eq!(effective.generated_path, None);
        assert!(effective.validation.ok);
        assert_eq!(
            effective.elements_by_slug["customer"].base.name,
            "Personal Banking Customer"
        );
        assert_eq!(
            effective.elements_by_slug["customer"].source,
            crate::SourceKind::Authored
        );
        assert_eq!(
            effective.elements_by_slug["internet_banking"].element_type,
            crate::ElementType::System
        );
        assert_eq!(
            effective.elements_by_slug["api"].parent_slug.as_deref(),
            Some("internet_banking")
        );
        assert_eq!(
            effective.elements_by_slug["signin"]
                .container_slug
                .as_deref(),
            Some("api")
        );
        assert_eq!(effective.relationships[0].from, "customer");
        assert_eq!(effective.relationships[0].to, "api");
        assert_ne!(effective.source_sha, "sample-phase0-v1");

        cleanup(root);
    }

    #[test]
    fn reports_model_not_found_when_no_authored_or_generated_model_exists() {
        let root = fresh_test_dir("missing-model");
        fs::create_dir(root.join("c4")).expect("create c4 dir");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("missing model should fail");

        assert_eq!(error.code, "model.not_found");

        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_authored_model_symlink_that_resolves_outside_repo() {
        use std::os::unix::fs::symlink;

        let root = fresh_test_dir("escaping-model");
        let outside = fresh_test_dir("outside-model");
        fs::create_dir(root.join("c4")).expect("create c4 dir");
        let outside_model = outside.join("model.yml");
        fs::write(&outside_model, "name: Outside\n").expect("write outside model");
        symlink(&outside_model, root.join("c4/model.yml")).expect("create model symlink");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("escaping model should fail");

        assert_eq!(error.code, "path.invalid");

        cleanup(root);
        cleanup(outside);
    }

    #[test]
    fn rejects_yaml_anchor_before_loading_model() {
        let root = fresh_test_dir("yaml-anchor");
        write_model(
            &root,
            r#"
name: Anchored
actors:
  customer: &customer
    name: Customer
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("anchor should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_alias_before_loading_model() {
        let root = fresh_test_dir("yaml-alias");
        write_model(
            &root,
            r#"
name: Alias
actors:
  customer: *customer
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("alias should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_anchor_in_explicit_mapping_key() {
        let root = fresh_test_dir("yaml-explicit-key-anchor");
        write_model(
            &root,
            r#"
name: Explicit Anchor
actors:
  ? &customer customer
  : name: Customer
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("anchor should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_alias_in_explicit_mapping_key() {
        let root = fresh_test_dir("yaml-explicit-key-alias");
        write_model(
            &root,
            r#"
name: Explicit Alias
actors:
  ? &customer customer
  : name: Customer
systems:
  ? *customer
  : name: Customer System
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("alias should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_anchor_after_tag() {
        let root = fresh_test_dir("yaml-tagged-anchor");
        write_model(&root, "name: !str &modelName Tagged Anchor\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("anchor should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_anchor_in_flow_mapping() {
        let root = fresh_test_dir("yaml-flow-anchor");
        write_model(
            &root,
            r#"
name: Flow Anchor
actors: { customer: &customer { name: Customer } }
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("anchor should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_anchor_after_compact_sequence_block_scalar() {
        let root = fresh_test_dir("yaml-anchor-after-sequence-block-scalar");
        write_model(
            &root,
            r#"
name: Hidden Anchor
relationships:
  - description: |
      Uses
    from: &source customer
    to: *source
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("anchor should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_anchor_after_extra_spaced_sequence_block_scalar() {
        let root = fresh_test_dir("yaml-anchor-after-extra-spaced-sequence-block-scalar");
        write_model(
            &root,
            r#"
name: Hidden Anchor
relationships:
  -   description: |
        Uses
      from: &source customer
      to: *source
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("anchor should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_anchor_after_hash_in_plain_flow_scalar() {
        let root = fresh_test_dir("yaml-flow-hash-anchor");
        write_model(
            &root,
            r#"
name: Flow Hash
actors: { a: { name: foo#bar }, customer: &customer { name: Customer } }
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("anchor should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn allows_anchor_like_text_inside_quotes_and_comments() {
        let root = fresh_test_dir("yaml-quoted-anchor-text");
        write_model(
            &root,
            r#"
name: "Quoted *not_alias &not_anchor"
description: 'Single quoted *text & text'
# *commented_alias &commented_anchor
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("quoted text should load");

        assert_eq!(effective.model.name, "Quoted *not_alias &not_anchor");
        assert_eq!(
            effective.model.description.as_deref(),
            Some("Single quoted *text & text")
        );

        cleanup(root);
    }

    #[test]
    fn allows_anchor_like_text_inside_multiline_quotes() {
        let root = fresh_test_dir("yaml-multiline-quoted-anchor-text");
        write_model(
            &root,
            r#"
name: Multiline Quoted
description: "first line
  &literal ampersand
  *literal star"
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective =
            load_effective_model_from_repo(repo).expect("multiline quoted text should load");

        assert_eq!(effective.model.name, "Multiline Quoted");
        assert_eq!(
            effective.model.description.as_deref(),
            Some("first line &literal ampersand *literal star")
        );

        cleanup(root);
    }

    #[test]
    fn allows_anchor_like_text_inside_multiline_plain_scalar() {
        let root = fresh_test_dir("yaml-multiline-plain-anchor-text");
        write_model(
            &root,
            r#"
name: Plain Multiline
description: first line
  &literal ampersand
  *literal star
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective =
            load_effective_model_from_repo(repo).expect("multiline plain text should load");

        assert_eq!(effective.model.name, "Plain Multiline");
        assert_eq!(
            effective.model.description.as_deref(),
            Some("first line &literal ampersand *literal star")
        );

        cleanup(root);
    }

    #[test]
    fn allows_anchor_like_text_inside_block_scalar() {
        let root = fresh_test_dir("yaml-block-scalar-text");
        write_model(
            &root,
            r#"
name: Block Scalar
description: |
  *important note
  & another note
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("block scalar should load");

        assert_eq!(effective.model.name, "Block Scalar");
        assert_eq!(
            effective.model.description.as_deref(),
            Some("*important note\n& another note\n")
        );

        cleanup(root);
    }

    #[test]
    fn rejects_yaml_merge_key_before_loading_model() {
        let root = fresh_test_dir("yaml-merge-key");
        write_model(
            &root,
            r#"
name: Merge Key
actors:
  customer:
    <<:
      name: Customer
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("merge key should fail");

        assert_eq!(error.code, "parse.unsupported_yaml_feature");

        cleanup(root);
    }

    #[test]
    fn rejects_duplicate_yaml_keys_before_loading_model() {
        let root = fresh_test_dir("duplicate-key");
        write_model(
            &root,
            r#"
name: First
name: Second
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("duplicate key should fail");

        assert_eq!(error.code, "parse.duplicate_key");

        cleanup(root);
    }

    #[test]
    fn rejects_nested_duplicate_yaml_keys_before_loading_model() {
        let root = fresh_test_dir("nested-duplicate-key");
        write_model(
            &root,
            r#"
name: Nested Duplicate
actors:
  customer:
    name: Customer
  customer:
    name: Customer Duplicate
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("duplicate key should fail");

        assert_eq!(error.code, "parse.duplicate_key");

        cleanup(root);
    }

    fn write_model(root: &PathBuf, contents: &str) {
        fs::create_dir_all(root.join("c4")).expect("create c4 dir");
        fs::write(root.join("c4/model.yml"), contents).expect("write model");
    }

    fn fresh_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "c4lens-loader-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test dir");
        path
    }

    fn cleanup(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
