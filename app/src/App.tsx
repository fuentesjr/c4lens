import { Children, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  type Edge,
  type NodeProps,
  useEdgesState,
  useNodesState,
} from "@xyflow/react";
import {
  AlertTriangle,
  Box,
  CheckCircle2,
  Circle,
  Database,
  Download,
  ExternalLink,
  FolderOpen,
  GitBranch,
  Network,
  RefreshCw,
  Search,
  Sparkles,
  UserRound,
} from "lucide-react";
import {
  exportView,
  fetchActiveModel,
  getElementCode,
  isTauriDesktop,
  listenToModelEvents,
  openRepositoryFromDialog,
  openInEditor,
  scanCodebase,
  searchRepository,
} from "./ipc/client";
import { serializeViewToSvg, svgToPngBase64 } from "./export/viewExport";
import { useGenerationCandidate } from "./hooks/useGenerationCandidate";
import { layoutWithElk, type C4FlowNode, type C4NodeData } from "./layout/elkLayout";
import { sampleModel } from "./model/sample";
import type {
  CodeRef,
  EffectiveModel,
  ElementNode,
  GenerationDiff,
  GenerationSummary,
  ScanSummary,
  SearchResults,
  ElementSearchResult,
  FileSearchResult,
  SymbolSearchResult,
  ValidationIssue,
  ValidationReport,
  ViewExportFormat,
} from "./model/types";
import { buildHashRoute, resolveHashRoute, type RouteIssue } from "./navigation/routes";
import {
  availableContainers,
  availableSystems,
  deriveView,
  type DerivedView,
  type ViewScope,
} from "./view_deriver/deriveView";

const nodeTypes = {
  c4Node: C4Node,
};

type SourcePreviewState =
  | { status: "idle"; codeRef: null; message: null }
  | { status: "loading"; repoId: string; elementSlug: string; codeRef: null; message: null }
  | { status: "ready"; repoId: string; elementSlug: string; codeRef: CodeRef; message: null }
  | { status: "missing"; repoId: string; elementSlug: string; codeRef: null; message: null }
  | { status: "error"; repoId: string; elementSlug: string; codeRef: null; message: string };

const idleSourcePreview: SourcePreviewState = {
  status: "idle",
  codeRef: null,
  message: null,
};

const emptySearchResults: SearchResults = {
  query: "",
  elements: [],
  files: [],
  symbols: [],
};

type FocusMode = "all" | "connected";

type DependencyState = {
  activeSlug: string | null;
  connectedNodeIds: Set<string>;
  activeEdgeIds: Set<string>;
};

