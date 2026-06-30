use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{
    BaseElement, CommandError, EffectiveModel, ElementNode, ElementType, Model, Relationship,
    RepoHandle, SourceKind, ValidationIssue, ValidationReport, ValidationSeverity, ValidationStage,
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
    validate_relationships(&relationships, &elements_by_slug)?;
    let issues = validate_code_paths(&repo_root, &elements_by_slug)?;
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
            issues,
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

fn validate_relationships(
    relationships: &[Relationship],
    elements_by_slug: &BTreeMap<String, ElementNode>,
) -> Result<(), CommandError> {
    for (index, relationship) in relationships.iter().enumerate() {
        if !elements_by_slug.contains_key(&relationship.from) {
            return Err(CommandError::with_details(
                "semantic.unresolved_relationship_source",
                "Relationship source does not resolve to an element.",
                serde_json::json!({
                    "path": format!("/relationships/{index}/from"),
                    "slug": relationship.from,
                }),
            ));
        }

        if !elements_by_slug.contains_key(&relationship.to) {
            return Err(CommandError::with_details(
                "semantic.unresolved_relationship_target",
                "Relationship target does not resolve to an element.",
                serde_json::json!({
                    "path": format!("/relationships/{index}/to"),
                    "slug": relationship.to,
                }),
            ));
        }
    }

    Ok(())
}

fn validate_code_paths(
    repo_root: &Path,
    elements_by_slug: &BTreeMap<String, ElementNode>,
) -> Result<Vec<ValidationIssue>, CommandError> {
    let mut issues = Vec::new();

    for element in elements_by_slug.values() {
        let Some(code_path) = element.base.code.as_deref() else {
            continue;
        };
        if let Some(issue) =
            validate_model_code_path(repo_root, element, code_path, &element_code_path(element))?
        {
            issues.push(issue);
        }
    }

    Ok(issues)
}

fn validate_model_code_path(
    repo_root: &Path,
    element: &ElementNode,
    code_path: &str,
    model_path: &str,
) -> Result<Option<ValidationIssue>, CommandError> {
    let relative_path = normalized_relative_code_path(code_path).map_err(|code| {
        CommandError::with_details(
            code,
            "Model code path uses unsupported syntax.",
            serde_json::json!({
                "path": model_path,
                "slug": element.base.slug,
                "code": code_path,
            }),
        )
    })?;

    let resolved_path = repo_root.join(&relative_path);
    let Some(existing_path) = deepest_existing_path(&resolved_path) else {
        return Err(CommandError::with_details(
            "semantic.code_path_outside_repo",
            "Model code path escapes the selected repository.",
            serde_json::json!({
                "path": model_path,
                "slug": element.base.slug,
                "code": code_path,
            }),
        ));
    };
    let canonical_existing_path = existing_path.canonicalize().map_err(|error| {
        CommandError::with_details(
            "semantic.code_path_outside_repo",
            "Model code path could not be resolved inside the selected repository.",
            serde_json::json!({
                "path": model_path,
                "slug": element.base.slug,
                "code": code_path,
                "error": error.to_string(),
            }),
        )
    })?;

    if !canonical_existing_path.starts_with(repo_root) {
        return Err(CommandError::with_details(
            "semantic.code_path_outside_repo",
            "Model code path resolves outside the selected repository.",
            serde_json::json!({
                "path": model_path,
                "slug": element.base.slug,
                "code": code_path,
            }),
        ));
    }

    if path_exists_without_following_final_symlink(&resolved_path) {
        return Ok(None);
    }

    Ok(Some(ValidationIssue {
        severity: ValidationSeverity::Warning,
        stage: ValidationStage::Semantic,
        code: "semantic.code_path_missing".to_string(),
        message: "Model code path does not exist inside the selected repository.".to_string(),
        path: Some(model_path.to_string()),
        line: None,
        column: None,
    }))
}

fn normalized_relative_code_path(code_path: &str) -> Result<PathBuf, &'static str> {
    if code_path.is_empty()
        || code_path.starts_with('/')
        || code_path.contains('\\')
        || code_path.contains('\0')
        || code_path.contains("://")
        || looks_like_windows_drive_path(code_path)
    {
        return Err("semantic.code_path_invalid");
    }

    let mut normalized = PathBuf::new();
    for segment in code_path.split('/') {
        if segment.is_empty() {
            return Err("semantic.code_path_invalid");
        }
        match segment {
            "." => {}
            ".." => {
                if !normalized.pop() {
                    return Err("semantic.code_path_outside_repo");
                }
            }
            _ => normalized.push(segment),
        }
    }

    Ok(normalized)
}

fn looks_like_windows_drive_path(code_path: &str) -> bool {
    let mut chars = code_path.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
        && matches!(chars.next(), Some(':'))
}

