import { describe, expect, it } from "vitest";
import { sampleModel } from "../model/sample";
import { deriveView } from "../view_deriver/deriveView";
import { clearLayoutCache, layoutCacheKeyFor, layoutWithElk } from "./elkLayout";

describe("layoutWithElk", () => {
  it("produces finite, non-overlapping node positions for the sample context view", async () => {
    const view = deriveView(sampleModel, { level: "context" });
    const layout = await layoutWithElk(view);

    expect(layout.nodes).toHaveLength(3);
    expect(layout.edges).toHaveLength(2);

    layout.nodes.forEach((node) => {
      expect(Number.isFinite(node.position.x)).toBe(true);
      expect(Number.isFinite(node.position.y)).toBe(true);
    });

    for (let index = 0; index < layout.nodes.length; index += 1) {
      for (let next = index + 1; next < layout.nodes.length; next += 1) {
        expect(overlaps(layout.nodes[index], layout.nodes[next])).toBe(false);
      }
    }
  });

  it("reuses cached layout results for the same source, scope, and input", async () => {
    clearLayoutCache();
    const scope = { level: "context" as const };
    const view = deriveView(sampleModel, scope);

    const first = await layoutWithElk(view, { sourceSha: sampleModel.sourceSha, scope });
    const second = await layoutWithElk(view, { sourceSha: sampleModel.sourceSha, scope });

    expect(second).toBe(first);
  });

  it("changes cache keys when source, scope, or layout input changes", () => {
    const contextScope = { level: "context" as const };
    const containerScope = { level: "container" as const, slug: "acme_api" };
    const contextView = deriveView(sampleModel, contextScope);
    const containerView = deriveView(sampleModel, containerScope);

    const contextKey = layoutCacheKeyFor(contextView, {
      sourceSha: sampleModel.sourceSha,
      scope: contextScope,
    });
    const changedSourceKey = layoutCacheKeyFor(contextView, {
      sourceSha: "another-source",
      scope: contextScope,
    });
    const containerKey = layoutCacheKeyFor(containerView, {
      sourceSha: sampleModel.sourceSha,
      scope: containerScope,
    });

    expect(changedSourceKey).not.toBe(contextKey);
    expect(containerKey).not.toBe(contextKey);
  });
});

function overlaps(
  left: { position: { x: number; y: number }; width?: number; height?: number },
  right: { position: { x: number; y: number }; width?: number; height?: number },
) {
  const leftWidth = left.width ?? 0;
  const leftHeight = left.height ?? 0;
  const rightWidth = right.width ?? 0;
  const rightHeight = right.height ?? 0;

  return !(
    left.position.x + leftWidth <= right.position.x ||
    right.position.x + rightWidth <= left.position.x ||
    left.position.y + leftHeight <= right.position.y ||
    right.position.y + rightHeight <= left.position.y
  );
}
