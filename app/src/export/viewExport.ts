import type { Edge } from "@xyflow/react";
import type { C4FlowNode } from "../layout/elkLayout";

export type SerializedSvg = {
  svg: string;
  width: number;
  height: number;
};

const EXPORT_PADDING = 48;
const DEFAULT_WIDTH = 640;
const DEFAULT_HEIGHT = 360;

export function serializeViewToSvg(nodes: C4FlowNode[], edges: Edge[], title: string): SerializedSvg {
  if (nodes.length === 0) {
    return {
      width: DEFAULT_WIDTH,
      height: DEFAULT_HEIGHT,
      svg: `<svg xmlns="http://www.w3.org/2000/svg" width="${DEFAULT_WIDTH}" height="${DEFAULT_HEIGHT}" viewBox="0 0 ${DEFAULT_WIDTH} ${DEFAULT_HEIGHT}"><title>${escapeXml(title)}</title></svg>`,
    };
  }

  const bounds = graphBounds(nodes);
  const width = Math.ceil(bounds.maxX - bounds.minX + EXPORT_PADDING * 2);
  const height = Math.ceil(bounds.maxY - bounds.minY + EXPORT_PADDING * 2);
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const translateX = (x: number) => x - bounds.minX + EXPORT_PADDING;
  const translateY = (y: number) => y - bounds.minY + EXPORT_PADDING;

  const edgeMarkup = edges
    .map((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      if (!source || !target) {
        return "";
      }
      const sourceX = translateX(source.position.x + (source.width ?? 0) / 2);
      const sourceY = translateY(source.position.y + (source.height ?? 0) / 2);
      const targetX = translateX(target.position.x + (target.width ?? 0) / 2);
      const targetY = translateY(target.position.y + (target.height ?? 0) / 2);
      const label = typeof edge.label === "string" ? edge.label : "";
      const labelX = (sourceX + targetX) / 2;
      const labelY = (sourceY + targetY) / 2 - 8;
      return `<g><line x1="${sourceX}" y1="${sourceY}" x2="${targetX}" y2="${targetY}" stroke="#526170" stroke-width="1.5" marker-end="url(#arrow)" />${label ? `<text x="${labelX}" y="${labelY}" text-anchor="middle" fill="#44515f" font-size="12">${escapeXml(label)}</text>` : ""}</g>`;
    })
    .join("");

  const nodeMarkup = nodes
    .map((node) => {
      const x = translateX(node.position.x);
      const y = translateY(node.position.y);
      const width = node.width ?? 220;
      const height = node.height ?? 96;
      const accent = node.data.generated ? "#8f3f71" : "#167d7f";
      return `<g><rect x="${x}" y="${y}" width="${width}" height="${height}" rx="8" fill="#ffffff" stroke="#cfd6df" /><rect x="${x}" y="${y}" width="5" height="${height}" rx="2" fill="${accent}" /><text x="${x + 18}" y="${y + 28}" fill="#1f2933" font-size="14" font-weight="700">${escapeXml(truncate(node.data.label, 28))}</text><text x="${x + 18}" y="${y + 48}" fill="#687483" font-size="11">${escapeXml(node.data.subtitle)}</text>${node.data.tech ? `<text x="${x + 18}" y="${y + height - 18}" fill="#526170" font-size="12">${escapeXml(truncate(node.data.tech, 30))}</text>` : ""}</g>`;
    })
    .join("");

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><title>${escapeXml(title)}</title><defs><marker id="arrow" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto" markerUnits="strokeWidth"><path d="M0,0 L0,6 L9,3 z" fill="#526170" /></marker></defs><rect width="100%" height="100%" fill="#fbfcfd" />${edgeMarkup}${nodeMarkup}</svg>`;
  return { svg, width, height };
}

export async function svgToPngBase64(serialized: SerializedSvg): Promise<string> {
  const image = new Image();
  const blob = new Blob([serialized.svg], { type: "image/svg+xml;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  try {
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve();
      image.onerror = () => reject(new Error("Unable to render SVG for PNG export"));
      image.src = url;
    });

    const canvas = document.createElement("canvas");
    canvas.width = serialized.width;
    canvas.height = serialized.height;
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Canvas export is unavailable");
    }
    context.drawImage(image, 0, 0);
    return canvas.toDataURL("image/png").split(",")[1] ?? "";
  } finally {
    URL.revokeObjectURL(url);
  }
}

function graphBounds(nodes: C4FlowNode[]) {
  return nodes.reduce(
    (bounds, node) => ({
      minX: Math.min(bounds.minX, node.position.x),
      minY: Math.min(bounds.minY, node.position.y),
      maxX: Math.max(bounds.maxX, node.position.x + (node.width ?? 0)),
      maxY: Math.max(bounds.maxY, node.position.y + (node.height ?? 0)),
    }),
    {
      minX: Number.POSITIVE_INFINITY,
      minY: Number.POSITIVE_INFINITY,
      maxX: Number.NEGATIVE_INFINITY,
      maxY: Number.NEGATIVE_INFINITY,
    },
  );
}

function truncate(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, Math.max(0, maxLength - 3))}...`;
}

function escapeXml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