fn deepest_existing_path(path: &Path) -> Option<&Path> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if path_exists_without_following_final_symlink(candidate) {
            return Some(candidate);
        }
        current = candidate.parent();
    }

    None
}

fn path_exists_without_following_final_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn element_code_path(element: &ElementNode) -> String {
    match element.element_type {
        ElementType::Actor => format!("/actors/{}/code", element.base.slug),
        ElementType::System => format!("/systems/{}/code", element.base.slug),
        ElementType::Container => format!(
            "/systems/{}/containers/{}/code",
            element.system_slug.as_deref().unwrap_or("<unknown>"),
            element.base.slug
        ),
        ElementType::Component => format!(
            "/systems/{}/containers/{}/components/{}/code",
            element.system_slug.as_deref().unwrap_or("<unknown>"),
            element.container_slug.as_deref().unwrap_or("<unknown>"),
            element.base.slug
        ),
    }
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

    let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(contents).map_err(|error| {
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
    normalize_relationship_shorthand(&mut value)?;
    validate_top_level_model_schema(&value)?;
    normalize_technology_aliases(&mut value, "")?;
    Ok(value)
}

fn normalize_relationship_shorthand(value: &mut serde_yaml_ng::Value) -> Result<(), CommandError> {
    let serde_yaml_ng::Value::Mapping(mapping) = value else {
        return Ok(());
    };
    let Some(serde_yaml_ng::Value::Sequence(relationships)) = mapping.get_mut("relationships")
    else {
        return Ok(());
    };

    for (index, relationship) in relationships.iter_mut().enumerate() {
        let path = format!("/relationships/{index}");
        if let Some(normalized) = relationship_shorthand_to_mapping(relationship, &path)? {
            *relationship = normalized;
        }
    }

    Ok(())
}

fn relationship_shorthand_to_mapping(
    value: &serde_yaml_ng::Value,
    path: &str,
) -> Result<Option<serde_yaml_ng::Value>, CommandError> {
    match value {
        serde_yaml_ng::Value::String(shorthand) => {
            parse_relationship_shorthand_string(shorthand, path).map(Some)
        }
        serde_yaml_ng::Value::Mapping(mapping) if mapping.len() == 1 => {
            let Some((key, description)) = mapping.iter().next() else {
                return Ok(None);
            };
            let Some(key) = yaml_string_value(key) else {
                return Ok(None);
            };
            if !key.contains("->") {
                return Ok(None);
            }
            let Some(description) = yaml_string_value(description) else {
                return Err(schema_error(
                    "schema.invalid_type",
                    "Relationship shorthand description must be a string.",
                    path,
                    serde_json::json!({ "expected": "string" }),
                ));
            };

            relationship_mapping_from_parts(key, description, path).map(Some)
        }
        _ => Ok(None),
    }
}

fn parse_relationship_shorthand_string(
    shorthand: &str,
    path: &str,
) -> Result<serde_yaml_ng::Value, CommandError> {
    let Some((identity, description)) = shorthand.split_once(':') else {
        return Err(schema_error(
            "schema.pattern",
            "Relationship shorthand must use `from -> to: description`.",
            path,
            serde_json::json!({ "pattern": "from -> to: description" }),
        ));
    };

    relationship_mapping_from_parts(identity, description, path)
}

fn relationship_mapping_from_parts(
    identity: &str,
    description: &str,
    path: &str,
) -> Result<serde_yaml_ng::Value, CommandError> {
    let Some((from, to)) = identity.split_once("->") else {
        return Err(schema_error(
            "schema.pattern",
            "Relationship shorthand must use `from -> to`.",
            path,
            serde_json::json!({ "pattern": "from -> to" }),
        ));
    };
    let from = from.trim();
    let to = to.trim();
    let description = description.trim();

    if !is_valid_slug(from) || !is_valid_slug(to) {
        return Err(schema_error(
            "schema.pattern",
            "Relationship shorthand endpoints must be valid slugs.",
            path,
            serde_json::json!({ "pattern": "^[a-z][a-z0-9_]*$" }),
        ));
    }
    if description.is_empty() {
        return Err(schema_error(
            "schema.required",
            "Relationship shorthand description is required.",
            path,
            serde_json::json!({ "required": "description" }),
        ));
    }

    let mut mapping = serde_yaml_ng::Mapping::new();
    mapping.insert(
        serde_yaml_ng::Value::String("from".to_string()),
        serde_yaml_ng::Value::String(from.to_string()),
    );
    mapping.insert(
        serde_yaml_ng::Value::String("to".to_string()),
        serde_yaml_ng::Value::String(to.to_string()),
    );
    mapping.insert(
        serde_yaml_ng::Value::String("description".to_string()),
        serde_yaml_ng::Value::String(description.to_string()),
    );
    mapping.insert(
        serde_yaml_ng::Value::String("status".to_string()),
        serde_yaml_ng::Value::String("live".to_string()),
    );
    mapping.insert(
        serde_yaml_ng::Value::String("generated".to_string()),
        serde_yaml_ng::Value::Bool(false),
    );

    Ok(serde_yaml_ng::Value::Mapping(mapping))
}

fn normalize_technology_aliases(
    value: &mut serde_yaml_ng::Value,
    path: &str,
) -> Result<(), CommandError> {
    match value {
        serde_yaml_ng::Value::Mapping(mapping) => {
            let technology = mapping.get("technology").cloned();
            if let Some(technology) = technology {
                if let Some(tech) = mapping.get("tech") {
                    if tech != &technology {
                        return Err(CommandError::with_details(
                            "semantic.conflicting_technology_alias",
                            "`tech` and `technology` must not disagree.",
                            serde_json::json!({ "path": path }),
                        ));
                    }
                }

                mapping.shift_remove("technology");
                if !mapping.contains_key("tech") {
                    mapping.insert(serde_yaml_ng::Value::String("tech".to_string()), technology);
                }
            }

            for (key, value) in mapping.iter_mut() {
                let key = yaml_string_value(key).unwrap_or("<non-string>");
                normalize_technology_aliases(value, &format!("{path}/{key}"))?;
            }
        }
        serde_yaml_ng::Value::Sequence(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                normalize_technology_aliases(item, &format!("{path}/{index}"))?;
            }
        }
        serde_yaml_ng::Value::Tagged(tagged) => {
            normalize_technology_aliases(&mut tagged.value, path)?;
        }
        serde_yaml_ng::Value::Null
        | serde_yaml_ng::Value::Bool(_)
        | serde_yaml_ng::Value::Number(_)
        | serde_yaml_ng::Value::String(_) => {}
    }

    Ok(())
}

fn validate_top_level_model_schema(value: &serde_yaml_ng::Value) -> Result<(), CommandError> {
    let serde_yaml_ng::Value::Mapping(mapping) = value else {
        return Err(schema_error(
            "schema.invalid_type",
            "Model document must be an object.",
            "",
            serde_json::json!({ "expected": "object" }),
        ));
    };

    let mut has_name = false;
    for (key, value) in mapping {
        let Some(key) = yaml_string_value(key) else {
            return Err(schema_error(
                "schema.invalid_type",
                "Model property names must be strings.",
                "",
                serde_json::json!({ "expected": "string" }),
            ));
        };

        match key {
            "name" => {
                has_name = true;
                validate_required_string(value, "/name", "Model name is required.")?;
            }
            "description" => {
                validate_string(value, "/description", "Model description must be a string.")?
            }
            "generated" => validate_bool(
                value,
                "/generated",
                "Model generated flag must be a boolean.",
            )?,
            "actors" => validate_mapping(value, "/actors", "Model actors must be an object.")?,
            "systems" => validate_mapping(value, "/systems", "Model systems must be an object.")?,
            "relationships" => validate_sequence(
                value,
                "/relationships",
                "Model relationships must be an array.",
            )?,
            other => {
                return Err(schema_error(
                    "schema.additional_property",
                    "Model contains an unsupported top-level property.",
                    format!("/{other}"),
                    serde_json::json!({ "property": other }),
                ));
            }
        }
    }

    if !has_name {
        return Err(schema_error(
            "schema.required",
            "Model name is required.",
            "/name",
            serde_json::json!({ "required": "name" }),
        ));
    }

    if let Some(actors) = mapping_value(mapping, "actors") {
        validate_actor_map(actors, "/actors")?;
    }
    if let Some(systems) = mapping_value(mapping, "systems") {
        validate_system_map(systems, "/systems")?;
    }
    if let Some(relationships) = mapping_value(mapping, "relationships") {
        validate_relationship_items(relationships, "/relationships")?;
    }

    Ok(())
}

fn validate_actor_map(value: &serde_yaml_ng::Value, path: &str) -> Result<(), CommandError> {
    let mapping = expect_mapping(value, path, "Actors must be an object.")?;
    for (slug, actor) in mapping {
        let slug = validate_slug_key(slug, path)?;
        validate_base_element(actor, &format!("{path}/{slug}"), &[])?;
    }

    Ok(())
}

fn validate_system_map(value: &serde_yaml_ng::Value, path: &str) -> Result<(), CommandError> {
    let mapping = expect_mapping(value, path, "Systems must be an object.")?;
    for (slug, system) in mapping {
        let slug = validate_slug_key(slug, path)?;
        validate_system(system, &format!("{path}/{slug}"))?;
    }

    Ok(())
}

fn validate_container_map(value: &serde_yaml_ng::Value, path: &str) -> Result<(), CommandError> {
    let mapping = expect_mapping(value, path, "Containers must be an object.")?;
    for (slug, container) in mapping {
        let slug = validate_slug_key(slug, path)?;
        validate_container(container, &format!("{path}/{slug}"))?;
    }

    Ok(())
}

fn validate_component_map(value: &serde_yaml_ng::Value, path: &str) -> Result<(), CommandError> {
    let mapping = expect_mapping(value, path, "Components must be an object.")?;
    for (slug, component) in mapping {
        let slug = validate_slug_key(slug, path)?;
        validate_base_element(component, &format!("{path}/{slug}"), &[])?;
    }

    Ok(())
}

fn validate_system(value: &serde_yaml_ng::Value, path: &str) -> Result<(), CommandError> {
    let mapping = validate_base_element(value, path, &["external", "containers"])?;
    for (key, value) in mapping {
        let key = yaml_string_value(key).expect("base element validator checked keys");
        match key {
            "external" => validate_bool(
                value,
                &format!("{path}/external"),
                "System external flag must be a boolean.",
            )?,
            "containers" => validate_container_map(value, &format!("{path}/containers"))?,
            _ => {}
        }
    }

    Ok(())
}

fn validate_container(value: &serde_yaml_ng::Value, path: &str) -> Result<(), CommandError> {
    let mapping = validate_base_element(value, path, &["kind", "components"])?;
    for (key, value) in mapping {
        let key = yaml_string_value(key).expect("base element validator checked keys");
        match key {
            "kind" => validate_enum(
                value,
                &format!("{path}/kind"),
                "Container kind is unsupported.",
                &["service", "app", "store", "queue", "worker", "job"],
            )?,
            "components" => validate_component_map(value, &format!("{path}/components"))?,
            _ => {}
        }
    }

    Ok(())
}

fn validate_relationship_items(
    value: &serde_yaml_ng::Value,
    path: &str,
) -> Result<(), CommandError> {
    let serde_yaml_ng::Value::Sequence(relationships) = value else {
        return Err(schema_error(
            "schema.invalid_type",
            "Model relationships must be an array.",
            path,
            serde_json::json!({ "expected": "array" }),
        ));
    };

    for (index, relationship) in relationships.iter().enumerate() {
        validate_relationship_schema(relationship, &format!("{path}/{index}"))?;
    }

    Ok(())
}

fn validate_relationship_schema(
    value: &serde_yaml_ng::Value,
    path: &str,
) -> Result<(), CommandError> {
    let mapping = expect_mapping(value, path, "Relationship must be an object.")?;
    let mut has_from = false;
    let mut has_to = false;
    let mut has_description = false;

    for (key, value) in mapping {
        let Some(key) = yaml_string_value(key) else {
            return Err(schema_error(
                "schema.invalid_type",
                "Relationship property names must be strings.",
                path,
                serde_json::json!({ "expected": "string" }),
            ));
        };

        match key {
            "from" => {
                has_from = true;
                validate_slug_value(
                    value,
                    &format!("{path}/from"),
                    "Relationship source must be a valid slug.",
                )?;
            }
            "to" => {
                has_to = true;
                validate_slug_value(
                    value,
                    &format!("{path}/to"),
                    "Relationship target must be a valid slug.",
                )?;
            }
            "description" => {
                has_description = true;
                validate_required_string(
                    value,
                    &format!("{path}/description"),
                    "Relationship description is required.",
                )?;
            }
            "tech" | "technology" => validate_string(
                value,
                &format!("{path}/{key}"),
                "Relationship technology must be a string.",
            )?,
            "status" => validate_enum(
                value,
                &format!("{path}/status"),
                "Relationship status is unsupported.",
                &["live", "planned", "deprecated"],
            )?,
            "generated" => validate_bool(
                value,
                &format!("{path}/generated"),
                "Relationship generated flag must be a boolean.",
            )?,
            other => {
                return Err(schema_error(
                    "schema.additional_property",
                    "Relationship contains an unsupported property.",
                    format!("{path}/{other}"),
                    serde_json::json!({ "property": other }),
                ));
            }
        }
    }

    if !has_from {
        return Err(schema_error(
            "schema.required",
            "Relationship source is required.",
            format!("{path}/from"),
            serde_json::json!({ "required": "from" }),
        ));
    }
    if !has_to {
        return Err(schema_error(
            "schema.required",
            "Relationship target is required.",
            format!("{path}/to"),
            serde_json::json!({ "required": "to" }),
        ));
    }
    if !has_description {
        return Err(schema_error(
            "schema.required",
            "Relationship description is required.",
            format!("{path}/description"),
            serde_json::json!({ "required": "description" }),
        ));
    }

    Ok(())
}

fn validate_base_element<'a>(
    value: &'a serde_yaml_ng::Value,
    path: &str,
    allowed_extra_keys: &[&str],
) -> Result<&'a serde_yaml_ng::Mapping, CommandError> {
    let mapping = expect_mapping(value, path, "Element must be an object.")?;
    let mut has_name = false;

    for (key, value) in mapping {
        let Some(key) = yaml_string_value(key) else {
            return Err(schema_error(
                "schema.invalid_type",
                "Element property names must be strings.",
                path,
                serde_json::json!({ "expected": "string" }),
            ));
        };

        match key {
            "name" => {
                has_name = true;
                validate_required_string(
                    value,
                    &format!("{path}/name"),
                    "Element name is required.",
                )?;
            }
            "description" => validate_string(
                value,
                &format!("{path}/description"),
                "Element description must be a string.",
            )?,
            "tech" | "technology" => validate_string(
                value,
                &format!("{path}/{key}"),
                "Element technology must be a string.",
            )?,
            "status" => validate_enum(
                value,
                &format!("{path}/status"),
                "Element status is unsupported.",
                &["live", "planned", "deprecated"],
            )?,
            "code" => validate_required_string(
                value,
                &format!("{path}/code"),
                "Element code path must be a non-empty string.",
            )?,
            "generated" => validate_bool(
                value,
                &format!("{path}/generated"),
                "Element generated flag must be a boolean.",
            )?,
            allowed if allowed_extra_keys.contains(&allowed) => {}
            other => {
                return Err(schema_error(
                    "schema.additional_property",
                    "Element contains an unsupported property.",
                    format!("{path}/{other}"),
                    serde_json::json!({ "property": other }),
                ));
            }
        }
    }

    if !has_name {
        return Err(schema_error(
            "schema.required",
            "Element name is required.",
            format!("{path}/name"),
            serde_json::json!({ "required": "name" }),
        ));
    }

    Ok(mapping)
}

