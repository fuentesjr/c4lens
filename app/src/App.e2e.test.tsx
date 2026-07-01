/** @vitest-environment jsdom */
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

type MockFlowNode = {
  id: string;
  data?: { label?: string };
};

type MockDerivedNode = {
  id: string;
  element: {
    name: string;
    type: string;
    external?: boolean;
    kind?: string;
    description?: string;
    tech?: string;
    generated: boolean;
    status: string;
  };
};

type MockDerivedEdge = {
  id: string;
  source: string;
  target: string;
};

vi.mock("./model/sample", async () => {
  const actual = await vi.importActual<typeof import("./model/sample")>("./model/sample");

  return {
    ...actual,
    sampleModel: {
      ...actual.sampleModel,
      validation: {
        ...actual.sampleModel.validation,
        issues: actual.sampleModel.validation.issues.map((issue) => ({
          ...issue,
          path: issue.path ?? "/sample/model",
        })),
      },
    },
  };
});

vi.mock("@xyflow/react", async () => {
  const actual = await vi.importActual<typeof import("@xyflow/react")>("@xyflow/react");

  return {
    ...actual,
    ReactFlow: ({
      nodes,
      onNodeClick,
      onNodeDoubleClick,
      children,
    }: {
      nodes: MockFlowNode[];
      onNodeClick?: (event: MouseEvent, node: MockFlowNode) => void;
      onNodeDoubleClick?: (event: MouseEvent, node: MockFlowNode) => void;
      children: ReactNode;
    }) => (
      <div>
        <div aria-label="Flow canvas">
          {nodes.map((node) => (
            <button
              key={node.id}
              type="button"
              data-node-id={node.id}
              onClick={(event) => {
                onNodeClick?.(event as unknown as MouseEvent, node);
              }}
              onDoubleClick={(event) => {
                onNodeDoubleClick?.(event as unknown as MouseEvent, node);
              }}
            >
              {node.data?.label ?? node.id}
            </button>
          ))}
        </div>
        {children}
      </div>
    ),
    Background: () => null,
    Controls: () => null,
    MiniMap: () => null,
    useNodesState: actual.useNodesState,
    useEdgesState: actual.useEdgesState,
  };
});

vi.mock("./layout/elkLayout", () => ({
  layoutWithElk: vi.fn(async (view: { nodes: MockDerivedNode[]; edges: MockDerivedEdge[] }) => {
    return {
      nodes: view.nodes.map((item, index) => ({
        id: item.id,
        type: "c4Node",
        position: { x: 0, y: index * 120 },
        width: 220,
        height: 96,
        data: {
          label: item.element.name,
          elementType: item.element.type,
          subtitle: item.element.external ? "external system" : item.element.kind ?? item.element.type,
          description: item.element.description,
          tech: item.element.tech,
          generated: item.element.generated,
          external: Boolean(item.element.external),
          status: item.element.status,
        },
      })),
      edges: view.edges.map((edge) => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
      })),
    };
  }),
}));

function mountApp(): { container: HTMLElement; cleanup: () => void } {
  const container = document.createElement("div");
  const root = createRoot(container);
  act(() => {
    root.render(<App />);
  });

  document.body.appendChild(container);

  return {
    container,
    cleanup: () => {
      act(() => {
        root.unmount();
      });
      container.remove();
    },
  };
}

async function flushLayout() {
  await act(async () => {
    await Promise.resolve();
  });
}

function canvasButtons(container: HTMLElement) {
  const canvasRegion = container.querySelector('[aria-label="Architecture canvas"]');
  if (!canvasRegion) {
    throw new Error("Architecture canvas not found");
  }

  return Array.from(canvasRegion.querySelectorAll<HTMLButtonElement>("button[data-node-id]"));
}

function getCanvasNode(container: HTMLElement, label: string): HTMLButtonElement | null {
  return canvasButtons(container).find((button) => button.textContent?.trim() === label) ?? null;
}

function getCanvasLabels(container: HTMLElement): string[] {
  return canvasButtons(container).map((button) => button.textContent?.trim() ?? "");
}

function getDetailAction(container: HTMLElement, label: string): HTMLButtonElement | null {
  const panel = container.querySelector("aside.detail-panel");
  if (!panel) {
    throw new Error("Detail panel not found");
  }

  return (
    Array.from(panel.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === label,
    ) ?? null
  );
}

