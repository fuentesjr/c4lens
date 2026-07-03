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

export function serializeViewToPdfBase64(nodes: C4FlowNode[], edges: Edge[], title: string): string {
  const bounds =
    nodes.length === 0
      ? { minX: 0, minY: 0, maxX: DEFAULT_WIDTH - EXPORT_PADDING * 2, maxY: DEFAULT_HEIGHT - EXPORT_PADDING * 2 }
      : graphBounds(nodes);
  const width = Math.ceil(bounds.maxX - bounds.minX + EXPORT_PADDING * 2);
  const height = Math.ceil(bounds.maxY - bounds.minY + EXPORT_PADDING * 2);
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const translateX = (x: number) => x - bounds.minX + EXPORT_PADDING;
  const translateY = (y: number) => y - bounds.minY + EXPORT_PADDING;
  const pageY = (y: number) => height - y;
  const commands: string[] = [
    "q",
    "0.984 0.988 0.992 rg",
    `0 0 ${pdfNumber(width)} ${pdfNumber(height)} re f`,
    "Q",
  ];

  commands.push(textCommand(title, 18, EXPORT_PADDING, pageY(24), "0.122 0.161 0.200"));

  edges.forEach((edge) => {
    const source = nodeById.get(edge.source);
    const target = nodeById.get(edge.target);
    if (!source || !target) {
      return;
    }
    const sourceX = translateX(source.position.x + (source.width ?? 0) / 2);
    const sourceY = pageY(translateY(source.position.y + (source.height ?? 0) / 2));
    const targetX = translateX(target.position.x + (target.width ?? 0) / 2);
    const targetY = pageY(translateY(target.position.y + (target.height ?? 0) / 2));
    commands.push(
      "q",
      "0.322 0.380 0.439 RG",
      "1.5 w",
      `${pdfNumber(sourceX)} ${pdfNumber(sourceY)} m ${pdfNumber(targetX)} ${pdfNumber(targetY)} l S`,
      "Q",
    );
    if (typeof edge.label === "string" && edge.label) {
      const labelX = (sourceX + targetX) / 2;
      const labelY = (sourceY + targetY) / 2 + 8;
      commands.push(textCommand(edge.label, 12, labelX, labelY, "0.267 0.318 0.373", "center"));
    }
  });

  nodes.forEach((node) => {
    const x = translateX(node.position.x);
    const y = translateY(node.position.y);
    const nodeWidth = node.width ?? 220;
    const nodeHeight = node.height ?? 96;
    const pdfY = height - y - nodeHeight;
    const accent = node.data.generated ? "0.561 0.247 0.443" : "0.086 0.490 0.498";
    commands.push(
      "q",
      "1 1 1 rg",
      "0.812 0.839 0.875 RG",
      "1 w",
      `${pdfNumber(x)} ${pdfNumber(pdfY)} ${pdfNumber(nodeWidth)} ${pdfNumber(nodeHeight)} re B`,
      `${accent} rg`,
      `${pdfNumber(x)} ${pdfNumber(pdfY)} 5 ${pdfNumber(nodeHeight)} re f`,
      "Q",
      textCommand(truncate(node.data.label, 28), 14, x + 18, pageY(y + 28), "0.122 0.161 0.200"),
      textCommand(node.data.subtitle, 11, x + 18, pageY(y + 48), "0.408 0.455 0.514"),
    );
    if (node.data.tech) {
      commands.push(textCommand(truncate(node.data.tech, 30), 12, x + 18, pageY(y + nodeHeight - 18), "0.322 0.380 0.439"));
    }
  });

  return bytesToBase64(pdfDocumentBytes(width, height, commands.join("\n")));
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

function pdfDocumentBytes(width: number, height: number, content: string): Uint8Array {
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${pdfNumber(width)} ${pdfNumber(height)}] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
  ];
  let pdf = "%PDF-1.4\n";
  const offsets = [0];
  objects.forEach((object, index) => {
    offsets.push(pdf.length);
    pdf += `${index + 1} 0 obj\n${object}\nendobj\n`;
  });
  const xrefOffset = pdf.length;
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  offsets.slice(1).forEach((offset) => {
    pdf += `${offset.toString().padStart(10, "0")} 00000 n \n`;
  });
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`;
  return new TextEncoder().encode(pdf);
}

function textCommand(
  value: string,
  size: number,
  x: number,
  y: number,
  color: string,
  alignment: "left" | "center" = "left",
): string {
  const text = pdfText(value);
  const adjustedX = alignment === "center" ? x - (text.length * size * 0.28) : x;
  return `BT /F1 ${size} Tf ${color} rg ${pdfNumber(adjustedX)} ${pdfNumber(y)} Td (${escapePdfText(text)}) Tj ET`;
}

function pdfNumber(value: number): string {
  return Number.isInteger(value) ? value.toString() : value.toFixed(2);
}

function pdfText(value: string): string {
  return value.replace(/[^\x20-\x7e]/g, "?");
}

function escapePdfText(value: string): string {
  return value.replaceAll("\\", "\\\\").replaceAll("(", "\\(").replaceAll(")", "\\)");
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
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