fn validate_string(
    value: &serde_yaml_ng::Value,
    path: &str,
    message: &str,
) -> Result<(), CommandError> {
    if yaml_string_value(value).is_some() {
        return Ok(());
    }

    Err(schema_error(
        "schema.invalid_type",
        message,
        path,
        serde_json::json!({ "expected": "string" }),
    ))
}

fn validate_required_string(
    value: &serde_yaml_ng::Value,
    path: &str,
    message: &str,
) -> Result<(), CommandError> {
    let Some(value) = yaml_string_value(value) else {
        return Err(schema_error(
            "schema.invalid_type",
            message,
            path,
            serde_json::json!({ "expected": "string" }),
        ));
    };

    if value.is_empty() {
        return Err(schema_error(
            "schema.required",
            message,
            path,
            serde_json::json!({ "minLength": 1 }),
        ));
    }

    Ok(())
}

fn validate_bool(
    value: &serde_yaml_ng::Value,
    path: &str,
    message: &str,
) -> Result<(), CommandError> {
    if matches!(value, serde_yaml_ng::Value::Bool(_)) {
        return Ok(());
    }

    Err(schema_error(
        "schema.invalid_type",
        message,
        path,
        serde_json::json!({ "expected": "boolean" }),
    ))
}

fn validate_mapping(
    value: &serde_yaml_ng::Value,
    path: &str,
    message: &str,
) -> Result<(), CommandError> {
    if matches!(value, serde_yaml_ng::Value::Mapping(_)) {
        return Ok(());
    }

    Err(schema_error(
        "schema.invalid_type",
        message,
        path,
        serde_json::json!({ "expected": "object" }),
    ))
}

