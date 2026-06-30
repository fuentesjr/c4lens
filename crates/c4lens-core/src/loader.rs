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
    let mut model: Model = serde_yaml_ng::from_str(&contents).map_err(|error| {
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
