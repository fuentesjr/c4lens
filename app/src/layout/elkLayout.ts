import type { Edge, Node } from "@xyflow/react";
import { MarkerType } from "@xyflow/react";
import ELK, { type ElkNode } from "elkjs/lib/elk.bundled.js";
import type { DerivedView, ViewScope } from "../view_deriver/deriveView";

export type C4NodeData = {
  label: string;
  elementType: string;
  subtitle: string;
  description?: string | null;
  tech?: string | null;
  generated: boolean;
  external: boolean;
  status: string;
} & Record<string, unknown>;

export type C4FlowNode = Node<C4NodeData>;

export const nodeTypes = {
  c4Node: "c4Node",
} as const;

const NODE_WIDTH = 220;
const NODE_HEIGHT = 96;
const LAYOUT_VERSION = 1;
const DEFAULT_LAYOUT_OPTIONS = {
  "elk.algorithm": "layered",
  "elk.direction": "RIGHT",
  "elk.spacing.nodeNode": "80",
  "elk.spacing.edgeNode": "40",
  "elk.spacing.edgeEdge": "20",
  "elk.layered.spacing.nodeNodeBetweenLayers": "120",
};

const elk = new ELK();
const layoutCache = new Map<string, LayoutResult>();

type LayoutResult = {
  nodes: C4FlowNode[];
  edges: Edge[];
};

export type LayoutCacheOptions = {
  sourceSha?: string;
  scope?: ViewScope;
  nodeDimensions?: Record<string, { width: number; height: number }>;
  layoutOptions?: Record<string, string>;
};

export function clearLayoutCache() {
  layoutCache.clear();
}

export function layoutCacheKeyFor(
  view: DerivedView,
  options: LayoutCacheOptions & { sourceSha: string; scope: ViewScope },
): string {
  const layoutOptions = { ...DEFAULT_LAYOUT_OPTIONS, ...options.layoutOptions };
  const nodeDimensions = options.nodeDimensions ?? {};
  const layoutInput = {
    version: LAYOUT_VERSION,
    layoutOptions: sortedRecord(layoutOptions),
    nodes: view.nodes.map((node) => ({
      id: node.id,
      width: nodeDimensions[node.id]?.width ?? NODE_WIDTH,
      height: nodeDimensions[node.id]?.height ?? NODE_HEIGHT,
      name: node.element.name,
      type: node.element.type,
      kind: node.element.kind ?? null,
      external: Boolean(node.element.external),
      generated: node.element.generated,
      status: node.element.status,
      tech: node.element.tech ?? null,
    })),
    edges: view.edges.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      label: edge.label,
      generated: edge.generated,
    })),
  };
  return [
    options.sourceSha,
    options.scope.level,
    options.scope.slug ?? "root",
    stableHash(JSON.stringify(layoutInput)),
  ].join(":");
}

export async function layoutWithElk(view: DerivedView, options: LayoutCacheOptions = {}): Promise<LayoutResult> {
  const layoutOptions = { ...DEFAULT_LAYOUT_OPTIONS, ...options.layoutOptions };
  const nodeDimensions = options.nodeDimensions ?? {};
  const cacheKey =
    options.sourceSha && options.scope
      ? layoutCacheKeyFor(view, {
          sourceSha: options.sourceSha,
          scope: options.scope,
          nodeDimensions,
          layoutOptions,
        })
      : null;
  if (cacheKey) {
    const cached = layoutCache.get(cacheKey);
    if (cached) {
      return cached;
    }
  }

  const graph: ElkNode = {
    id: "root",
    layoutOptions,
    children: view.nodes.map((node) => ({
      id: node.id,
      width: nodeDimensions[node.id]?.width ?? NODE_WIDTH,
      height: nodeDimensions[node.id]?.height ?? NODE_HEIGHT,
    })),
    edges: view.edges.map((edge) => ({
      id: edge.id,
      sources: [edge.source],
      targets: [edge.target],
    })),
  };

  const layouted = await elk.layout(graph);
  const positions = new Map((layouted.children ?? []).map((node) => [node.id, node]));

  const result = {
    nodes: view.nodes.map((node, index) => {
      const position = positions.get(node.id);
      return {
        id: node.id,
        type: nodeTypes.c4Node,
        position: {
          x: position?.x ?? (index % 3) * 280,
          y: position?.y ?? Math.floor(index / 3) * 150,
        },
        width: nodeDimensions[node.id]?.width ?? NODE_WIDTH,
        height: nodeDimensions[node.id]?.height ?? NODE_HEIGHT,
        data: {
          label: node.element.name,
          elementType: node.element.type,
          subtitle: node.element.external ? "external system" : node.element.kind ?? node.element.type,
          description: node.element.description,
          tech: node.element.tech,
          generated: node.element.generated,
          external: Boolean(node.element.external),
          status: node.element.status,
        },
      };
    }),
    edges: view.edges.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      label: edge.label,
      type: "smoothstep",
      animated: edge.generated,
      markerEnd: {
        type: MarkerType.ArrowClosed,
      },
      data: {
        generated: edge.generated,
      },
    })),
  };

  if (cacheKey) {
    layoutCache.set(cacheKey, result);
  }

  return result;
}

function stableHash(value: string): string {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index));
    hash = (hash * prime) & mask;
  }
  return hash.toString(16).padStart(16, "0");
}

function sortedRecord(record: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(record).sort(([left], [right]) => left.localeCompare(right)));
}