fn validate_sequence(
    value: &serde_yaml_ng::Value,
    path: &str,
    message: &str,
) -> Result<(), CommandError> {
    if matches!(value, serde_yaml_ng::Value::Sequence(_)) {
        return Ok(());
    }

    Err(schema_error(
        "schema.invalid_type",
        message,
        path,
        serde_json::json!({ "expected": "array" }),
    ))
}

fn validate_enum(
    value: &serde_yaml_ng::Value,
    path: &str,
    message: &str,
    allowed: &[&str],
) -> Result<(), CommandError> {
    let Some(value) = yaml_string_value(value) else {
        return Err(schema_error(
            "schema.invalid_type",
            message,
            path,
            serde_json::json!({ "expected": "string" }),
        ));
    };

    if allowed.contains(&value) {
        return Ok(());
    }

    Err(schema_error(
        "schema.pattern",
        message,
        path,
        serde_json::json!({ "allowed": allowed }),
    ))
}

fn validate_slug_value(
    value: &serde_yaml_ng::Value,
    path: &str,
    message: &str,
) -> Result<(), CommandError> {
    let Some(slug) = yaml_string_value(value) else {
        return Err(schema_error(
            "schema.invalid_type",
            message,
            path,
            serde_json::json!({ "expected": "string" }),
        ));
    };

    if is_valid_slug(slug) {
        return Ok(());
    }

    Err(schema_error(
        "schema.pattern",
        message,
        path,
        serde_json::json!({ "pattern": "^[a-z][a-z0-9_]*$" }),
    ))
}

