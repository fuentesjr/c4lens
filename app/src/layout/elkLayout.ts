import type { Edge, Node } from "@xyflow/react";
import { MarkerType } from "@xyflow/react";
import ELK, { type ElkNode } from "elkjs/lib/elk.bundled.js";
import type { DerivedView } from "../view_deriver/deriveView";

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

const elk = new ELK();

export async function layoutWithElk(view: DerivedView): Promise<{
  nodes: C4FlowNode[];
  edges: Edge[];
}> {
  const graph: ElkNode = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.spacing.nodeNode": "80",
      "elk.spacing.edgeNode": "40",
      "elk.spacing.edgeEdge": "20",
      "elk.layered.spacing.nodeNodeBetweenLayers": "120",
    },
    children: view.nodes.map((node) => ({
      id: node.id,
      width: NODE_WIDTH,
      height: NODE_HEIGHT,
    })),
    edges: view.edges.map((edge) => ({
      id: edge.id,
      sources: [edge.source],
      targets: [edge.target],
    })),
  };

  const layouted = await elk.layout(graph);
  const positions = new Map((layouted.children ?? []).map((node) => [node.id, node]));

  return {
    nodes: view.nodes.map((node, index) => {
      const position = positions.get(node.id);
      return {
        id: node.id,
        type: nodeTypes.c4Node,
        position: {
          x: position?.x ?? (index % 3) * 280,
          y: position?.y ?? Math.floor(index / 3) * 150,
        },
        width: NODE_WIDTH,
        height: NODE_HEIGHT,
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
}