export function App() {
  const initialRoute = useMemo(() => resolveHashRoute(currentHashRoute(), sampleModel), []);
  const [model, setModel] = useState<EffectiveModel>(sampleModel);
  const [activeRepoId, setActiveRepoId] = useState(sampleModel.repo.id);
  const [scope, setScope] = useState<ViewScope>(initialRoute.scope);
  const [selectedSlug, setSelectedSlug] = useState<string | null>(initialRoute.selectedSlug);
  const [routeIssue, setRouteIssue] = useState<RouteIssue | null>(initialRoute.issue);
  const [validationOverride, setValidationOverride] = useState<ValidationReport | null>(null);
  const [status, setStatus] = useState("Sample model ready");
  const [sourcePreview, setSourcePreview] = useState<SourcePreviewState>(idleSourcePreview);
  const [indexRevision, setIndexRevision] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResults>(emptySearchResults);
  const [isSearchFocused, setIsSearchFocused] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [isExporting, setIsExporting] = useState<ViewExportFormat | null>(null);
  const [hoveredSlug, setHoveredSlug] = useState<string | null>(null);
  const [focusMode, setFocusMode] = useState<FocusMode>("all");
  const [isOpening, setIsOpening] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const [layoutStatus, setLayoutStatus] = useState("Layout ready");
  const [nodes, setNodes, onNodesChange] = useNodesState<C4FlowNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const activeRepoIdRef = useRef(activeRepoId);
  const {
    candidate: generationCandidate,
    isApplying: isApplyingGenerated,
    isGenerating,
    clearCandidate: clearGenerationCandidate,
    runGenerate,
    applyCandidate: applyGenerationCandidate,
  } = useGenerationCandidate({
    activeRepoIdRef,
    setStatus,
    formatError: errorStatus,
    formatSummary: generationSummaryStatus,
  });

  const view = useMemo(() => deriveView(model, scope), [model, scope]);
  const selectedElement = selectedSlug ? model.elementsBySlug[selectedSlug] : null;
  const activeDependencySlug = hoveredSlug ?? selectedSlug;
  const dependencyState = useMemo(() => dependencyStateFor(view, activeDependencySlug), [activeDependencySlug, view]);
  const decoratedNodes = useMemo(
    () => decorateNodes(nodes, selectedSlug, dependencyState, focusMode),
    [dependencyState, focusMode, nodes, selectedSlug],
  );
  const decoratedEdges = useMemo(
    () => decorateEdges(edges, dependencyState, focusMode),
    [dependencyState, edges, focusMode],
  );
  const visibleSourcePreview = useMemo(
    () => sourcePreviewForSelection(sourcePreview, activeRepoId, selectedElement),
    [activeRepoId, selectedElement, sourcePreview],
  );
  const activeValidation = validationOverride ?? model.validation;
  const warningIssues = useMemo(
    () => activeValidation.issues.filter((issue) => issue.severity === "warning"),
    [activeValidation.issues],
  );
  const errorIssues = useMemo(
    () => activeValidation.issues.filter((issue) => issue.severity === "error"),
    [activeValidation.issues],
  );
  const systems = useMemo(() => availableSystems(model), [model]);
  const activeSystemSlug = scope.level === "context" ? systems[0]?.slug : currentSystemSlug(model, scope);
  const containers = useMemo(
    () => (activeSystemSlug ? availableContainers(model, activeSystemSlug) : []),
    [activeSystemSlug, model],
  );

  const applyRoute = useCallback(
    (hash: string, routeModel: EffectiveModel = model) => {
      const nextRoute = resolveHashRoute(hash, routeModel);
      setScope(nextRoute.scope);
      setSelectedSlug(nextRoute.selectedSlug);
      setRouteIssue(nextRoute.issue);
    },
    [model],
  );

  const updateActiveRepoId = useCallback((repoId: string) => {
    activeRepoIdRef.current = repoId;
    setActiveRepoId(repoId);
  }, []);

  const navigateTo = useCallback(
    (nextScope: ViewScope, nextSelectedSlug: string | null = null) => {
      const nextHash = buildHashRoute(nextScope, nextSelectedSlug);
      if (currentHashRoute() !== nextHash) {
        window.location.hash = nextHash;
      }
      applyRoute(nextHash);
    },
    [applyRoute],
  );

  const selectNode = useCallback(
    (slug: string) => {
      if (routeIssue?.kind === "route") {
        return;
      }

      const element = model.elementsBySlug[slug];
      if (scope.level === "component" && element?.type === "component" && element.containerSlug === scope.slug) {
        navigateTo(scope, slug);
        return;
      }

      if (routeIssue?.kind === "selection") {
        navigateTo(scope);
        return;
      }

      setSelectedSlug(slug);
      setRouteIssue(null);
    },
    [model.elementsBySlug, navigateTo, routeIssue?.kind, scope],
  );

  const clearSelection = useCallback(() => {
    if (routeIssue?.kind === "route") {
      return;
    }

    if (selectedSlug || routeIssue?.kind === "selection") {
      navigateTo(scope);
      return;
    }

    setSelectedSlug(null);
  }, [navigateTo, routeIssue?.kind, scope, selectedSlug]);

  useEffect(() => {
    activeRepoIdRef.current = activeRepoId;
  }, [activeRepoId]);

  useEffect(() => {
    const handleHashChange = () => {
      applyRoute(currentHashRoute());
    };

    window.addEventListener("hashchange", handleHashChange);
    return () => {
      window.removeEventListener("hashchange", handleHashChange);
    };
  }, [applyRoute]);

  useEffect(() => {
    let cancelled = false;
    setLayoutStatus("Laying out");
    layoutWithElk(view, { sourceSha: model.sourceSha, scope })
      .then((layout) => {
        if (cancelled) {
          return;
        }
        setNodes(layout.nodes);
        setEdges(layout.edges);
        setLayoutStatus("ELK layout ready");
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLayoutStatus(error instanceof Error ? error.message : "Layout failed");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [model.sourceSha, scope, setEdges, setNodes, view]);

  useEffect(() => {
    let cancelled = false;
    const trimmedQuery = searchQuery.trim();
    if (!trimmedQuery) {
      setSearchResults(emptySearchResults);
      setIsSearching(false);
      return () => {
        cancelled = true;
      };
    }

    setIsSearching(true);
    const timeout = window.setTimeout(() => {
      const search = isTauriDesktop()
        ? searchRepository({ query: trimmedQuery, limit: 8 })
        : Promise.resolve(searchLocalModel(model, trimmedQuery, 8));
      search
        .then((results) => {
          if (!cancelled) {
            setSearchResults(results);
          }
        })
        .catch((error: unknown) => {
          if (!cancelled) {
            setSearchResults({ ...emptySearchResults, query: trimmedQuery });
            setStatus(errorStatus(error, "Search failed"));
          }
        })
        .finally(() => {
          if (!cancelled) {
            setIsSearching(false);
          }
        });
    }, 150);

    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
    };
  }, [indexRevision, model, searchQuery]);

  useEffect(() => {
    setNodes((currentNodes) =>
      currentNodes.map((node) => ({
        ...node,
        selected: node.id === selectedSlug,
      })),
    );
  }, [selectedSlug, setNodes]);

  useEffect(() => {
    if (selectedSlug && !view.nodes.some((node) => node.id === selectedSlug)) {
      setSelectedSlug(null);
    }
  }, [selectedSlug, view.nodes]);

  useEffect(() => {
    let cancelled = false;
    setSourcePreview(idleSourcePreview);

    if (!selectedElement?.code || !isTauriDesktop()) {
      return () => {
        cancelled = true;
      };
    }

    const sourceRepoId = activeRepoIdRef.current;
    const sourceSlug = selectedElement.slug;
    setSourcePreview({
      status: "loading",
      repoId: sourceRepoId,
      elementSlug: sourceSlug,
      codeRef: null,
      message: null,
    });

    getElementCode(sourceSlug)
      .then((codeRef) => {
        if (cancelled || activeRepoIdRef.current !== sourceRepoId) {
          return;
        }
        if (codeRef && codeRef.elementSlug !== sourceSlug) {
          return;
        }
        setSourcePreview(
          codeRef
            ? {
                status: "ready",
                repoId: sourceRepoId,
                elementSlug: sourceSlug,
                codeRef,
                message: null,
              }
            : {
                status: "missing",
                repoId: sourceRepoId,
                elementSlug: sourceSlug,
                codeRef: null,
                message: null,
              },
        );
      })
      .catch((error: unknown) => {
        if (!cancelled && activeRepoIdRef.current === sourceRepoId) {
          setSourcePreview({
            status: "error",
            repoId: sourceRepoId,
            elementSlug: sourceSlug,
            codeRef: null,
            message: errorStatus(error, "Source unavailable"),
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeRepoId, indexRevision, model.sourceSha, selectedElement?.code, selectedElement?.slug]);

  useEffect(() => {
    fetchActiveModel().then((activeModel) => {
      if (activeModel) {
        const nextRoute = resolveHashRoute(currentHashRoute(), activeModel);
        setModel(activeModel);
        updateActiveRepoId(activeModel.repo.id);
        setScope(nextRoute.scope);
        setSelectedSlug(nextRoute.selectedSlug);
        setRouteIssue(nextRoute.issue);
        setValidationOverride(null);
        setIndexRevision(null);
        clearGenerationCandidate();
        setStatus(`Opened ${activeModel.repo.name}`);
      }
    });
  }, [clearGenerationCandidate, updateActiveRepoId]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    listenToModelEvents({
      onModelChanged: async (payload) => {
        const eventRepoId = payload.repoId;
        if (eventRepoId !== activeRepoIdRef.current) {
          return;
        }

        const activeModel = await fetchActiveModel();
        if (!activeModel || disposed || activeRepoIdRef.current !== eventRepoId || activeModel.repo.id !== eventRepoId) {
          return;
        }

        const nextRoute = resolveHashRoute(currentHashRoute(), activeModel);
        setModel(activeModel);
        updateActiveRepoId(activeModel.repo.id);
        setScope(nextRoute.scope);
        setSelectedSlug(nextRoute.selectedSlug);
        setRouteIssue(nextRoute.issue);
        setValidationOverride(null);
        setIndexRevision(null);
        clearGenerationCandidate();
        setStatus("Model updated");
      },
      onValidationFailed: (payload) => {
        if (payload.repoId !== activeRepoIdRef.current) {
          return;
        }

        setValidationOverride(payload.validation);
        clearGenerationCandidate();
        setStatus("Model validation failed");
      },
      onIndexUpdated: (payload) => {
        if (payload.repoId !== activeRepoIdRef.current) {
          return;
        }

        setStatus(`Index updated: ${scanSummaryStatus(payload.summary)}`);
        setIndexRevision(payload.summary.scanToken);
        clearGenerationCandidate();
      },
    }).then((cleanup) => {
      if (disposed) {
        cleanup();
        return;
      }
      unlisten = cleanup;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [clearGenerationCandidate, updateActiveRepoId]);

  const openRepo = useCallback(async () => {
    if (!isTauriDesktop()) {
      setStatus("Folder picker is available in the Tauri desktop shell");
      return;
    }

    setIsOpening(true);
    setStatus("Opening repository");
    try {
      const result = await openRepositoryFromDialog();
      if (!result) {
        setStatus("Open canceled");
        return;
      }
      updateActiveRepoId(result.repo.id);
      setIndexRevision(null);
      clearGenerationCandidate();
      if (result.model) {
        const nextRoute = resolveHashRoute(currentHashRoute(), result.model);
        setModel(result.model);
        setScope(nextRoute.scope);
        setSelectedSlug(nextRoute.selectedSlug);
        setRouteIssue(nextRoute.issue);
        setValidationOverride(null);
        setStatus(`Opened ${result.repo.name}`);
      } else {
        setValidationOverride({
          ok: false,
          issues: [
            {
              severity: "error",
              stage: "parse",
              code: "model.load_failed",
              message: `Unable to load ${result.repo.name}. Waiting for a valid model file.`,
            },
          ],
        });
        setSelectedSlug(null);
        setRouteIssue(null);
        setStatus(`Watching ${result.repo.name}`);
      }
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to open repository");
    } finally {
      setIsOpening(false);
    }
  }, [clearGenerationCandidate, updateActiveRepoId]);

  const runScan = useCallback(async () => {
    if (!isTauriDesktop()) {
      setStatus("Scan is available in the Tauri desktop shell");
      return;
    }

    setIsScanning(true);
    setStatus("Scanning codebase");
    const scanRepoId = activeRepoIdRef.current;
    try {
      const summary = await scanCodebase();
      if (activeRepoIdRef.current === scanRepoId && summary.repo.id === scanRepoId) {
        setStatus(`Scanned ${scanSummaryStatus(summary)}`);
        setIndexRevision(summary.scanToken);
        clearGenerationCandidate();
      }
    } catch (error) {
      if (activeRepoIdRef.current === scanRepoId) {
        setStatus(errorStatus(error, "Scan failed"));
      }
    } finally {
      setIsScanning(false);
    }
  }, [clearGenerationCandidate]);

  const openSourceInEditor = useCallback(async () => {
    if (!isTauriDesktop() || sourcePreview.status !== "ready") {
      return;
    }

    try {
      await openInEditor(sourcePreview.codeRef.path);
      setStatus(`Opened ${sourcePreview.codeRef.path}`);
    } catch (error) {
      setStatus(errorStatus(error, "Failed to open source"));
    }
  }, [sourcePreview]);

  const openSearchPath = useCallback(async (path: string, range?: { startLine: number; startColumn: number }) => {
    if (!isTauriDesktop()) {
      setStatus(path);
      return;
    }

    try {
      await openInEditor(path, range?.startLine, range?.startColumn);
      setStatus(`Opened ${path}`);
    } catch (error) {
      setStatus(errorStatus(error, "Failed to open search result"));
    }
  }, []);

  const focusSearchElement = useCallback(
    (slug: string) => {
      const element = model.elementsBySlug[slug];
      if (!element) {
        return;
      }
      const targetScope = scopeForElement(element);
      if (!targetScope) {
        return;
      }

      if (element.type === "component") {
        navigateTo(targetScope, element.slug);
      } else {
        navigateTo(targetScope);
        setSelectedSlug(element.slug);
        setRouteIssue(null);
      }
      setSearchQuery("");
      setSearchResults(emptySearchResults);
      setIsSearchFocused(false);
    },
    [model.elementsBySlug, navigateTo],
  );

  const runExport = useCallback(
    async (format: ViewExportFormat) => {
      if (!isTauriDesktop()) {
        setStatus("Export is available in the Tauri desktop shell");
        return;
      }

      setIsExporting(format);
      try {
        const serialized = serializeViewToSvg(nodes, edges, model.model.name);
        const params =
          format === "svg"
            ? { format, scope, svg: serialized.svg }
            : { format, scope, pngBase64: await svgToPngBase64(serialized) };
        const result = await exportView(params);
        setStatus(`Exported ${format.toUpperCase()} to ${result.savedPath}`);
      } catch (error) {
        setStatus(errorStatus(error, "Export failed"));
      } finally {
        setIsExporting(null);
      }
    },
    [edges, model.model.name, nodes, scope],
  );

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <Network size={20} aria-hidden="true" />
          <div>
            <strong>c4lens</strong>
            <span>{model.repo.name}</span>
          </div>
        </div>

        <div className="topbar-actions">
          <button className="icon-button primary" onClick={openRepo} disabled={isOpening} title="Open folder">
            <FolderOpen size={17} aria-hidden="true" />
            <span>{isOpening ? "Opening" : "Open Folder"}</span>
          </button>
          <button className="icon-button" onClick={runScan} disabled={isScanning} title="Scan codebase">
            <RefreshCw size={17} aria-hidden="true" />
            <span>{isScanning ? "Scanning" : "Scan"}</span>
          </button>
          <button
            className="icon-button"
            onClick={runGenerate}
            disabled={isGenerating || isApplyingGenerated || isOpening || isScanning}
            title="Generate from code"
          >
            <Sparkles size={17} aria-hidden="true" />
            <span>{isGenerating ? "Generating" : "Generate"}</span>
          </button>
          {generationCandidate ? (
            <div className="generation-chip" role="status">
              <span>{generationSummaryStatus(generationCandidate.summary)}</span>
              <button
                className="icon-button compact"
                onClick={applyGenerationCandidate}
                disabled={isApplyingGenerated || isGenerating || isOpening || isScanning}
                title="Apply generated overlay"
              >
                <CheckCircle2 size={15} aria-hidden="true" />
                <span>{isApplyingGenerated ? "Applying" : "Apply"}</span>
              </button>
            </div>
          ) : null}
          <SearchBox
            query={searchQuery}
            results={searchResults}
            isFocused={isSearchFocused}
            isSearching={isSearching}
            onQueryChange={setSearchQuery}
            onFocusChange={setIsSearchFocused}
            onElementSelect={focusSearchElement}
            onFileSelect={(result) => void openSearchPath(result.path)}
            onSymbolSelect={(result) => void openSearchPath(result.path, result.range)}
          />
          <div className="export-actions" aria-label="Export view">
            <button
              className="icon-button"
              onClick={() => void runExport("svg")}
              disabled={Boolean(isExporting) || nodes.length === 0}
              title="Export SVG"
            >
              <Download size={17} aria-hidden="true" />
              <span>{isExporting === "svg" ? "Saving" : "SVG"}</span>
            </button>
            <button
              className="icon-button compact"
              onClick={() => void runExport("png")}
              disabled={Boolean(isExporting) || nodes.length === 0}
              title="Export PNG"
            >
              <span>{isExporting === "png" ? "Saving" : "PNG"}</span>
            </button>
          </div>
        </div>
      </header>

      <nav className="scopebar" aria-label="View scope">
        <button
          className={scope.level === "context" ? "scope-button active" : "scope-button"}
          onClick={() => navigateTo({ level: "context" })}
        >
          Context
        </button>
        {systems.map((system) => (
          <button
            key={system.slug}
            className={scope.level === "container" && scope.slug === system.slug ? "scope-button active" : "scope-button"}
            onClick={() => navigateTo({ level: "container", slug: system.slug })}
          >
            {system.name}
          </button>
        ))}
        {containers.map((container) => (
          <button
            key={container.slug}
            className={scope.level === "component" && scope.slug === container.slug ? "scope-button active" : "scope-button"}
            onClick={() => navigateTo({ level: "component", slug: container.slug })}
          >
            {container.name}
          </button>
        ))}
        <div className="scope-spacer" />
        <div className="segmented-control" aria-label="Dependency focus">
          <button
            className={focusMode === "all" ? "active" : ""}
            type="button"
            onClick={() => setFocusMode("all")}
          >
            All
          </button>
          <button
            className={focusMode === "connected" ? "active" : ""}
            type="button"
            onClick={() => setFocusMode("connected")}
          >
            Linked
          </button>
        </div>
      </nav>

      <main className="workspace">
        <section className="canvas-region" aria-label="Architecture canvas">
          <ReactFlow
            nodes={decoratedNodes}
            edges={decoratedEdges}
            nodeTypes={nodeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onNodeClick={(_, node) => selectNode(node.id)}
            onNodeMouseEnter={(_, node) => setHoveredSlug(node.id)}
            onNodeMouseLeave={() => setHoveredSlug(null)}
            onNodeDoubleClick={(_, node) => {
              const nextScope = nextScopeForDrilldown(model, scope, model.elementsBySlug[node.id]);
              if (nextScope) {
                navigateTo(nextScope);
              }
            }}
            onPaneClick={clearSelection}
            fitView
            minZoom={0.25}
            maxZoom={1.7}
          >
            <Background gap={24} size={1} />
            <Controls position="bottom-left" />
            <MiniMap position="bottom-right" pannable zoomable />
          </ReactFlow>
          {routeIssue ? <RouteIssueNotice issue={routeIssue} /> : null}
        </section>

        <DetailPanel
          selectedElement={selectedElement}
          sourcePreview={visibleSourcePreview}
          view={view}
          model={model}
          scope={scope}
          validationReport={activeValidation}
          warningIssues={warningIssues}
          errorIssues={errorIssues}
          generationCandidate={generationCandidate}
          isApplyingGenerated={isApplyingGenerated}
          onDrillDown={navigateTo}
          onOpenInEditor={openSourceInEditor}
          onApplyGeneration={applyGenerationCandidate}
        />
      </main>

      <footer className="statusbar">
        <span className={activeValidation.ok && warningIssues.length === 0 ? "status-ok" : "status-warning"}>
          {activeValidation.ok && warningIssues.length === 0 ? (
            <CheckCircle2 size={15} />
          ) : (
            <AlertTriangle size={15} />
          )}
          {validationStatusText(activeValidation, warningIssues)}
        </span>
        <span>{status}</span>
        <span>{layoutStatus}</span>
        <span>{view.nodes.length} nodes</span>
        <span>{view.edges.length} edges</span>
      </footer>
    </div>
  );
}

function currentHashRoute(): string {
  return typeof window === "undefined" ? "" : window.location.hash;
}

function sourcePreviewForSelection(
  state: SourcePreviewState,
  activeRepoId: string,
  selectedElement: ElementNode | null,
): SourcePreviewState {
  if (state.status === "idle" || !selectedElement) {
    return idleSourcePreview;
  }

  if (state.repoId !== activeRepoId || state.elementSlug !== selectedElement.slug) {
    return idleSourcePreview;
  }

  return state;
}

function DetailPanel({
  selectedElement,
  sourcePreview,
  view,
  model,
  scope,
  validationReport,
  warningIssues,
  errorIssues,
  generationCandidate,
  isApplyingGenerated,
  onDrillDown,
  onOpenInEditor,
  onApplyGeneration,
}: {
  selectedElement: ElementNode | null;
  sourcePreview: SourcePreviewState;
  view: DerivedView;
  model: EffectiveModel;
  scope: ViewScope;
  validationReport: ValidationReport;
  warningIssues: ValidationIssue[];
  errorIssues: ValidationIssue[];
  generationCandidate: GenerationDiff | null;
  isApplyingGenerated: boolean;
  onDrillDown: (scope: ViewScope) => void;
  onOpenInEditor: () => void;
  onApplyGeneration: () => void;
}) {
  const relatedEdges = selectedElement
    ? view.edges.filter((edge) => edge.source === selectedElement.slug || edge.target === selectedElement.slug)
    : [];
  const drillTarget = nextScopeForDrilldown(model, scope, selectedElement);

  return (
    <aside className="detail-panel">
      {generationCandidate ? (
        <GenerationReview
          candidate={generationCandidate}
          isApplying={isApplyingGenerated}
          onApply={onApplyGeneration}
        />
      ) : null}
      {selectedElement ? (
        <>
          <div className="detail-heading">
            <ElementIcon element={selectedElement} />
            <div>
              <h2>{selectedElement.name}</h2>
              <p>{selectedElement.slug}</p>
            </div>
          </div>
          <div className="metadata-row">
            <span>{selectedElement.type}</span>
            <span>{selectedElement.status}</span>
            <span className={selectedElement.generated ? "generated-badge" : "authored-badge"}>
              {selectedElement.generated ? "generated" : "authored"}
            </span>
          </div>
          {selectedElement.description ? <p className="detail-copy">{selectedElement.description}</p> : null}
          <dl className="detail-list">
            {selectedElement.tech ? (
              <>
                <dt>Tech</dt>
                <dd>{selectedElement.tech}</dd>
              </>
            ) : null}
            {selectedElement.code ? (
              <>
                <dt>Code</dt>
                <dd>{selectedElement.code}</dd>
              </>
            ) : null}
          </dl>
          <SourcePreview state={sourcePreview} onOpenInEditor={onOpenInEditor} />
          {drillTarget ? (
            <button className="detail-action" onClick={() => onDrillDown(drillTarget)}>
              {drillTarget.level === "component" ? "Open components" : "Open containers"}
            </button>
          ) : null}
          <h3>Relationships</h3>
          <div className="relationship-list">
            {relatedEdges.length > 0 ? (
              relatedEdges.map((edge) => (
                <div className={edge.generated ? "relationship-item generated" : "relationship-item"} key={edge.id}>
                  <span>{model.elementsBySlug[edge.source]?.name ?? edge.source}</span>
                  <strong>{edge.label}</strong>
                  <span>{model.elementsBySlug[edge.target]?.name ?? edge.target}</span>
                  <em>{edge.generated ? "generated" : "authored"}</em>
                </div>
              ))
            ) : (
              <p className="muted">No visible relationships in this view.</p>
            )}
          </div>
        </>
      ) : (
        <>
          <div className="detail-heading">
            <Network size={22} aria-hidden="true" />
            <div>
              <h2>{model.model.name}</h2>
              <p>{model.sourceSha}</p>
            </div>
          </div>
          {model.model.description ? <p className="detail-copy">{model.model.description}</p> : null}
          <dl className="detail-list">
            <dt>Repository</dt>
            <dd>{model.repo.name}</dd>
            <dt>Validation</dt>
            <dd>{validationStatusText(validationReport, warningIssues)}</dd>
          </dl>
          {!validationReport.ok && errorIssues.length > 0 ? (
            <>
              <h3>Validation</h3>
              <div className="validation-list">
                {errorIssues.map((issue, index) => (
                  <ValidationIssueCard issue={issue} key={`${issue.code}-${issue.path ?? "model"}-${index}`} />
                ))}
              </div>
            </>
          ) : null}
          {validationReport.ok && warningIssues.length > 0 ? (
            <>
              <h3>Warnings</h3>
              <div className="validation-list">
                {warningIssues.map((issue, index) => (
                  <ValidationIssueCard issue={issue} key={`${issue.code}-${issue.path ?? "model"}-${index}`} />
                ))}
              </div>
            </>
          ) : null}
        </>
      )}
    </aside>
  );
}

function GenerationReview({
  candidate,
  isApplying,
  onApply,
}: {
  candidate: GenerationDiff;
  isApplying: boolean;
  onApply: () => void;
}) {
  const diffLines = useMemo(
    () => yamlDiffLines(candidate.beforeYaml ?? "", candidate.afterYaml),
    [candidate.afterYaml, candidate.beforeYaml],
  );

  return (
    <section className="generation-review" aria-label="Generation review">
      <div className="panel-section-heading">
        <h3>Generation Review</h3>
        <span>{candidate.changes.length} changes</span>
      </div>
      <div className="generation-review-summary">
        {generationSummaryParts(candidate.summary).map((part) => (
          <span key={part}>{part}</span>
        ))}
      </div>
      <div className="generation-review-changes">
        {candidate.changes.slice(0, 6).map((change) => (
          <span key={change.id}>
            {change.target}: {change.slug ?? change.relationshipKey ?? change.id}
          </span>
        ))}
        {candidate.changes.length > 6 ? <span>{candidate.changes.length - 6} more changes</span> : null}
      </div>
      <pre className="yaml-diff" aria-label="Generated YAML diff">
        {diffLines.map((line, index) => (
          <span className={`yaml-diff-line ${line.kind}`} key={`${line.kind}-${index}`}>
            {line.prefix}
            {line.text}
          </span>
        ))}
      </pre>
      <button className="detail-action" onClick={onApply} disabled={isApplying}>
        <CheckCircle2 size={14} aria-hidden="true" />
        <span>{isApplying ? "Applying" : "Apply generated overlay"}</span>
      </button>
    </section>
  );
}

function SourcePreview({
  state,
  onOpenInEditor,
}: {
  state: SourcePreviewState;
  onOpenInEditor: () => void;
}) {
  if (state.status === "idle") {
    return null;
  }

  return (
    <section className="source-preview" aria-label="Source preview">
      <h3>Source</h3>
      {state.status === "loading" ? <p className="muted">Loading source</p> : null}
      {state.status === "missing" ? <p className="muted">Source not indexed</p> : null}
      {state.status === "error" ? <p className="muted">{state.message}</p> : null}
      {state.status === "ready" ? (
        <>
          <div className="source-preview-meta">
            <span>{state.codeRef.path}</span>
            {state.codeRef.language ? <span>{state.codeRef.language}</span> : null}
          </div>
          <button className="detail-action" onClick={onOpenInEditor}>
            <ExternalLink size={14} aria-hidden="true" />
            <span>Jump to code</span>
          </button>
          {state.codeRef.snippet ? (
            <pre>{state.codeRef.snippet}</pre>
          ) : (
            <p className="muted">No snippet available.</p>
          )}
        </>
      ) : null}
    </section>
  );
}

function SearchBox({
  query,
  results,
  isFocused,
  isSearching,
  onQueryChange,
  onFocusChange,
  onElementSelect,
  onFileSelect,
  onSymbolSelect,
}: {
  query: string;
  results: SearchResults;
  isFocused: boolean;
  isSearching: boolean;
  onQueryChange: (query: string) => void;
  onFocusChange: (focused: boolean) => void;
  onElementSelect: (slug: string) => void;
  onFileSelect: (result: FileSearchResult) => void;
  onSymbolSelect: (result: SymbolSearchResult) => void;
}) {
  const trimmedQuery = query.trim();
  const resultCount = results.elements.length + results.files.length + results.symbols.length;
  const isOpen = isFocused && Boolean(trimmedQuery);

  const closeAfter = (action: () => void) => {
    action();
    onQueryChange("");
    onFocusChange(false);
  };

  return (
    <div className="search-control">
      <label className="search-box">
        <Search size={16} aria-hidden="true" />
        <input
          placeholder="Search"
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          onFocus={() => onFocusChange(true)}
          onBlur={() => onFocusChange(false)}
        />
      </label>
      {isOpen ? (
        <div className="search-results" role="listbox" onMouseDown={(event) => event.preventDefault()}>
          {isSearching ? <div className="search-empty">Searching</div> : null}
          {!isSearching && resultCount === 0 ? <div className="search-empty">No results</div> : null}
          <SearchResultGroup title="Elements">
            {results.elements.map((result) => (
              <button key={result.slug} type="button" onClick={() => closeAfter(() => onElementSelect(result.slug))}>
                <strong>{result.name}</strong>
                <span>{result.type} - {result.match}</span>
              </button>
            ))}
          </SearchResultGroup>
          <SearchResultGroup title="Files">
            {results.files.map((result) => (
              <button key={result.path} type="button" onClick={() => closeAfter(() => onFileSelect(result))}>
                <strong>{result.path}</strong>
                <span>{result.language ?? "file"}</span>
              </button>
            ))}
          </SearchResultGroup>
          <SearchResultGroup title="Symbols">
            {results.symbols.map((result) => (
              <button
                key={`${result.path}:${result.range.startLine}:${result.name}`}
                type="button"
                onClick={() => closeAfter(() => onSymbolSelect(result))}
              >
                <strong>{result.qualifiedName ?? result.name}</strong>
                <span>{result.path}:{result.range.startLine}</span>
              </button>
            ))}
          </SearchResultGroup>
        </div>
      ) : null}
    </div>
  );
}

function SearchResultGroup({ title, children }: { title: string; children: ReactNode }) {
  const items = Children.toArray(children);
  if (items.length === 0) {
    return null;
  }

  return (
    <section>
      <h3>{title}</h3>
      <div>{items}</div>
    </section>
  );
}

function ValidationIssueCard({ issue }: { issue: ValidationIssue }) {
  return (
    <div className="validation-item">
      <strong>{issue.code}</strong>
      <span>{issue.message}</span>
      {issue.path ? <span>{issue.path}</span> : null}
    </div>
  );
}

function RouteIssueNotice({ issue }: { issue: RouteIssue }) {
  return (
    <div className="route-notice" role="status">
      <AlertTriangle size={24} aria-hidden="true" />
      <div>
        <strong>{issue.title}</strong>
        <span>{issue.slug}</span>
        <p>{issue.message}</p>
      </div>
    </div>
  );
}

function validationStatusText(validation: ValidationReport, warningIssues: ValidationIssue[]): string {
  if (!validation.ok) {
    return "Validation issues";
  }

  if (warningIssues.length > 0) {
    return `${warningIssues.length} ${warningIssues.length === 1 ? "warning" : "warnings"}`;
  }

  return "Valid model";
}

function scanSummaryStatus(summary: ScanSummary): string {
  return `${summary.scannedFiles} files (${summary.changedFiles} changed, ${summary.deletedFiles} deleted)`;
}

function generationSummaryStatus(summary: GenerationSummary): string {
  const parts = generationSummaryParts(summary);

  return parts.length > 0 ? parts.join(", ") : "No generated changes";
}

function generationSummaryParts(summary: GenerationSummary): string[] {
  return [
    countLabel(summary.systemsAdded, "system"),
    countLabel(summary.containersAdded, "container"),
    countLabel(summary.componentsAdded, "component"),
    countLabel(summary.relationshipsAdded, "relationship"),
    countLabel(summary.externalSystemsAdded, "external system"),
  ].filter((part): part is string => Boolean(part));
}

function countLabel(count: number, singular: string): string | null {
  if (count === 0) {
    return null;
  }
  return `${count} ${count === 1 ? singular : `${singular}s`}`;
}

function errorStatus(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }
  if (typeof error === "string" && error) {
    return error;
  }
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message) {
      return message;
    }
  }
  return fallback;
}

type YamlDiffLine = {
  kind: "same" | "added" | "removed";
  prefix: string;
  text: string;
};

function yamlDiffLines(beforeYaml: string, afterYaml: string, maxLines = 180): YamlDiffLine[] {
  const before = splitYamlLines(beforeYaml);
  const after = splitYamlLines(afterYaml);
  if (before.length === 0) {
    return after.slice(0, maxLines).map((text) => ({ kind: "added", prefix: "+ ", text }));
  }

  const lines: YamlDiffLine[] = [];
  const lineCount = Math.max(before.length, after.length);
  for (let index = 0; index < lineCount && lines.length < maxLines; index += 1) {
    const beforeLine = before[index];
    const afterLine = after[index];
    if (beforeLine === afterLine && beforeLine !== undefined) {
      lines.push({ kind: "same", prefix: "  ", text: beforeLine });
      continue;
    }
    if (beforeLine !== undefined) {
      lines.push({ kind: "removed", prefix: "- ", text: beforeLine });
    }
    if (afterLine !== undefined && lines.length < maxLines) {
      lines.push({ kind: "added", prefix: "+ ", text: afterLine });
    }
  }
  if (lineCount > maxLines) {
    lines.push({ kind: "same", prefix: "  ", text: "...diff truncated" });
  }
  return lines;
}

function splitYamlLines(value: string): string[] {
  return value ? value.replace(/\n$/, "").split("\n") : [];
}

function searchLocalModel(model: EffectiveModel, query: string, limit: number): SearchResults {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return emptySearchResults;
  }

  const elements = Object.values(model.elementsBySlug)
    .flatMap((element): ElementSearchResult[] => {
      const match = localElementMatch(element, normalized);
      return match
        ? [
            {
              slug: element.slug,
              name: element.name,
              type: element.type,
              match,
            },
          ]
        : [];
    })
    .sort((left, right) => left.slug.localeCompare(right.slug))
    .slice(0, limit);

  return {
    query: query.trim(),
    elements,
    files: [],
    symbols: [],
  };
}

function localElementMatch(element: ElementNode, normalizedQuery: string): ElementSearchResult["match"] | null {
  if (element.slug.toLowerCase().includes(normalizedQuery)) {
    return "slug";
  }
  if (element.name.toLowerCase().includes(normalizedQuery)) {
    return "name";
  }
  if (element.description?.toLowerCase().includes(normalizedQuery)) {
    return "description";
  }
  if (element.tech?.toLowerCase().includes(normalizedQuery)) {
    return "tech";
  }
  return null;
}

function dependencyStateFor(view: DerivedView, activeSlug: string | null): DependencyState {
  if (!activeSlug) {
    return {
      activeSlug: null,
      connectedNodeIds: new Set(),
      activeEdgeIds: new Set(),
    };
  }

  const connectedNodeIds = new Set([activeSlug]);
  const activeEdgeIds = new Set<string>();
  view.edges.forEach((edge) => {
    if (edge.source === activeSlug || edge.target === activeSlug) {
      activeEdgeIds.add(edge.id);
      connectedNodeIds.add(edge.source);
      connectedNodeIds.add(edge.target);
    }
  });
  return { activeSlug, connectedNodeIds, activeEdgeIds };
}

function decorateNodes(
  nodes: C4FlowNode[],
  selectedSlug: string | null,
  dependencyState: DependencyState,
  focusMode: FocusMode,
): C4FlowNode[] {
  return nodes.map((node) => {
    const relationshipState = relationshipStateForNode(node.id, dependencyState, focusMode);
    return {
      ...node,
      selected: node.id === selectedSlug,
      className: relationshipState === "muted" ? "dependency-muted" : undefined,
      data: {
        ...node.data,
        relationshipState,
      },
    };
  });
}

function decorateEdges(edges: Edge[], dependencyState: DependencyState, focusMode: FocusMode): Edge[] {
  return edges.map((edge) => {
    const relationshipState = relationshipStateForEdge(edge.id, dependencyState, focusMode);
    return {
      ...edge,
      className: [
        edge.className,
        relationshipState === "active" ? "dependency-active" : null,
        relationshipState === "muted" ? "dependency-muted" : null,
        edge.data?.generated ? "generated-edge" : null,
      ]
        .filter(Boolean)
        .join(" "),
      data: {
        ...edge.data,
        relationshipState,
      },
    };
  });
}

function relationshipStateForNode(
  nodeId: string,
  dependencyState: DependencyState,
  focusMode: FocusMode,
): "active" | "muted" | "neutral" {
  if (!dependencyState.activeSlug) {
    return "neutral";
  }
  if (dependencyState.connectedNodeIds.has(nodeId)) {
    return "active";
  }
  return focusMode === "connected" ? "muted" : "neutral";
}

function relationshipStateForEdge(
  edgeId: string,
  dependencyState: DependencyState,
  focusMode: FocusMode,
): "active" | "muted" | "neutral" {
  if (!dependencyState.activeSlug) {
    return "neutral";
  }
  if (dependencyState.activeEdgeIds.has(edgeId)) {
    return "active";
  }
  return focusMode === "connected" ? "muted" : "neutral";
}

function C4Node({ data, selected }: NodeProps) {
  const node = data as C4NodeData;
  const relationshipState = typeof node.relationshipState === "string" ? node.relationshipState : "neutral";
  return (
    <div
      className={[
        "c4-node",
        selected ? "selected" : null,
        node.generated ? "generated" : null,
        relationshipState === "active" ? "dependency-active" : null,
        relationshipState === "muted" ? "dependency-muted" : null,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="node-heading">
        <span className={`node-icon ${node.elementType}`}>
          <NodeIcon type={node.elementType} external={node.external} />
        </span>
        <div>
          <strong>{node.label}</strong>
          <span>{node.subtitle}</span>
        </div>
      </div>
      <div className="node-footer">
        {node.tech ? <span>{node.tech}</span> : <span>{node.status}</span>}
        <span className={node.generated ? "node-provenance generated" : "node-provenance authored"}>
          {node.generated ? "generated" : "authored"}
        </span>
        {node.external ? <ExternalLink size={14} aria-hidden="true" /> : <Circle size={10} aria-hidden="true" />}
      </div>
    </div>
  );
}

function ElementIcon({ element }: { element: ElementNode }) {
  return (
    <span className={`detail-icon ${element.type}`}>
      <NodeIcon type={element.type} external={Boolean(element.external)} />
    </span>
  );
}

function NodeIcon({ type, external }: { type: string; external: boolean }) {
  if (external) {
    return <ExternalLink size={17} aria-hidden="true" />;
  }
  if (type === "actor") {
    return <UserRound size={17} aria-hidden="true" />;
  }
  if (type === "container") {
    return <Box size={17} aria-hidden="true" />;
  }
  if (type === "component") {
    return <GitBranch size={17} aria-hidden="true" />;
  }
  if (type === "system") {
    return <Database size={17} aria-hidden="true" />;
  }
  return <Circle size={17} aria-hidden="true" />;
}

function currentSystemSlug(model: EffectiveModel, scope: ViewScope): string | null {
  if (scope.level === "container") {
    return scope.slug;
  }
  if (scope.level === "component") {
    return model.elementsBySlug[scope.slug]?.systemSlug ?? null;
  }
  return null;
}

function scopeForElement(element: ElementNode): ViewScope | null {
  if (element.type === "actor" || element.type === "system") {
    return { level: "context" };
  }
  if (element.type === "container" && element.systemSlug) {
    return { level: "container", slug: element.systemSlug };
  }
  if (element.type === "component" && element.containerSlug) {
    return { level: "component", slug: element.containerSlug };
  }
  return null;
}

function nextScopeForDrilldown(
  model: EffectiveModel,
  scope: ViewScope,
  selectedElement: ElementNode | null,
): ViewScope | null {
  if (!selectedElement) {
    return null;
  }
  if (scope.level === "context" && selectedElement.type === "system" && !selectedElement.external) {
    return { level: "container", slug: selectedElement.slug };
  }
  if (
    scope.level === "container" &&
    selectedElement.type === "container" &&
    selectedElement.systemSlug === scope.slug &&
    model.elementsBySlug[scope.slug]?.type === "system"
  ) {
    return { level: "component", slug: selectedElement.slug };
  }
  return null;
}
