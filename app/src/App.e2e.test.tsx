/** @vitest-environment jsdom */
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { sampleModel } from "./model/sample";
import type { EffectiveModel, ScanSummary, ValidationReport } from "./model/types";

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

const ipcMocks = vi.hoisted(() => {
  const state = {
    handlers: null as null | {
      onModelChanged: (payload: { repoId: string; sourceSha: string }) => void | Promise<void>;
      onValidationFailed: (payload: { repoId: string; validation: unknown }) => void | Promise<void>;
      onIndexUpdated?: (payload: { repoId: string; summary: ScanSummary }) => void | Promise<void>;
    },
    unlisten: vi.fn(),
  };

  return {
    state,
    fetchActiveModel: vi.fn<() => Promise<EffectiveModel | null>>(async () => null),
    isTauriDesktop: vi.fn(() => false),
    listenToModelEvents: vi.fn(async (handlers: NonNullable<typeof state.handlers>) => {
      state.handlers = handlers;
      return state.unlisten;
    }),
    openRepositoryFromDialog: vi.fn<
      () => Promise<{ repo: EffectiveModel["repo"]; model: EffectiveModel | null } | null>
    >(async () => null),
    scanCodebase: vi.fn<() => Promise<ScanSummary>>(async () => scanSummaryFor()),
  };
});

vi.mock("./ipc/client", () => ({
  fetchActiveModel: ipcMocks.fetchActiveModel,
  isTauriDesktop: ipcMocks.isTauriDesktop,
  listenToModelEvents: ipcMocks.listenToModelEvents,
  openRepositoryFromDialog: ipcMocks.openRepositoryFromDialog,
  scanCodebase: ipcMocks.scanCodebase,
}));

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