fn expect_mapping<'a>(
    value: &'a serde_yaml_ng::Value,
    path: &str,
    message: &str,
) -> Result<&'a serde_yaml_ng::Mapping, CommandError> {
    let serde_yaml_ng::Value::Mapping(mapping) = value else {
        return Err(schema_error(
            "schema.invalid_type",
            message,
            path,
            serde_json::json!({ "expected": "object" }),
        ));
    };

    Ok(mapping)
}

fn mapping_value<'a>(
    mapping: &'a serde_yaml_ng::Mapping,
    key: &str,
) -> Option<&'a serde_yaml_ng::Value> {
    mapping.iter().find_map(|(mapping_key, value)| {
        (yaml_string_value(mapping_key) == Some(key)).then_some(value)
    })
}

fn validate_slug_key<'a>(
    key: &'a serde_yaml_ng::Value,
    path: &str,
) -> Result<&'a str, CommandError> {
    let Some(slug) = yaml_string_value(key) else {
        return Err(schema_error(
            "schema.invalid_type",
            "Slug keys must be strings.",
            path,
            serde_json::json!({ "expected": "string" }),
        ));
    };

    if is_valid_slug(slug) {
        return Ok(slug);
    }

    Err(schema_error(
        "schema.pattern",
        "Slug does not match the required pattern.",
        format!("{path}/{slug}"),
        serde_json::json!({ "pattern": "^[a-z][a-z0-9_]*$" }),
    ))
}

