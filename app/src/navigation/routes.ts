import type { EffectiveModel, Slug } from "../model/types";
import type { ViewScope } from "../view_deriver/deriveView";

export interface ParsedRoute {
  scope: ViewScope;
  selectedSlug: Slug | null;
}

export type RouteIssue =
  | {
      kind: "route";
      title: "Route not found";
      slug: string;
      message: string;
    }
  | {
      kind: "selection";
      title: "Component not found";
      slug: Slug;
      message: string;
    };

export interface ResolvedRoute {
  scope: ViewScope;
  selectedSlug: Slug | null;
  issue: RouteIssue | null;
}

export function parseHashRoute(hash: string): ParsedRoute | null {
  const segments = routeSegments(hash);

  if (segments.length === 0 || (segments.length === 1 && segments[0] === "context")) {
    return { scope: { level: "context" }, selectedSlug: null };
  }

  if (segments.length === 2 && segments[0] === "system") {
    return { scope: { level: "container", slug: segments[1] }, selectedSlug: null };
  }

  if (segments.length === 2 && segments[0] === "container") {
    return { scope: { level: "component", slug: segments[1] }, selectedSlug: null };
  }

  if (segments.length === 4 && segments[0] === "container" && segments[2] === "component") {
    return { scope: { level: "component", slug: segments[1] }, selectedSlug: segments[3] };
  }

  return null;
}

export function resolveHashRoute(hash: string, model: EffectiveModel): ResolvedRoute {
  const parsed = parseHashRoute(hash);

  if (!parsed) {
    const slug = hash.replace(/^#/, "") || "/";
    return {
      scope: { level: "context" },
      selectedSlug: null,
      issue: {
        kind: "route",
        title: "Route not found",
        slug,
        message: "No view route matches this path.",
      },
    };
  }

  const scopeIssue = validateScope(parsed.scope, model);
  if (scopeIssue) {
    return { scope: parsed.scope, selectedSlug: null, issue: scopeIssue };
  }

  if (!parsed.selectedSlug) {
    return { ...parsed, issue: null };
  }

  const selectedElement = model.elementsBySlug[parsed.selectedSlug];
  if (
    parsed.scope.level === "component" &&
    selectedElement?.type === "component" &&
    selectedElement.containerSlug === parsed.scope.slug
  ) {
    return { ...parsed, issue: null };
  }

  return {
    scope: parsed.scope,
    selectedSlug: null,
    issue: {
      kind: "selection",
      title: "Component not found",
      slug: parsed.selectedSlug,
      message: `Component "${parsed.selectedSlug}" is not in container "${parsed.scope.slug}".`,
    },
  };
}

export function buildHashRoute(scope: ViewScope, selectedSlug: Slug | null = null): string {
  if (scope.level === "context") {
    return "#/context";
  }

  if (scope.level === "container") {
    return `#/system/${encodeURIComponent(scope.slug)}`;
  }

  const containerRoute = `#/container/${encodeURIComponent(scope.slug)}`;
  return selectedSlug ? `${containerRoute}/component/${encodeURIComponent(selectedSlug)}` : containerRoute;
}

function validateScope(scope: ViewScope, model: EffectiveModel): RouteIssue | null {
  if (scope.level === "context") {
    return null;
  }

  if (scope.level === "container") {
    const system = model.elementsBySlug[scope.slug];
    if (system?.type === "system" && !system.external) {
      return null;
    }
    return notFound(scope.slug);
  }

  const container = model.elementsBySlug[scope.slug];
  return container?.type === "container" ? null : notFound(scope.slug);
}

function notFound(slug: string): RouteIssue {
  return {
    kind: "route",
    title: "Route not found",
    slug,
    message: `No view route exists for "${slug}".`,
  };
}

function routeSegments(hash: string): string[] {
  const path = hash.replace(/^#/, "").replace(/^\/+|\/+$/g, "");
  if (!path) {
    return [];
  }

  const segments = path.split("/");
  if (segments.some((segment) => segment.length === 0)) {
    return ["__invalid__"];
  }

  try {
    return segments.map((segment) => decodeURIComponent(segment));
  } catch {
    return ["__invalid__"];
  }
}
