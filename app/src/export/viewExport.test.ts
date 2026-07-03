import { describe, expect, it } from "vitest";
import type { Edge } from "@xyflow/react";
import type { C4FlowNode } from "../layout/elkLayout";
import { serializeViewToSvg } from "./viewExport";

describe("serializeViewToSvg", () => {
  it("renders nodes and edges as escaped SVG markup", () => {
    const nodes: C4FlowNode[] = [
      {
        id: "api",
        type: "c4Node",
        position: { x: 10, y: 20 },
        width: 220,
        height: 96,
        data: {
          label: "API <Core>",
          elementType: "container",
          subtitle: "service",
          tech: "Rust & Axum",
          generated: false,
          external: false,
          status: "live",
        },
      },
      {
        id: "db",
        type: "c4Node",
        position: { x: 320, y: 20 },
        width: 220,
        height: 96,
        data: {
          label: "Database",
          elementType: "container",
          subtitle: "store",
          tech: "SQLite",
          generated: true,
          external: false,
          status: "live",
        },
      },
    ];
    const edges: Edge[] = [
      {
        id: "api-db",
        source: "api",
        target: "db",
        label: "Reads & writes",
      },
    ];

    const serialized = serializeViewToSvg(nodes, edges, "Architecture <View>");

    expect(serialized.width).toBeGreaterThan(0);
    expect(serialized.height).toBeGreaterThan(0);
    expect(serialized.svg).toContain("<svg");
    expect(serialized.svg).toContain("Architecture &lt;View&gt;");
    expect(serialized.svg).toContain("API &lt;Core&gt;");
    expect(serialized.svg).toContain("Rust &amp; Axum");
    expect(serialized.svg).toContain("Reads &amp; writes");
    expect(serialized.svg).toContain('marker-end="url(#arrow)"');
  });
});
