import { describe, expect, it } from "vitest";
import { sampleModel } from "../model/sample";
import { buildHashRoute, parseHashRoute, resolveHashRoute } from "./routes";

describe("hash routes", () => {
  it("parses documented routes to view scopes and selection", () => {
    expect(parseHashRoute("#/context")).toEqual({
      scope: { level: "context" },
      selectedSlug: null,
    });
    expect(parseHashRoute("#/system/acme_api")).toEqual({
      scope: { level: "container", slug: "acme_api" },
      selectedSlug: null,
    });
    expect(parseHashRoute("#/container/api_server")).toEqual({
      scope: { level: "component", slug: "api_server" },
      selectedSlug: null,
    });
    expect(parseHashRoute("#/container/api_server/component/auth_component")).toEqual({
      scope: { level: "component", slug: "api_server" },
      selectedSlug: "auth_component",
    });
  });

  it("builds hash routes from scopes and component selection", () => {
    expect(buildHashRoute({ level: "context" })).toBe("#/context");
    expect(buildHashRoute({ level: "container", slug: "acme_api" })).toBe("#/system/acme_api");
    expect(buildHashRoute({ level: "component", slug: "api_server" })).toBe("#/container/api_server");
    expect(buildHashRoute({ level: "component", slug: "api_server" }, "auth_component")).toBe(
      "#/container/api_server/component/auth_component",
    );
  });

  it("marks wrong-type route slugs as route not found while preserving the requested scope", () => {
    expect(resolveHashRoute("#/system/customer", sampleModel)).toEqual({
      scope: { level: "container", slug: "customer" },
      selectedSlug: null,
      issue: {
        kind: "route",
        title: "Route not found",
        slug: "customer",
        message: 'No view route exists for "customer".',
      },
    });
  });

  it("marks non-child component selection as not found without invalidating the container view", () => {
    expect(resolveHashRoute("#/container/event_bus/component/auth_component", sampleModel)).toEqual({
      scope: { level: "component", slug: "event_bus" },
      selectedSlug: null,
      issue: {
        kind: "selection",
        title: "Component not found",
        slug: "auth_component",
        message: 'Component "auth_component" is not in container "event_bus".',
      },
    });
  });
});
