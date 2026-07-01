import { describe, expect, it } from "vitest";
import { sampleModel } from "../model/sample";
import type { EffectiveModel, ElementNode, Relationship } from "../model/types";
import { deriveView } from "./deriveView";

describe("deriveView", () => {
  it("projects lower-level relationships to system boundaries in the context view", () => {
    const view = deriveView(sampleModel, { level: "context" });

    expect(view.nodes.map((node) => node.id)).toEqual(["customer", "acme_api", "payments"]);
    expect(view.edges.map((edge) => `${edge.source}->${edge.target}:${edge.label}`)).toEqual([
      "acme_api->payments:Charges",
      "customer->acme_api:Uses",
    ]);
  });

  it("renders containers, external dependencies, and actor neighbors for a system view", () => {
    const view = deriveView(sampleModel, { level: "container", slug: "acme_api" });

    expect(view.nodes.map((node) => node.id)).toEqual(["customer", "payments", "api_server", "event_bus"]);
    expect(view.edges.map((edge) => `${edge.source}->${edge.target}:${edge.label}`)).toEqual([
      "api_server->event_bus:Publishes",
      "api_server->payments:Charges",
      "customer->api_server:Uses",
    ]);
  });

  it("scopes context view to one system and direct projected actor/system neighbors", () => {
    const view = deriveView(scopedContextModel, { level: "context", slug: "acme_api" });

    expect(view.nodes.map((node) => node.id)).toEqual(["customer", "acme_api", "payments"]);
    expect(view.edges.map((edge) => `${edge.source}->${edge.target}:${edge.label}`)).toEqual([
      "acme_api->payments:Charges",
      "customer->acme_api:Uses",
      "payments->customer:Receives receipt",
    ]);
  });

  it("returns an empty view for invalid scoped context slugs", () => {
    const missing = deriveView(scopedContextModel, { level: "context", slug: "missing" });
    const wrongType = deriveView(scopedContextModel, { level: "context", slug: "api_server" });

    expect(missing.scope).toEqual({ level: "context", slug: "missing" });
    expect(missing.nodes).toEqual([]);
    expect(missing.edges).toEqual([]);
    expect(wrongType.scope).toEqual({ level: "context", slug: "api_server" });
    expect(wrongType.nodes).toEqual([]);
    expect(wrongType.edges).toEqual([]);
  });
});

const customer = element({
  slug: "customer",
  name: "Customer",
  type: "actor",
});
const acmeApi = element({
  slug: "acme_api",
  name: "Acme API",
  type: "system",
  systemSlug: "acme_api",
  external: false,
});
const payments = element({
  slug: "payments",
  name: "Payments",
  type: "system",
  systemSlug: "payments",
  external: true,
});
const analytics = element({
  slug: "analytics",
  name: "Analytics",
  type: "system",
  systemSlug: "analytics",
  external: true,
});
const apiServer = element({
  slug: "api_server",
  name: "API Server",
  type: "container",
  parentSlug: "acme_api",
  systemSlug: "acme_api",
  kind: "service",
});
const billing = element({
  slug: "billing",
  name: "Billing",
  type: "component",
  parentSlug: "api_server",
  systemSlug: "acme_api",
  containerSlug: "api_server",
});

const scopedContextRelationships: Relationship[] = [
  relationship("customer", "api_server", "Uses"),
  relationship("billing", "payments", "Charges"),
  relationship("payments", "customer", "Receives receipt"),
  relationship("payments", "analytics", "Reports"),
];

const scopedContextModel: EffectiveModel = {
  repo: {
    id: "scoped-context",
    rootPath: "",
    name: "Scoped Context",
  },
  sourceSha: "scoped-context",
  model: {
    name: "Scoped Context",
    actors: {
      customer,
    },
    systems: {
      acme_api: {
        ...acmeApi,
        external: false,
        containers: {
          api_server: {
            ...apiServer,
            kind: "service",
            components: {
              billing,
            },
          },
        },
      },
      payments: {
        ...payments,
        external: true,
        containers: {},
      },
      analytics: {
        ...analytics,
        external: true,
        containers: {},
      },
    },
    relationships: scopedContextRelationships,
    generated: false,
  },
  elementsBySlug: {
    customer,
    acme_api: acmeApi,
    payments,
    analytics,
    api_server: apiServer,
    billing,
  },
  relationships: scopedContextRelationships,
  validation: {
    ok: true,
    issues: [],
  },
};

function element(overrides: Partial<ElementNode> & Pick<ElementNode, "slug" | "name" | "type">): ElementNode {
  return {
    status: "live",
    generated: false,
    source: "authored",
    ...overrides,
  };
}

function relationship(from: string, to: string, description: string): Relationship {
  return {
    from,
    to,
    description,
    status: "live",
    generated: false,
  };
}