describe("App drill-down renderer behavior", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("double-clicks a system node to open that system's container view", async () => {
    const { container, cleanup } = mountApp();
    await flushLayout();

    const systemNode = getCanvasNode(container, "Acme API");
    expect(systemNode).not.toBeNull();

    act(() => {
      systemNode!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });
    await flushLayout();

    const labels = getCanvasLabels(container);
    expect(labels).toContain("API Server");
    expect(labels).toContain("Event Bus");
    expect(labels).not.toContain("Acme API");

    cleanup();
  });

  it("double-clicks a container node to open that container's component view", async () => {
    const { container, cleanup } = mountApp();
    await flushLayout();

    const systemNode = getCanvasNode(container, "Acme API");
    expect(systemNode).not.toBeNull();

    act(() => {
      systemNode!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });
    await flushLayout();

    const containerNode = getCanvasNode(container, "API Server");
    expect(containerNode).not.toBeNull();

    act(() => {
      containerNode!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });
    await flushLayout();

    const labels = getCanvasLabels(container);
    expect(labels).toContain("Authentication Component");
    expect(labels).toContain("Billing Component");
    expect(labels).not.toContain("Acme API");

    cleanup();
  });

  it("exposes a detail-panel action for the selected drillable node and matches double-click behavior", async () => {
    const { container, cleanup } = mountApp();
    await flushLayout();

    const systemNode = getCanvasNode(container, "Acme API");
    expect(systemNode).not.toBeNull();

    act(() => {
      systemNode!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    const drillAction = getDetailAction(container, "Open containers");
    expect(drillAction).not.toBeNull();

    act(() => {
      drillAction!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    const labels = getCanvasLabels(container);
    expect(labels).toContain("API Server");
    expect(labels).toContain("Event Bus");

    cleanup();
  });

  it("exposes a detail-panel action for selected containers", async () => {
    const { container, cleanup } = mountApp();
    await flushLayout();

    const systemNode = getCanvasNode(container, "Acme API");
    expect(systemNode).not.toBeNull();

    act(() => {
      systemNode!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });
    await flushLayout();

    const containerNode = getCanvasNode(container, "API Server");
    expect(containerNode).not.toBeNull();

    act(() => {
      containerNode!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    const drillAction = getDetailAction(container, "Open components");
    expect(drillAction).not.toBeNull();

    act(() => {
      drillAction!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    const labels = getCanvasLabels(container);
    expect(labels).toContain("Authentication Component");
    expect(labels).toContain("Billing Component");

    cleanup();
  });

  it("does not render a detail-panel drill action for non-drillable nodes", async () => {
    const { container, cleanup } = mountApp();
    await flushLayout();

    const actorNode = getCanvasNode(container, "Customer");
    expect(actorNode).not.toBeNull();

    act(() => {
      actorNode!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    expect(getDetailAction(container, "Open containers")).toBeNull();
    expect(getDetailAction(container, "Open components")).toBeNull();

    cleanup();
  });

  it("does not drill into external systems", async () => {
    const { container, cleanup } = mountApp();
    await flushLayout();

    const externalSystemNode = getCanvasNode(container, "Payments");
    expect(externalSystemNode).not.toBeNull();

    act(() => {
      externalSystemNode!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    expect(getDetailAction(container, "Open containers")).toBeNull();

    act(() => {
      externalSystemNode!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });
    await flushLayout();

    const labels = getCanvasLabels(container);
    expect(labels).toContain("Customer");
    expect(labels).toContain("Acme API");
    expect(labels).toContain("Payments");
    expect(labels).not.toContain("API Server");

    cleanup();
  });
});

describe("App validation warning surface", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("surfaces validation warnings without blocking the canvas", async () => {
    const { container, cleanup } = mountApp();
    await flushLayout();

    expect(container.querySelector(".statusbar")?.textContent).toContain("1 warning");
    expect(container.querySelector(".statusbar")?.textContent).not.toContain("Valid sample");
    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("schema.phase0_placeholder");
    expect(container.querySelector("aside.detail-panel")?.textContent).toContain(
      "No model files loaded yet. Using phase-0 sample model.",
    );
    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("/sample/model");
    expect(getCanvasLabels(container)).toEqual(["Customer", "Acme API", "Payments"]);

    cleanup();
  });
});