function resetDomAndRoute() {
  document.body.innerHTML = "";
  window.location.hash = "";
  ipcMocks.fetchActiveModel.mockReset();
  ipcMocks.fetchActiveModel.mockResolvedValue(null);
  ipcMocks.isTauriDesktop.mockReset();
  ipcMocks.isTauriDesktop.mockReturnValue(false);
  ipcMocks.listenToModelEvents.mockClear();
  ipcMocks.openRepositoryFromDialog.mockReset();
  ipcMocks.openRepositoryFromDialog.mockResolvedValue(null);
  ipcMocks.scanCodebase.mockReset();
  ipcMocks.scanCodebase.mockResolvedValue(scanSummaryFor());
  ipcMocks.state.handlers = null;
  ipcMocks.state.unlisten.mockClear();
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
    resetDomAndRoute();
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

describe("App hash route behavior", () => {
  afterEach(() => {
    resetDomAndRoute();
  });

  it("loads a system hash route into that system's container view", async () => {
    window.location.hash = "#/system/acme_api";

    const { container, cleanup } = mountApp();
    await flushLayout();

    const labels = getCanvasLabels(container);
    expect(labels).toContain("API Server");
    expect(labels).toContain("Event Bus");
    expect(labels).not.toContain("Acme API");

    cleanup();
  });

  it("shows a canvas not-found state for a wrong-type route slug without unloading the model", async () => {
    window.location.hash = "#/system/customer";

    const { container, cleanup } = mountApp();
    await flushLayout();

    const canvasText = container.querySelector('[aria-label="Architecture canvas"]')?.textContent;
    expect(canvasText).toContain("Route not found");
    expect(canvasText).toContain("customer");
    expect(container.querySelector(".statusbar")?.textContent).toContain("1 warning");

    cleanup();
  });

  it("keeps an unmatched route not-found state when the canvas is clicked", async () => {
    window.location.hash = "#/bogus";

    const { container, cleanup } = mountApp();
    await flushLayout();

    const customerNode = getCanvasNode(container, "Customer");
    expect(customerNode).not.toBeNull();

    act(() => {
      customerNode!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    const canvasText = container.querySelector('[aria-label="Architecture canvas"]')?.textContent;
    expect(canvasText).toContain("Route not found");
    expect(canvasText).toContain("/bogus");
    expect(container.querySelector("aside.detail-panel")?.textContent).not.toContain("Customer");
    expect(window.location.hash).toBe("#/bogus");

    cleanup();
  });

  it("keeps the container component view when a component route selects a non-child component", async () => {
    window.location.hash = "#/container/event_bus/component/auth_component";

    const { container, cleanup } = mountApp();
    await flushLayout();

    const canvasText = container.querySelector('[aria-label="Architecture canvas"]')?.textContent;
    expect(getCanvasLabels(container)).toContain("Event Bus");
    expect(canvasText).toContain("Component not found");
    expect(canvasText).toContain("auth_component");
    expect(container.querySelector("aside.detail-panel")?.textContent).not.toContain("Authentication Component");

    cleanup();
  });
});

describe("App validation warning surface", () => {
  afterEach(() => {
    resetDomAndRoute();
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

describe("App model event behavior", () => {
  afterEach(() => {
    resetDomAndRoute();
  });

  it("refetches and renders the model after a valid model-changed event", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);

    const { container, cleanup } = mountApp();
    await flushLayout();

    const changedModel = effectiveModelWithName("Changed Architecture");
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(changedModel);

    expect(ipcMocks.state.handlers).not.toBeNull();
    await act(async () => {
      await ipcMocks.state.handlers!.onModelChanged({
        repoId: changedModel.repo.id,
        sourceSha: changedModel.sourceSha,
      });
    });
    await flushLayout();

    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("Changed Architecture");
    expect(container.querySelector(".statusbar")?.textContent).toContain("Valid model");
    expect(container.querySelector(".statusbar")?.textContent).toContain("Model updated");

    cleanup();
  });

  it("keeps the last valid canvas and surfaces validation errors after validation-failed", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);

    const { container, cleanup } = mountApp();
    await flushLayout();
    const labelsBeforeFailure = getCanvasLabels(container);
    const validation = validationFailureReport("parse.invalid_yaml", "Failed to parse c4/model.yml.");

    expect(ipcMocks.state.handlers).not.toBeNull();
    await act(async () => {
      await ipcMocks.state.handlers!.onValidationFailed({
        repoId: sampleModel.repo.id,
        validation,
      });
    });
    await flushLayout();

    expect(getCanvasLabels(container)).toEqual(labelsBeforeFailure);
    expect(container.querySelector(".statusbar")?.textContent).toContain("Validation issues");
    expect(container.querySelector(".statusbar")?.textContent).toContain("Model validation failed");
    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("parse.invalid_yaml");
    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("Failed to parse c4/model.yml.");
    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("c4/model.yml");

    cleanup();
  });

  it("accepts later model-changed events after opening a repo with an invalid model", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);
    const invalidRepo = {
      id: "invalid-repo",
      rootPath: "/tmp/invalid-repo",
      name: "Invalid Repo",
    };
    ipcMocks.openRepositoryFromDialog.mockResolvedValueOnce({
      repo: invalidRepo,
      model: null,
    });

    const { container, cleanup } = mountApp();
    await flushLayout();
    const labelsBeforeOpen = getCanvasLabels(container);

    const openButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Open Folder",
    );
    expect(openButton).not.toBeNull();

    await act(async () => {
      openButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    expect(getCanvasLabels(container)).toEqual(labelsBeforeOpen);
    expect(container.querySelector(".statusbar")?.textContent).toContain("Watching Invalid Repo");
    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("model.load_failed");

    const recoveredModel = effectiveModelWithName("Recovered Architecture", invalidRepo.id);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(recoveredModel);

    expect(ipcMocks.state.handlers).not.toBeNull();
    await act(async () => {
      await ipcMocks.state.handlers!.onModelChanged({
        repoId: invalidRepo.id,
        sourceSha: recoveredModel.sourceSha,
      });
    });
    await flushLayout();

    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("Recovered Architecture");
    expect(container.querySelector(".statusbar")?.textContent).toContain("Model updated");

    cleanup();
  });

  it("ignores stale model-changed refetch results after the active repo changes", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);
    let resolveFetch: (model: EffectiveModel) => void = () => {};
    ipcMocks.fetchActiveModel.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveFetch = resolve;
      }),
    );
    const nextRepo = {
      id: "next-repo",
      rootPath: "/tmp/next-repo",
      name: "Next Repo",
    };
    ipcMocks.openRepositoryFromDialog.mockResolvedValueOnce({
      repo: nextRepo,
      model: effectiveModelWithName("Next Repo Architecture", nextRepo.id),
    });

    const { container, cleanup } = mountApp();
    await flushLayout();

    expect(ipcMocks.state.handlers).not.toBeNull();
    let modelChangedPromise: void | Promise<void>;
    await act(async () => {
      modelChangedPromise = ipcMocks.state.handlers!.onModelChanged({
        repoId: sampleModel.repo.id,
        sourceSha: "stale-source-sha",
      });
      await Promise.resolve();
    });

    const openButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Open Folder",
    );
    expect(openButton).not.toBeNull();

    await act(async () => {
      openButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
      resolveFetch(effectiveModelWithName("Stale Repo Architecture", sampleModel.repo.id));
      await modelChangedPromise;
    });
    await flushLayout();

    expect(container.querySelector("aside.detail-panel")?.textContent).toContain("Next Repo Architecture");
    expect(container.querySelector("aside.detail-panel")?.textContent).not.toContain("Stale Repo Architecture");
    expect(container.querySelector(".statusbar")?.textContent).toContain("Opened Next Repo");
    expect(container.querySelector(".statusbar")?.textContent).not.toContain("Model updated");

    cleanup();
  });
});

