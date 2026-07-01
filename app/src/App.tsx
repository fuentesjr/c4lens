import { useCallback, useEffect, useMemo, useState } from "react";
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
  UserRound,
} from "lucide-react";
import { openRepositoryFromDialog, fetchActiveModel, isTauriDesktop } from "./ipc/client";
import { layoutWithElk, type C4FlowNode, type C4NodeData } from "./layout/elkLayout";
import { sampleModel } from "./model/sample";
import type { EffectiveModel, ElementNode, ValidationIssue } from "./model/types";
import {
  availableContainers,
  availableSystems,
  defaultScope,
  deriveView,
  type DerivedView,
  type ViewScope,
} from "./view_deriver/deriveView";

const nodeTypes = {
  c4Node: C4Node,
};

export function App() {
  const [model, setModel] = useState<EffectiveModel>(sampleModel);
  const [scope, setScope] = useState<ViewScope>(() => defaultScope(sampleModel));
  const [selectedSlug, setSelectedSlug] = useState<string | null>(null);
  const [status, setStatus] = useState("Sample model ready");
  const [isOpening, setIsOpening] = useState(false);
  const [layoutStatus, setLayoutStatus] = useState("Layout ready");
  const [nodes, setNodes, onNodesChange] = useNodesState<C4FlowNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

  const view = useMemo(() => deriveView(model, scope), [model, scope]);
  const selectedElement = selectedSlug ? model.elementsBySlug[selectedSlug] : null;
  const warningIssues = useMemo(
    () => model.validation.issues.filter((issue) => issue.severity === "warning"),
    [model.validation.issues],
  );
  const systems = useMemo(() => availableSystems(model), [model]);
  const activeSystemSlug = scope.level === "context" ? systems[0]?.slug : currentSystemSlug(model, scope);
  const containers = useMemo(
    () => (activeSystemSlug ? availableContainers(model, activeSystemSlug) : []),
    [activeSystemSlug, model],
  );

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
    fetchActiveModel().then((activeModel) => {
      if (activeModel) {
        setModel(activeModel);
        setScope(defaultScope(activeModel));
        setStatus(`Opened ${activeModel.repo.name}`);
      }
    });
  }, []);

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
      setModel(result.model);
      setScope(defaultScope(result.model));
      setSelectedSlug(null);
      setStatus(`Opened ${result.repo.name}`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to open repository");
    } finally {
      setIsOpening(false);
    }
  }, []);

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
          <button className="icon-button" disabled title="Generate">
            <RefreshCw size={17} aria-hidden="true" />
            <span>Generate</span>
          </button>
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
          onClick={() => setScope({ level: "context" })}
        >
          Context
        </button>
        {systems.map((system) => (
          <button
            key={system.slug}
            className={scope.level === "container" && scope.slug === system.slug ? "scope-button active" : "scope-button"}
            onClick={() => setScope({ level: "container", slug: system.slug })}
          >
            {system.name}
          </button>
        ))}
        {containers.map((container) => (
          <button
            key={container.slug}
            className={scope.level === "component" && scope.slug === container.slug ? "scope-button active" : "scope-button"}
            onClick={() => setScope({ level: "component", slug: container.slug })}
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
            onNodeClick={(_, node) => setSelectedSlug(node.id)}
            onNodeDoubleClick={(_, node) => {
              const nextScope = nextScopeForDrilldown(model, scope, model.elementsBySlug[node.id]);
              if (nextScope) {
                setScope(nextScope);
              }
            }}
            onPaneClick={() => setSelectedSlug(null)}
            fitView
            minZoom={0.25}
            maxZoom={1.7}
          >
            <Background gap={24} size={1} />
            <Controls position="bottom-left" />
            <MiniMap position="bottom-right" pannable zoomable />
          </ReactFlow>
        </section>

        <DetailPanel
          selectedElement={selectedElement}
          view={view}
          model={model}
          scope={scope}
          warningIssues={warningIssues}
          onDrillDown={setScope}
        />
      </main>

      <footer className="statusbar">
        <span className={model.validation.ok && warningIssues.length === 0 ? "status-ok" : "status-warning"}>
          {model.validation.ok && warningIssues.length === 0 ? (
            <CheckCircle2 size={15} />
          ) : (
            <AlertTriangle size={15} />
          )}
          {validationStatusText(model, warningIssues)}
        </span>
        <span>{status}</span>
        <span>{layoutStatus}</span>
        <span>{view.nodes.length} nodes</span>
        <span>{view.edges.length} edges</span>
      </footer>
    </div>
  );
}

function DetailPanel({
  selectedElement,
  view,
  model,
  scope,
  warningIssues,
  onDrillDown,
}: {
  selectedElement: ElementNode | null;
  view: DerivedView;
  model: EffectiveModel;
  scope: ViewScope;
  warningIssues: ValidationIssue[];
  onDrillDown: (scope: ViewScope) => void;
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
            <dd>{validationStatusText(model, warningIssues)}</dd>
          </dl>
          {model.validation.ok && warningIssues.length > 0 ? (
            <>
              <h3>Warnings</h3>
              <div className="validation-list">
                {warningIssues.map((issue, index) => (
                  <div className="validation-item" key={`${issue.code}-${issue.path ?? "model"}-${index}`}>
                    <strong>{issue.code}</strong>
                    <span>{issue.message}</span>
                    {issue.path ? <span>{issue.path}</span> : null}
                  </div>
                ))}
              </div>
            </>
          ) : null}
        </>
      )}
    </aside>
  );
}

function validationStatusText(model: EffectiveModel, warningIssues: ValidationIssue[]): string {
  if (!model.validation.ok) {
    return "Validation issues";
  }

  if (warningIssues.length > 0) {
    return `${warningIssues.length} ${warningIssues.length === 1 ? "warning" : "warnings"}`;
  }

  return "Valid model";
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
