# Product Roadmap

This roadmap tracks visible product gaps separately from technical debt. Items
listed here are planned or deferred capabilities, not regressions.

## Phase 2 Backlog

| Item | Status | Notes |
| --- | --- | --- |
| Search | Planned | Enable the topbar search control once the app can search elements, indexed files, and indexed symbols with deterministic, bounded results. |
| SVG/PNG export | Planned | Enable export after the renderer can serialize the current view and the Tauri backend can save SVG or PNG through a native save dialog. |
| Layout caching | Planned | Cache ELK output by source SHA, view scope, layout inputs, layout options, and measured node dimensions. Invalidate on model, source, option, or measurement changes. |
| Generation diff/review UX | Planned | Expand the current accept-all generation flow with a diff summary, generated YAML diff, and eventually per-element or per-change review. |
| macOS packaging | Planned | Produce signed and notarized macOS artifacts for the MVP target before release. |

## Later Backlog

| Item | Status | Notes |
| --- | --- | --- |
| Rendered L4 code-level views | Deferred | Use indexed symbols to render lower-level diagrams once MVP navigation and generation are stable. |
| LSP-backed relationship inference | Deferred | Improve relationship accuracy beyond manifest and import heuristics. |
| Generated slug rename/move preservation | Deferred | Preserve generated slugs across file moves instead of relying on validator-reported dangling references. |
| Multi-repo workspace | Deferred | Support more than one active repository once single-repo workflows are complete. |
| Local agent API | Deferred | Expose app data to local automation only after the desktop workflow is stable. |

## Backlog Rules

- Keep disabled controls only when they map to a roadmap item.
- Move implemented roadmap items into release notes or delete them from this
  file once they ship.
- Track code quality problems in `technical_debt.md`, not here.