fn is_valid_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn yaml_string_value(value: &serde_yaml_ng::Value) -> Option<&str> {
    match value {
        serde_yaml_ng::Value::String(value) => Some(value),
        _ => None,
    }
}

fn schema_error(
    code: &'static str,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
) -> CommandError {
    let path = path.into();
    let mut details = details;
    if let Some(details) = details.as_object_mut() {
        details.insert("path".to_string(), serde_json::Value::String(path));
    } else {
        details = serde_json::json!({
            "path": path,
            "details": details,
        });
    }

    CommandError::with_details(code, message.into(), details)
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

    #[test]
    fn rejects_missing_required_model_name_as_schema_error() {
        let root = fresh_test_dir("schema-missing-name");
        write_model(
            &root,
            r#"
actors:
  customer:
    name: Customer
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("missing name should fail");

        assert_eq!(error.code, "schema.required");

        cleanup(root);
    }

    #[test]
    fn rejects_non_string_model_name_as_schema_error() {
        let root = fresh_test_dir("schema-invalid-name-type");
        write_model(&root, "name: 42\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("invalid name should fail");

        assert_eq!(error.code, "schema.invalid_type");

        cleanup(root);
    }

    #[test]
    fn rejects_top_level_additional_property_as_schema_error() {
        let root = fresh_test_dir("schema-additional-property");
        write_model(
            &root,
            r#"
name: Additional Property
unexpected: true
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("additional key should fail");

        assert_eq!(error.code, "schema.additional_property");

        cleanup(root);
    }

    #[test]
    fn rejects_top_level_description_type_as_schema_error() {
        let root = fresh_test_dir("schema-description-type");
        write_model(&root, "name: Invalid Description\ndescription: false\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("description should fail");

        assert_eq!(error.code, "schema.invalid_type");

        cleanup(root);
    }

    #[test]
    fn rejects_top_level_generated_type_as_schema_error() {
        let root = fresh_test_dir("schema-generated-type");
        write_model(&root, "name: Invalid Generated\ngenerated: yes please\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("generated should fail");

        assert_eq!(error.code, "schema.invalid_type");

        cleanup(root);
    }

    #[test]
    fn rejects_top_level_collection_types_as_schema_error() {
        let root = fresh_test_dir("schema-collection-types");
        write_model(&root, "name: Invalid Collections\nactors: []\n");

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("actors should fail");

        assert_eq!(error.code, "schema.invalid_type");

        cleanup(root);
    }

    #[test]
    fn rejects_invalid_actor_slug_as_schema_pattern_error() {
        let root = fresh_test_dir("schema-invalid-actor-slug");
        write_model(
            &root,
            r#"
name: Invalid Actor Slug
actors:
  Customer:
    name: Customer
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("actor slug should fail");

        assert_eq!(error.code, "schema.pattern");

        cleanup(root);
    }

    #[test]
    fn rejects_actor_missing_required_name_as_schema_error() {
        let root = fresh_test_dir("schema-actor-missing-name");
        write_model(
            &root,
            r#"
name: Actor Missing Name
actors:
  customer:
    description: Customer
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("actor name should fail");

        assert_eq!(error.code, "schema.required");

        cleanup(root);
    }

    #[test]
    fn rejects_actor_additional_property_as_schema_error() {
        let root = fresh_test_dir("schema-actor-additional-property");
        write_model(
            &root,
            r#"
name: Actor Additional Property
actors:
  customer:
    name: Customer
    external: false
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("actor extra key should fail");

        assert_eq!(error.code, "schema.additional_property");

        cleanup(root);
    }

    #[test]
    fn rejects_invalid_container_kind_as_schema_error() {
        let root = fresh_test_dir("schema-invalid-container-kind");
        write_model(
            &root,
            r#"
name: Invalid Container Kind
systems:
  banking:
    name: Banking
    containers:
      web:
        name: Web
        kind: spaceship
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("container kind should fail");

        assert_eq!(error.code, "schema.pattern");

        cleanup(root);
    }

    #[test]
    fn normalizes_element_technology_alias_to_tech() {
        let root = fresh_test_dir("schema-technology-alias");
        write_model(
            &root,
            r#"
name: Technology Alias
systems:
  banking:
    name: Banking
    technology: Rust
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("technology alias should load");

        assert_eq!(
            effective.model.systems["banking"].base.tech.as_deref(),
            Some("Rust")
        );

        cleanup(root);
    }

    #[test]
    fn rejects_conflicting_technology_aliases() {
        let root = fresh_test_dir("schema-conflicting-technology-alias");
        write_model(
            &root,
            r#"
name: Technology Alias Conflict
systems:
  banking:
    name: Banking
    tech: Rust
    technology: Go
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("conflict should fail");

        assert_eq!(error.code, "semantic.conflicting_technology_alias");

        cleanup(root);
    }

    #[test]
    fn normalizes_unquoted_relationship_shorthand_mapping() {
        let root = fresh_test_dir("relationship-shorthand-mapping");
        write_model(
            &root,
            r#"
name: Shorthand Mapping
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - customer -> banking: Views balances using
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("shorthand should load");

        assert_eq!(effective.relationships.len(), 1);
        assert_eq!(effective.relationships[0].from, "customer");
        assert_eq!(effective.relationships[0].to, "banking");
        assert_eq!(
            effective.relationships[0].description,
            "Views balances using"
        );

        cleanup(root);
    }

    #[test]
    fn normalizes_quoted_relationship_shorthand_string() {
        let root = fresh_test_dir("relationship-shorthand-string");
        write_model(
            &root,
            r#"
name: Shorthand String
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - "customer -> banking: Calls (JSON/HTTPS)"
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("shorthand should load");

        assert_eq!(effective.relationships.len(), 1);
        assert_eq!(effective.relationships[0].from, "customer");
        assert_eq!(effective.relationships[0].to, "banking");
        assert_eq!(effective.relationships[0].description, "Calls (JSON/HTTPS)");

        cleanup(root);
    }

    #[test]
    fn rejects_relationship_missing_required_description_as_schema_error() {
        let root = fresh_test_dir("relationship-missing-description");
        write_model(
            &root,
            r#"
name: Missing Relationship Description
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: customer
    to: banking
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("schema should fail");

        assert_eq!(error.code, "schema.required");

        cleanup(root);
    }

    #[test]
    fn rejects_relationship_invalid_endpoint_slug_as_schema_error() {
        let root = fresh_test_dir("relationship-invalid-endpoint-slug");
        write_model(
            &root,
            r#"
name: Invalid Relationship Endpoint
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: Customer
    to: banking
    description: Uses
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("schema should fail");

        assert_eq!(error.code, "schema.pattern");

        cleanup(root);
    }

    #[test]
    fn rejects_relationship_additional_property_as_schema_error() {
        let root = fresh_test_dir("relationship-additional-property");
        write_model(
            &root,
            r#"
name: Extra Relationship Property
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: customer
    to: banking
    description: Uses
    protocol: HTTPS
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("schema should fail");

        assert_eq!(error.code, "schema.additional_property");

        cleanup(root);
    }

    #[test]
    fn rejects_relationship_invalid_status_as_schema_error() {
        let root = fresh_test_dir("relationship-invalid-status");
        write_model(
            &root,
            r#"
name: Invalid Relationship Status
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: customer
    to: banking
    description: Uses
    status: retired
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("schema should fail");

        assert_eq!(error.code, "schema.pattern");

        cleanup(root);
    }

    #[test]
    fn rejects_relationship_generated_type_as_schema_error() {
        let root = fresh_test_dir("relationship-generated-type");
        write_model(
            &root,
            r#"
name: Invalid Relationship Generated Flag
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: customer
    to: banking
    description: Uses
    generated: "false"
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("schema should fail");

        assert_eq!(error.code, "schema.invalid_type");

        cleanup(root);
    }

    #[test]
    fn normalizes_relationship_technology_alias() {
        let root = fresh_test_dir("relationship-technology-alias");
        write_model(
            &root,
            r#"
name: Relationship Technology Alias
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: customer
    to: banking
    description: Uses
    technology: HTTPS
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("alias should load");

        assert_eq!(effective.relationships[0].tech.as_deref(), Some("HTTPS"));

        cleanup(root);
    }

    #[test]
    fn rejects_relationship_with_missing_source_slug() {
        let root = fresh_test_dir("missing-relationship-source");
        write_model(
            &root,
            r#"
name: Missing Source
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: missing
    to: banking
    description: Uses
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("missing source should fail");

        assert_eq!(error.code, "semantic.unresolved_relationship_source");

        cleanup(root);
    }

    #[test]
    fn rejects_relationship_with_missing_target_slug() {
        let root = fresh_test_dir("missing-relationship-target");
        write_model(
            &root,
            r#"
name: Missing Target
actors:
  customer:
    name: Customer
systems:
  banking:
    name: Banking
relationships:
  - from: customer
    to: missing
    description: Uses
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("missing target should fail");

        assert_eq!(error.code, "semantic.unresolved_relationship_target");

        cleanup(root);
    }

    #[test]
    fn rejects_invalid_code_path_syntax_as_semantic_error() {
        let root = fresh_test_dir("invalid-code-path");
        write_model(
            &root,
            r#"
name: Invalid Code Path
systems:
  banking:
    name: Banking
    code: src\banking
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("invalid code should fail");

        assert_eq!(error.code, "semantic.code_path_invalid");

        cleanup(root);
    }

    #[test]
    fn rejects_escaping_code_path_as_semantic_error() {
        let root = fresh_test_dir("escaping-code-path");
        write_model(
            &root,
            r#"
name: Escaping Code Path
systems:
  banking:
    name: Banking
    code: ../outside
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("escaping code should fail");

        assert_eq!(error.code, "semantic.code_path_outside_repo");

        cleanup(root);
    }

    #[test]
    fn records_missing_code_path_as_semantic_warning() {
        let root = fresh_test_dir("missing-code-path");
        write_model(
            &root,
            r#"
name: Missing Code Path
systems:
  banking:
    name: Banking
    code: src/banking
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("missing code is warning");

        assert!(effective.validation.ok);
        assert_eq!(effective.validation.issues.len(), 1);
        assert_eq!(
            effective.validation.issues[0].code,
            "semantic.code_path_missing"
        );

        cleanup(root);
    }

    #[test]
    fn accepts_existing_code_path_without_warning() {
        let root = fresh_test_dir("existing-code-path");
        fs::create_dir_all(root.join("src/banking")).expect("create code dir");
        write_model(
            &root,
            r#"
name: Existing Code Path
systems:
  banking:
    name: Banking
    code: src/banking
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let effective = load_effective_model_from_repo(repo).expect("existing code should load");

        assert!(effective.validation.ok);
        assert!(effective.validation.issues.is_empty());

        cleanup(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_code_path_symlink_that_resolves_outside_repo() {
        use std::os::unix::fs::symlink;

        let root = fresh_test_dir("escaping-code-symlink");
        let outside = fresh_test_dir("outside-code");
        fs::create_dir(root.join("src")).expect("create src dir");
        symlink(&outside, root.join("src/outside")).expect("create code symlink");
        write_model(
            &root,
            r#"
name: Escaping Code Symlink
systems:
  banking:
    name: Banking
    code: src/outside
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("escaping symlink should fail");

        assert_eq!(error.code, "semantic.code_path_outside_repo");

        cleanup(root);
        cleanup(outside);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_broken_code_path_symlink_that_points_outside_repo() {
        use std::os::unix::fs::symlink;

        let root = fresh_test_dir("broken-escaping-code-symlink");
        fs::create_dir(root.join("src")).expect("create src dir");
        symlink("../../outside/missing", root.join("src/broken")).expect("create broken symlink");
        write_model(
            &root,
            r#"
name: Broken Escaping Code Symlink
systems:
  banking:
    name: Banking
    code: src/broken
"#,
        );

        let repo = repo_handle_from_path(&root).expect("repo handle");
        let error = load_effective_model_from_repo(repo).expect_err("broken symlink should fail");

        assert_eq!(error.code, "semantic.code_path_outside_repo");

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
