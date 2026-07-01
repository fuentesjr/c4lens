import type { EffectiveModel, ElementNode, Relationship, Slug } from "../model/types";

export type ViewScope =
  | { level: "context"; slug?: Slug }
  | { level: "container"; slug: Slug }
  | { level: "component"; slug: Slug };

export interface DerivedNode {
  id: Slug;
  element: ElementNode;
}

export interface DerivedEdge {
  id: string;
  source: Slug;
  target: Slug;
  label: string;
  generated: boolean;
  relationships: Relationship[];
}

export interface DerivedView {
  scope: ViewScope;
  nodes: DerivedNode[];
  edges: DerivedEdge[];
}

export function deriveView(model: EffectiveModel, scope: ViewScope): DerivedView {
  const elements = model.elementsBySlug;
  const visible = visibleNodeIds(model, scope);
  const edgeGroups = new Map<string, Relationship[]>();

  model.relationships.forEach((relationship) => {
    const source = projectEndpoint(elements[relationship.from], scope);
    const target = projectEndpoint(elements[relationship.to], scope);

    if (!source || !target || source === target || !visible.has(source) || !visible.has(target)) {
      return;
    }

    const key = `${source}\u0000${target}`;
    const group = edgeGroups.get(key) ?? [];
    group.push(relationship);
    edgeGroups.set(key, group);
  });

  const nodes = Array.from(visible)
    .map((id) => elements[id])
    .filter((element): element is ElementNode => Boolean(element))
    .sort(compareElements)
    .map((element) => ({ id: element.slug, element }));

  const edges = Array.from(edgeGroups.entries())
    .map(([key, relationships]) => {
      const [source, target] = key.split("\u0000");
      const descriptions = Array.from(new Set(relationships.map((item) => item.description))).sort();
      return {
        id: `${source}->${target}`,
        source,
        target,
        label: descriptions.length === 1 ? descriptions[0] : `${relationships.length} dependencies`,
        generated: relationships.every((item) => item.generated),
        relationships: [...relationships].sort(compareRelationships),
      };
    })
    .sort((left, right) => left.id.localeCompare(right.id));

  return { scope, nodes, edges };
}

export function defaultScope(model: EffectiveModel): ViewScope {
  const firstSystem = availableSystems(model)[0];
  return firstSystem ? { level: "context" } : { level: "context" };
}

export function availableSystems(model: EffectiveModel): ElementNode[] {
  return Object.values(model.elementsBySlug)
    .filter((element) => element.type === "system" && !element.external)
    .sort(compareElements);
}

export function availableContainers(model: EffectiveModel, systemSlug: Slug): ElementNode[] {
  return Object.values(model.elementsBySlug)
    .filter((element) => element.type === "container" && element.systemSlug === systemSlug)
    .sort(compareElements);
}

function visibleNodeIds(model: EffectiveModel, scope: ViewScope): Set<Slug> {
  const elements = Object.values(model.elementsBySlug);
  const visible = new Set<Slug>();

  if (scope.level === "context") {
    if (scope.slug) {
      const scopedSystem = model.elementsBySlug[scope.slug];
      if (scopedSystem?.type !== "system") {
        return visible;
      }

      visible.add(scopedSystem.slug);
      model.relationships.forEach((relationship) => {
        const source = projectEndpoint(model.elementsBySlug[relationship.from], scope);
        const target = projectEndpoint(model.elementsBySlug[relationship.to], scope);

        if (!source || !target || source === target) {
          return;
        }

        if (source === scopedSystem.slug) {
          visible.add(target);
        } else if (target === scopedSystem.slug) {
          visible.add(source);
        }
      });
      return visible;
    }

    elements.forEach((element) => {
      if (element.type === "actor" || element.type === "system") {
        visible.add(element.slug);
      }
    });
    return visible;
  }

  if (scope.level === "container") {
    elements.forEach((element) => {
      if (element.type === "container" && element.systemSlug === scope.slug) {
        visible.add(element.slug);
      }
    });

    addRelationshipNeighbors(model, scope, visible);
    return visible;
  }

  const container = model.elementsBySlug[scope.slug];
  if (container?.type === "container") {
    visible.add(container.slug);
  }

  elements.forEach((element) => {
    if (element.type === "component" && element.containerSlug === scope.slug) {
      visible.add(element.slug);
    }
  });

  addRelationshipNeighbors(model, scope, visible);
  return visible;
}

function addRelationshipNeighbors(model: EffectiveModel, scope: ViewScope, visible: Set<Slug>) {
  const elements = model.elementsBySlug;
  let changed = true;

  while (changed) {
    changed = false;
    model.relationships.forEach((relationship) => {
      const source = projectEndpoint(elements[relationship.from], scope);
      const target = projectEndpoint(elements[relationship.to], scope);

      if (!source || !target || source === target) {
        return;
      }

      if (visible.has(source) && !visible.has(target)) {
        visible.add(target);
        changed = true;
      }

      if (visible.has(target) && !visible.has(source)) {
        visible.add(source);
        changed = true;
      }
    });
  }
}

function projectEndpoint(element: ElementNode | undefined, scope: ViewScope): Slug | null {
  if (!element) {
    return null;
  }

  if (scope.level === "context") {
    if (element.type === "actor" || element.type === "system") {
      return element.slug;
    }
    return element.systemSlug ?? null;
  }

  if (scope.level === "container") {
    if (element.type === "actor") {
      return element.slug;
    }
    if (element.type === "system") {
      return element.slug === scope.slug || element.external ? element.slug : null;
    }
    if (element.systemSlug === scope.slug) {
      return element.type === "component" ? element.containerSlug ?? null : element.slug;
    }
    return element.systemSlug ?? element.slug;
  }

  if (element.type === "actor" || element.external) {
    return element.slug;
  }
  if (element.type === "component" && element.containerSlug === scope.slug) {
    return element.slug;
  }
  if (element.type === "container" && element.slug === scope.slug) {
    return element.slug;
  }
  if (element.type === "component") {
    return element.containerSlug ?? element.systemSlug ?? null;
  }
  return element.slug;
}

function compareElements(left: ElementNode, right: ElementNode): number {
  const typeOrder = new Map([
    ["actor", 0],
    ["system", 1],
    ["container", 2],
    ["component", 3],
  ]);
  return (
    (typeOrder.get(left.type) ?? 99) - (typeOrder.get(right.type) ?? 99) ||
    left.name.localeCompare(right.name) ||
    left.slug.localeCompare(right.slug)
  );
}

function compareRelationships(left: Relationship, right: Relationship): number {
  return (
    left.from.localeCompare(right.from) ||
    left.to.localeCompare(right.to) ||
    left.description.localeCompare(right.description)
  );
}
