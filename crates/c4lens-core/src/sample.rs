use std::collections::BTreeMap;

use crate::model::{
    BaseElement, Container, ContainerKind, EffectiveModel, ElementNode, ElementType, Lifecycle,
    Model, Relationship, RepoHandle, SourceKind, System, ValidationIssue, ValidationReport,
    ValidationSeverity, ValidationStage,
};

const SAMPLE_SOURCE_SHA: &str = "sample-phase0-v1";

pub fn hardcoded_sample_model(repo: RepoHandle) -> EffectiveModel {
    let actors = sample_actors();
    let systems = sample_systems();
    let relationships = sample_relationships();
    let elements_by_slug = flatten_elements(&actors, &systems);

    let model = Model {
        name: "Acme Demo Platform".to_string(),
        description: Some("A phase-0 sample model for desktop onboarding.".to_string()),
        actors,
        systems,
        relationships: relationships.clone(),
        generated: false,
    };

    EffectiveModel {
        repo,
        source_sha: SAMPLE_SOURCE_SHA.to_string(),
        authored_path: None,
        generated_path: None,
        model: model.clone(),
        elements_by_slug,
        relationships,
        validation: ValidationReport {
            ok: true,
            source_sha: Some(SAMPLE_SOURCE_SHA.to_string()),
            issues: vec![sample_warning()],
        },
    }
}

fn sample_actors() -> BTreeMap<String, crate::model::Actor> {
    let mut actors = BTreeMap::new();

    actors.insert(
        "customer".to_string(),
        crate::model::Actor {
            base: BaseElement {
                slug: "customer".to_string(),
                name: "Customer".to_string(),
                description: Some("People using Acme services.".to_string()),
                tech: None,
                status: Lifecycle::Live,
                code: None,
                generated: false,
            },
        },
    );

    actors
}

fn sample_systems() -> BTreeMap<String, System> {
    let mut systems = BTreeMap::new();

    let api_component = System {
        base: BaseElement {
            slug: "acme_api".to_string(),
            name: "Acme API".to_string(),
            description: Some("HTTP API providing business capabilities.".to_string()),
            tech: Some("Rust".to_string()),
            status: Lifecycle::Live,
            code: Some("src/api".to_string()),
            generated: false,
        },
        external: false,
        containers: BTreeMap::from([
            (
                "api_server".to_string(),
                Container {
                    base: BaseElement {
                        slug: "api_server".to_string(),
                        name: "API Server".to_string(),
                        description: Some("Public request handler cluster.".to_string()),
                        tech: Some("Axum".to_string()),
                        status: Lifecycle::Live,
                        code: Some("src/api/server".to_string()),
                        generated: false,
                    },
                    kind: ContainerKind::Service,
                    components: BTreeMap::from([
                        (
                            "auth_component".to_string(),
                            crate::model::Component {
                                base: BaseElement {
                                    slug: "auth_component".to_string(),
                                    name: "Authentication Component".to_string(),
                                    description: Some(
                                        "Validates sessions and issues tokens.".to_string(),
                                    ),
                                    tech: Some("JWT".to_string()),
                                    status: Lifecycle::Live,
                                    code: Some("src/api/server/auth".to_string()),
                                    generated: false,
                                },
                            },
                        ),
                        (
                            "billing_component".to_string(),
                            crate::model::Component {
                                base: BaseElement {
                                    slug: "billing_component".to_string(),
                                    name: "Billing Component".to_string(),
                                    description: Some(
                                        "Invoices and subscription workflows.".to_string(),
                                    ),
                                    tech: Some("Rust".to_string()),
                                    status: Lifecycle::Live,
                                    code: Some("src/api/server/billing".to_string()),
                                    generated: false,
                                },
                            },
                        ),
                    ]),
                },
            ),
            (
                "event_bus".to_string(),
                Container {
                    base: BaseElement {
                        slug: "event_bus".to_string(),
                        name: "Event Bus".to_string(),
                        description: Some("Durable event stream for async work.".to_string()),
                        tech: Some("Redis".to_string()),
                        status: Lifecycle::Live,
                        code: Some("src/api/events".to_string()),
                        generated: false,
                    },
                    kind: ContainerKind::Queue,
                    components: BTreeMap::new(),
                },
            ),
        ]),
    };

    let data_store = System {
        base: BaseElement {
            slug: "payments".to_string(),
            name: "Payments".to_string(),
            description: Some("External payment provider integration.".to_string()),
            tech: Some("Stripe".to_string()),
            status: Lifecycle::Live,
            code: None,
            generated: false,
        },
        external: true,
        containers: BTreeMap::new(),
    };

    systems.insert("acme_api".to_string(), api_component);
    systems.insert("payments".to_string(), data_store);
    systems
}

