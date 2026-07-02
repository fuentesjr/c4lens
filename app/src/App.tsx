import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
  applyGenerated,
  fetchActiveModel,
  getElementCode,
  generateModel,
  isTauriDesktop,
  listenToModelEvents,
  openRepositoryFromDialog,
  openInEditor,
  scanCodebase,
} from "./ipc/client";
import { layoutWithElk, type C4FlowNode, type C4NodeData } from "./layout/elkLayout";
import { sampleModel } from "./model/sample";
import type {
  CodeRef,
  EffectiveModel,
  ElementNode,
  GenerationDiff,
  GenerationSummary,
  ScanSummary,
  ValidationIssue,
  ValidationReport,
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
  const [generationCandidate, setGenerationCandidate] = useState<GenerationDiff | null>(null);
  const [isOpening, setIsOpening] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isApplyingGenerated, setIsApplyingGenerated] = useState(false);
  const [layoutStatus, setLayoutStatus] = useState("Layout ready");
  const [nodes, setNodes, onNodesChange] = useNodesState<C4FlowNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const activeRepoIdRef = useRef(activeRepoId);

  const view = useMemo(() => deriveView(model, scope), [model, scope]);
  const selectedElement = selectedSlug ? model.elementsBySlug[selectedSlug] : null;
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
    layoutWithElk(view)
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
  }, [setEdges, setNodes, view]);

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
        setGenerationCandidate(null);
        setStatus(`Opened ${activeModel.repo.name}`);
      }
    });
  }, [updateActiveRepoId]);

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
        setGenerationCandidate(null);
        setStatus("Model updated");
      },
      onValidationFailed: (payload) => {
        if (payload.repoId !== activeRepoIdRef.current) {
          return;
        }

        setValidationOverride(payload.validation);
        setGenerationCandidate(null);
        setStatus("Model validation failed");
      },
      onIndexUpdated: (payload) => {
        if (payload.repoId !== activeRepoIdRef.current) {
          return;
        }

        setStatus(`Index updated: ${scanSummaryStatus(payload.summary)}`);
        setIndexRevision(payload.summary.scanToken);
        setGenerationCandidate(null);
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
  }, [updateActiveRepoId]);

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
      setGenerationCandidate(null);
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
  }, [updateActiveRepoId]);

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
        setGenerationCandidate(null);
      }
    } catch (error) {
      if (activeRepoIdRef.current === scanRepoId) {
        setStatus(errorStatus(error, "Scan failed"));
      }
    } finally {
      setIsScanning(false);
    }
  }, []);

  const runGenerate = useCallback(async () => {
    if (!isTauriDesktop()) {
      setStatus("Generation is available in the Tauri desktop shell");
      return;
    }

    setIsGenerating(true);
    setGenerationCandidate(null);
    setStatus("Generating model");
    const generationRepoId = activeRepoIdRef.current;
    try {
      const candidate = await generateModel({ scanFirst: true });
      if (activeRepoIdRef.current === generationRepoId && candidate.repo.id === generationRepoId) {
        setGenerationCandidate(candidate);
        setStatus(`Generated ${generationSummaryStatus(candidate.summary)}`);
      }
    } catch (error) {
      if (activeRepoIdRef.current === generationRepoId) {
        setStatus(errorStatus(error, "Generation failed"));
      }
    } finally {
      setIsGenerating(false);
    }
  }, []);

  const applyGenerationCandidate = useCallback(async () => {
    if (!generationCandidate) {
      return;
    }
    if (!isTauriDesktop()) {
      setStatus("Apply is available in the Tauri desktop shell");
      return;
    }

    setIsApplyingGenerated(true);
    setStatus("Applying generated model");
    const generationRepoId = generationCandidate.repo.id;
    try {
      await applyGenerated({
        generationId: generationCandidate.candidateId,
        expectedAuthoredSha: generationCandidate.baseAuthoredSha,
        expectedOverlaySha: generationCandidate.baseOverlaySha,
        expectedModelSourceSha: generationCandidate.modelSourceSha,
        expectedIndexScanToken: generationCandidate.indexScanToken,
        expectedSchemaVersion: generationCandidate.schemaVersion,
        selection: { acceptAll: true },
      });
      if (activeRepoIdRef.current === generationRepoId) {
        setGenerationCandidate(null);
        setStatus("Applied generated model");
      }
    } catch (error) {
      if (activeRepoIdRef.current === generationRepoId) {
        setStatus(errorStatus(error, "Apply failed"));
      }
    } finally {
      setIsApplyingGenerated(false);
    }
  }, [generationCandidate]);

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
          <label className="search-box">
            <Search size={16} aria-hidden="true" />
            <input placeholder="Search" disabled />
          </label>
          <button className="icon-button" disabled title="Export">
            <Download size={17} aria-hidden="true" />
            <span>Export</span>
          </button>
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
      </nav>

      <main className="workspace">
        <section className="canvas-region" aria-label="Architecture canvas">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onNodeClick={(_, node) => selectNode(node.id)}
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
          onDrillDown={navigateTo}
          onOpenInEditor={openSourceInEditor}
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
  onDrillDown,
  onOpenInEditor,
}: {
  selectedElement: ElementNode | null;
  sourcePreview: SourcePreviewState;
  view: DerivedView;
  model: EffectiveModel;
  scope: ViewScope;
  validationReport: ValidationReport;
  warningIssues: ValidationIssue[];
  errorIssues: ValidationIssue[];
  onDrillDown: (scope: ViewScope) => void;
  onOpenInEditor: () => void;
}) {
  const relatedEdges = selectedElement
    ? view.edges.filter((edge) => edge.source === selectedElement.slug || edge.target === selectedElement.slug)
    : [];
  const drillTarget = nextScopeForDrilldown(model, scope, selectedElement);

  return (
    <aside className="detail-panel">
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
            {selectedElement.generated ? <span>generated</span> : <span>authored</span>}
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
                <div className="relationship-item" key={edge.id}>
                  <span>{model.elementsBySlug[edge.source]?.name ?? edge.source}</span>
                  <strong>{edge.label}</strong>
                  <span>{model.elementsBySlug[edge.target]?.name ?? edge.target}</span>
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
  const parts = [
    countLabel(summary.systemsAdded, "system"),
    countLabel(summary.containersAdded, "container"),
    countLabel(summary.componentsAdded, "component"),
    countLabel(summary.relationshipsAdded, "relationship"),
    countLabel(summary.externalSystemsAdded, "external system"),
  ].filter((part): part is string => Boolean(part));

  return parts.length > 0 ? parts.join(", ") : "No generated changes";
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

function C4Node({ data, selected }: NodeProps) {
  const node = data as C4NodeData;
  return (
    <div className={selected ? "c4-node selected" : "c4-node"}>
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
