import { describe, expect, it } from "vitest";
import { sampleModel } from "../model/sample";
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
});