fn sample_relationships() -> Vec<Relationship> {
    vec![
        Relationship {
            from: "customer".to_string(),
            to: "api_server".to_string(),
            description: "Uses".to_string(),
            tech: Some("HTTPS".to_string()),
            status: Lifecycle::Live,
            generated: false,
        },
        Relationship {
            from: "api_server".to_string(),
            to: "auth_component".to_string(),
            description: "Authenticates".to_string(),
            tech: None,
            status: Lifecycle::Live,
            generated: false,
        },
        Relationship {
            from: "api_server".to_string(),
            to: "billing_component".to_string(),
            description: "Triggers".to_string(),
            tech: None,
            status: Lifecycle::Live,
            generated: false,
        },
        Relationship {
            from: "billing_component".to_string(),
            to: "event_bus".to_string(),
            description: "Publishes".to_string(),
            tech: Some("Jobs".to_string()),
            status: Lifecycle::Live,
            generated: false,
        },
        Relationship {
            from: "api_server".to_string(),
            to: "payments".to_string(),
            description: "Charges".to_string(),
            tech: Some("REST API".to_string()),
            status: Lifecycle::Live,
            generated: false,
        },
    ]
}

fn flatten_elements(
    actors: &BTreeMap<String, crate::model::Actor>,
    systems: &BTreeMap<String, System>,
) -> BTreeMap<String, ElementNode> {
    let mut output = BTreeMap::new();

    for actor in actors.values() {
        output.insert(
            actor.base.slug.clone(),
            ElementNode {
                base: actor.base.clone(),
                element_type: ElementType::Actor,
                parent_slug: None,
                system_slug: None,
                container_slug: None,
                external: None,
                kind: None,
                source: SourceKind::Merged,
            },
        );
    }

    for system in systems.values() {
        output.insert(
            system.base.slug.clone(),
            ElementNode {
                base: system.base.clone(),
                element_type: ElementType::System,
                parent_slug: None,
                system_slug: Some(system.base.slug.clone()),
                container_slug: None,
                external: Some(system.external),
                kind: None,
                source: SourceKind::Merged,
            },
        );

        for container in system.containers.values() {
            output.insert(
                container.base.slug.clone(),
                ElementNode {
                    base: container.base.clone(),
                    element_type: ElementType::Container,
                    parent_slug: Some(system.base.slug.clone()),
                    system_slug: Some(system.base.slug.clone()),
                    container_slug: None,
                    external: Some(system.external),
                    kind: Some(container.kind.clone()),
                    source: SourceKind::Merged,
                },
            );

            for component in container.components.values() {
                output.insert(
                    component.base.slug.clone(),
                    ElementNode {
                        base: component.base.clone(),
                        element_type: ElementType::Component,
                        parent_slug: Some(container.base.slug.clone()),
                        system_slug: Some(system.base.slug.clone()),
                        container_slug: Some(container.base.slug.clone()),
                        external: Some(system.external),
                        kind: None,
                        source: SourceKind::Merged,
                    },
                );
            }
        }
    }

    output
}

fn sample_warning() -> ValidationIssue {
    ValidationIssue {
        severity: ValidationSeverity::Warning,
        stage: ValidationStage::Schema,
        code: "schema.phase0_placeholder".to_string(),
        message: "No model files loaded yet. Using phase-0 sample model.".to_string(),
        path: None,
        line: None,
        column: None,
    }
}
