# Product Roadmap

This roadmap tracks visible product gaps separately from technical debt. Items
listed here are planned or deferred capabilities, not regressions.

## Phase 2 Backlog

| Item | Status | Notes |
| --- | --- | --- |
| Search | Implemented | Topbar search queries elements, indexed files, and indexed symbols with deterministic, bounded results. |
| SVG/PNG export | Implemented | The renderer serializes the current view and the Tauri backend saves SVG or PNG through a native save dialog. |
| Layout caching | Implemented | ELK output is cached by source SHA, view scope, and stable layout input. Model/source/scope/input changes produce a new cache key. |
| Generation diff/review UX | Implemented | The app shows a diff summary, change list, generated YAML preview, and accept-all apply action. |
| Generated provenance badges | Implemented | Generated elements and relationships are visibly marked in the canvas and detail panel. |
| Dependency highlighting and focus mode | Implemented | Hovering or selecting a node highlights connected dependencies; Linked focus mode dims unrelated nodes. |
| macOS packaging | Ready | `npm run tauri:build:macos` builds unsigned universal macOS app and DMG artifacts for internal development. Signing/notarization remains a release step. |

## MVP Hardening

| Item | Status | Notes |
| --- | --- | --- |
| Persisted color theme | Implemented | The renderer stores the selected light/dark theme locally and reapplies it on startup. |
| Last repository restore | Implemented | The desktop shell remembers the last opened repository path and reopens it when no active model is already loaded. |
| Layout cache acceptance inputs | Implemented | ELK cache keys include layout options and measured node dimensions, not only model/source/scope data. |
| MVP smoke command | Implemented | `npm run smoke:mvp` validates the CLI path from temporary repo creation through scan, generate, write, and drift check. |
| Roadmap and quality-gate docs | Implemented | README and roadmap now document the MVP hardening gate and remaining deferred work. |

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