describe("App scan behavior", () => {
  afterEach(() => {
    resetDomAndRoute();
  });

  it("runs a desktop codebase scan and displays the scan counts", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);
    ipcMocks.scanCodebase.mockResolvedValueOnce(
      scanSummaryFor({
        scannedFiles: 7,
        changedFiles: 2,
        deletedFiles: 1,
      }),
    );

    const { container, cleanup } = mountApp();
    await flushLayout();

    const scanButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Scan",
    );
    expect(scanButton).not.toBeNull();

    await act(async () => {
      scanButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    expect(ipcMocks.scanCodebase).toHaveBeenCalledTimes(1);
    expect(container.querySelector(".statusbar")?.textContent).toContain("Scanned 7 files");
    expect(container.querySelector(".statusbar")?.textContent).toContain("2 changed");
    expect(container.querySelector(".statusbar")?.textContent).toContain("1 deleted");

    cleanup();
  });

  it("surfaces scan failures without unloading the current canvas", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);
    ipcMocks.scanCodebase.mockRejectedValueOnce(new Error("scan failed"));

    const { container, cleanup } = mountApp();
    await flushLayout();
    const labelsBeforeFailure = getCanvasLabels(container);

    const scanButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Scan",
    );
    expect(scanButton).not.toBeNull();

    await act(async () => {
      scanButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    expect(getCanvasLabels(container)).toEqual(labelsBeforeFailure);
    expect(container.querySelector(".statusbar")?.textContent).toContain("scan failed");

    cleanup();
  });

  it("shows structured command error messages from scan failures", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);
    ipcMocks.scanCodebase.mockRejectedValueOnce({
      code: "repo.not_open",
      message: "No repository is open.",
    });

    const { container, cleanup } = mountApp();
    await flushLayout();

    const scanButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Scan",
    );
    expect(scanButton).not.toBeNull();

    await act(async () => {
      scanButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await flushLayout();

    expect(container.querySelector(".statusbar")?.textContent).toContain("No repository is open.");

    cleanup();
  });

  it("ignores a direct scan response after the active repo changes", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);
    let resolveScan: (summary: ScanSummary) => void = () => {};
    ipcMocks.scanCodebase.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveScan = resolve;
      }),
    );
    const nextRepo = {
      id: "next-repo",
      rootPath: "/tmp/next-repo",
      name: "Next Repo",
    };
    ipcMocks.openRepositoryFromDialog.mockResolvedValueOnce({
      repo: nextRepo,
      model: effectiveModelWithName("Next Repo Architecture", nextRepo.id),
    });

    const { container, cleanup } = mountApp();
    await flushLayout();

    const scanButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Scan",
    );
    const openButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Open Folder",
    );
    expect(scanButton).not.toBeNull();
    expect(openButton).not.toBeNull();

    await act(async () => {
      scanButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await act(async () => {
      openButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
      resolveScan(
        scanSummaryFor({
          scannedFiles: 99,
          changedFiles: 88,
          deletedFiles: 77,
        }),
      );
    });
    await flushLayout();

    expect(container.querySelector(".statusbar")?.textContent).toContain("Opened Next Repo");
    expect(container.querySelector(".statusbar")?.textContent).not.toContain("99 files");

    cleanup();
  });

  it("updates scan status after an index-updated event for the active repo", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);

    const { container, cleanup } = mountApp();
    await flushLayout();

    expect(ipcMocks.state.handlers?.onIndexUpdated).toBeTypeOf("function");
    await act(async () => {
      await ipcMocks.state.handlers!.onIndexUpdated?.({
        repoId: sampleModel.repo.id,
        summary: scanSummaryFor({
          scannedFiles: 11,
          changedFiles: 4,
          deletedFiles: 2,
        }),
      });
    });
    await flushLayout();

    expect(container.querySelector(".statusbar")?.textContent).toContain("Index updated: 11 files");
    expect(container.querySelector(".statusbar")?.textContent).toContain("4 changed");
    expect(container.querySelector(".statusbar")?.textContent).toContain("2 deleted");

    cleanup();
  });

  it("ignores index-updated events for another repo", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);

    const { container, cleanup } = mountApp();
    await flushLayout();
    const statusBeforeEvent = container.querySelector(".statusbar")?.textContent;

    await act(async () => {
      await ipcMocks.state.handlers?.onIndexUpdated?.({
        repoId: "another-repo",
        summary: scanSummaryFor({
          repo: {
            ...sampleModel.repo,
            id: "another-repo",
          },
          scannedFiles: 99,
          changedFiles: 88,
          deletedFiles: 77,
        }),
      });
    });
    await flushLayout();

    expect(container.querySelector(".statusbar")?.textContent).toBe(statusBeforeEvent);

    cleanup();
  });

  it("ignores old-repo index events during a repo switch before rerender", async () => {
    ipcMocks.isTauriDesktop.mockReturnValue(true);
    ipcMocks.fetchActiveModel.mockResolvedValueOnce(null);
    const nextRepo = {
      id: "next-repo",
      rootPath: "/tmp/next-repo",
      name: "Next Repo",
    };
    ipcMocks.openRepositoryFromDialog.mockResolvedValueOnce({
      repo: nextRepo,
      model: effectiveModelWithName("Next Repo Architecture", nextRepo.id),
    });

    const { container, cleanup } = mountApp();
    await flushLayout();

    const openButton = Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Open Folder",
    );
    expect(openButton).not.toBeNull();

    await act(async () => {
      openButton!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
      await ipcMocks.state.handlers?.onIndexUpdated?.({
        repoId: sampleModel.repo.id,
        summary: scanSummaryFor({
          scannedFiles: 99,
          changedFiles: 88,
          deletedFiles: 77,
        }),
      });
    });
    await flushLayout();

    expect(container.querySelector(".statusbar")?.textContent).toContain("Opened Next Repo");
    expect(container.querySelector(".statusbar")?.textContent).not.toContain("99 files");

    cleanup();
  });
});

function effectiveModelWithName(name: string, repoId = sampleModel.repo.id): EffectiveModel {
  return {
    ...sampleModel,
    repo: {
      ...sampleModel.repo,
      id: repoId,
      name: "Watched Repo",
    },
    sourceSha: "changed-source-sha",
    model: {
      ...sampleModel.model,
      name,
    },
    validation: {
      ok: true,
      sourceSha: "changed-source-sha",
      issues: [],
    },
  };
}

function scanSummaryFor(overrides: Partial<ScanSummary> = {}): ScanSummary {
  return {
    repo: sampleModel.repo,
    scanToken: "scan-token",
    scannedFiles: 3,
    changedFiles: 1,
    deletedFiles: 0,
    symbols: 0,
    imports: 0,
    durationMs: 12,
    warnings: [],
    ...overrides,
  };
}

function validationFailureReport(code: string, message: string): ValidationReport {
  return {
    ok: false,
    issues: [
      {
        severity: "error",
        stage: "parse",
        code,
        message,
        path: "c4/model.yml",
      },
    ],
  };
}
