# c4lens — Desktop (Tauri) Design Document

| | |
|---|---|
| **Status** | Draft for review |
| **Date** | 2026-06-28 |
| **Owner** | fuentesjr |
| **Relationship** | Chosen desktop baseline. The earlier web option is no longer active and is not a dependency of this document. |
| **One-liner** | A local desktop app that renders a YAML C4 model into an interactive navigator **and generates that model from your codebase** — because it can read your code directly. |
| **Thesis** | *Navigate your architecture — and generate it from your code.* |

> **Naming note.** The exploration was framed as "the Electron alternative." After weighing the shell options (§3, D-P1) the design commits to **Tauri**, so this file is named `c4lens-desktop-design.md` rather than `…-electron-design.md`. "Desktop" is the accurate umbrella.

---

## 1. Overview

c4lens-desktop is a model-as-data C4 navigator where an AI agent or a human authors a YAML model and the app renders the derived C4 views as an interactive, auto-laid-out canvas, packaged as a **local desktop application**.

The reason to leave the browser is a single capability the web app structurally cannot have: **direct, trusted access to the local filesystem and the codebase.** That unlocks the one part of C4 the hosted design had to defer — the **Code** abstraction and *code-derived modeling* — and turns it into the desktop variant's reason to exist:

1. **Open source locally** — a component's `code` link opens the real file in your editor (or previews it inline), not a GitHub URL.
2. **Generate the model from the repo** — scan the codebase and produce (or refresh) `c4/model.generated.yml`: containers from build manifests, components from module structure, relationships from imports. The hardest part of adoption — writing the first model — is largely automated without clobbering authored `c4/model.yml`.
3. **Index code elements** — a persisted SQLite index of files/symbols/imports powers jump-to-code and generation now, and makes C4 Level-4 (Code) views a cheap future addition.

The product stays model-first: text in, interactive exploration out, automatic layout, no drag-to-draw editor. The desktop app adds code in, model out.

### Core Product Commitments

The product and domain decisions are:

- Model-as-data, not a drawing tool; model-first.
- YAML (JSON accepted) + one JSON Schema as the source contract; file-in-repo delivery.
- Automatic layout via ELK; no stored coordinates; auto-derived Context/Container/Component views.
- C4 vocabulary — `system`, `container`, `component`, `actor` — and the four-abstraction model.
- The model file format, schema, validation rules, merge rules, IPC contract, and scanner/generator behavior are specified normatively in [`c4lens-desktop-spec.md`](../spec/c4lens-desktop-spec.md).
- The navigator interaction model includes drill-down, pan/zoom, dependency highlighting, detail panel, deep links, search, and export.
- The `ViewDeriver` boundary-aggregation algorithm runs in the renderer (D-P5).

### What is re-decided for desktop

Only the **platform** layer: the app shell, process model, persistence, IPC, the code scanner/generator/index, packaging, and where validation and view-derivation run. These are §3's `D-P*` decisions.

### Positioning — Hosted Web vs Desktop

| Axis | Hosted web app | **c4lens (Desktop / Tauri)** |
|---|---|---|
| Delivery | Self-hosted web app | Installable native app (macOS/Windows/Linux) |
| Source of truth | `c4/model.yml` in repo | `c4/model.yml` plus generated overlay in repo |
| Code access | `code` links → GitHub/GitLab URL | **Local files**: open in editor, inline preview, scan & index |
| Model authoring | AI/human writes the file | AI/human writes the file **+ generate/refresh from codebase** |
| Code abstraction (L4) | Deferred (links only) | **Indexed in MVP**; L4 views are a cheap Phase 3 |
| Backend | Server + HTTP | Rust core + SQLite (embedded) + IPC; no server, no HTTP |
| Ops surface | Run a server | Double-click an app |
| Offline | Needs the server running | Fully offline by construction |

Against the wider field (Excalidraw / Mermaid / Structurizr / IcePanel), c4lens occupies a local, single-user, AI-authorable, navigable niche and extends it with **code intelligence no hosted tool can match**, since it runs on your machine with your repo. IcePanel itself documents Level-4/code as *not diagrammed* and recommends "link to code/reality instead" (`.misc/research/icepanel/c4-comparison.md`); the desktop app is precisely the form factor that can go further.

---

## 2. Goals and non-goals

### Goals (MVP)

MVP goals:

- **Local jump-to-code** — open a component's source in the user's editor; preview it inline in the detail panel.
- **Generate the model from a codebase** — scan a repo and emit/refresh `c4/model.generated.yml` (containers, components, best-effort relationships) without clobbering hand- or agent-authored `c4/model.yml`.
- **Persisted code index** — a SQLite index of files/symbols/imports, built incrementally, so re-opening a large repo is fast and generation/jump-to-code are instant.
- **Zero-ops** — install and run; no server process, no localhost, no ports.
- **Live refresh** — file-watch `c4/model.yml`, `c4/model.generated.yml`, `c4/schema.json`, and indexed source files, then re-import or re-index automatically.

### Non-goals (MVP — see §10 roadmap)

MVP non-goals:

- **No rendered L4 code-level *views* in MVP.** MVP *indexes* code elements (classes/functions/interfaces) and links to them; rendering an auto-laid-out Code diagram is the first roadmap item (the index makes it cheap).
- **No multi-repo workspace** — one repo per window in MVP (multi-repo is roadmap).
- **No account, sync, or cloud** — strictly local and single-user.
- **No live write API / MCP push** — agents author via the file (and via the generator); a *local* MCP server the app exposes is roadmap.
- **No fully-automatic relationship inference beyond cheap signals** — generation infers intra-repo import edges and manifest-declared external deps, flags them as generated, and leaves the human/agent to refine.

---

## 3. Design decisions

Below are the platform decisions (`D-P*`) for the desktop product. Normative implementation details live in [`c4lens-desktop-spec.md`](../spec/c4lens-desktop-spec.md); this design doc explains why the shape is right.

### D-P1 — App shell: Tauri (Rust core + OS webview)
The app is a Tauri 2 application: a Rust "core" (the main process) and a webview "renderer" running the React/xyflow/ELK UI, communicating over Tauri commands and events. **Why:** three of the project's constraints point here. (1) *"As small as possible but useful"* — a Tauri bundle is ~10–40 MB vs Electron's ~150 MB+, with lower memory. (2) The headline feature is *local code parsing*, and Rust is its home turf — `tree-sitter` (Rust-native), ripgrep's `ignore`/`walkdir` for fast repo walks, `notify` for file-watching, embedded SQLite via `rusqlite`, and LSP clients all sit in the standard Rust ecosystem. (3) Rust matches the owner's stack taste. The interactivity-critical surface — the canvas — is the *same* React + xyflow + ELK bundle regardless of shell, and renders well in WKWebView (macOS) and Chromium-based WebView2 (Windows). **Rejected — Electron:** the most-trodden React-canvas path with bundled Chromium (uniform rendering on every OS, richest Node ecosystem), but ~10× larger, heavier, and it pushes code-parsing into Node where the libraries are less natural. Its one decisive edge — identical rendering everywhere — only matters on Linux (WebKitGTK is Tauri's weak spot); on macOS/Windows the webviews are strong. **This decision is low-regret and reversible:** the renderer is identical, so the ~10% native shell could be swapped to Electron later with little loss.

### D-P2 — Local code access is a core feature, not roadmap
Code-derived capability is in the MVP core. **Why:** a desktop app that only reads a local `model.yml` would not justify leaving the browser; the entire desktop thesis is code *intelligence*. **Rejected:** model-only desktop parity — would leave the desktop app without a distinct reason to exist.

### D-P3 — Generate the model from the codebase (overlay + provenance)
The app can scan a repo and produce/refresh the model. Generated content is written to a **separate overlay file, `c4/model.generated.yml`**; the authored `c4/model.yml` is merged *over* it, so human/agent edits always win. **Why:** generation must be *idempotent* and *non-destructive* — surgically editing a hand-authored file on every regenerate is the fragile "round-trip" problem IcePanel explicitly warns about (`.misc/research/icepanel/how-icepanel-works.md`, "export-modify-import"). An overlay makes regeneration a clean overwrite of one file, preserves the file-as-truth invariant, and lets the user commit or `.gitignore` the generated file at will. **Rejected:** in-place generation with `# c4lens:generated` regions (one file, but fragile and clobber-prone on re-run); DB-as-generated-truth (breaks file-as-truth and PR-reviewability).

### D-P4 — SQLite as a persisted code index/cache, never the source of truth
A local SQLite database (embedded via `rusqlite`, bundled engine) stores the **code index** (files, symbols, imports, element→source mappings) and an optional **derived-model cache**. It is keyed by content hashes for incremental rescans. The model files and codebase remain canonical; SQLite is always rebuildable from them. **Why:** scanning/parsing a large repo is expensive; caching it makes re-opening instant and powers fast code queries. **Rejected:** pure in-memory (re-scans the whole repo on every launch — too slow for real codebases); JSON-file cache (no indexed queries for symbols/imports); `tauri-plugin-sql` (runs SQL from the webview — we want the index owned by the Rust core, next to the scanner).

### D-P5 — `ViewDeriver` and ELK layout run in the renderer; the Rust core is the model/code provider
The Rust core owns file IO, parse, validate, scan, generate, index, and watch, and hands the **validated model graph** to the renderer. The renderer (TypeScript) runs the `ViewDeriver` boundary-aggregation algorithm, ELK layout, and xyflow rendering. **Why:** drill-down must be instant, and deriving a view from the small in-memory model with no IPC round-trip keeps navigation snappy; the deriver is tightly coupled to xyflow's node/edge shapes and to layout caching, so it belongs beside them. **Rejected:** deriver in Rust (clean for a headless CLI, but adds an IPC hop per drill-down and a second implementation; headless export is roadmap and can reuse the renderer in a hidden window).

### D-P6 — Replace the HTTP/Inertia layer with IPC; in-webview routing
There is no server and no HTTP layer. The renderer calls Rust **commands** (`invoke`) and subscribes to **events** (`listen`); navigation state lives in an in-webview router. **Why:** a local app needs no network transport; Tauri's typed command/event bridge is the natural replacement and removes unnecessary API/router/auth plumbing. **Rejected:** embedding a local HTTP server in the app (needless ports/CORS/attack surface for a single-process tool).

### D-P7 — Validation, scanning, and generation are also a headless CLI
The Rust core's pipeline is exposed as a small CLI (`c4lens validate`, `c4lens generate`, `c4lens scan`) sharing the same code. **Why:** `c4lens validate` can fail CI on an invalid model, and agents can generate/validate without launching the GUI. **Rejected:** GUI-only (loses CI and headless agent use).

---

## 4. Domain model & C4 primer

The domain model uses the four C4 abstractions (System -> Container -> Component -> Code), `actor` for people, relationships as directed edges at any level, the model-first invariant, and the minimal `live`/`planned`/`deprecated` lifecycle. The normative hierarchy, field contract, merge behavior, and validation rules are in the implementation spec.

### The Code abstraction, finally tractable

Per [c4model.com/abstractions/code](https://c4model.com/abstractions/code), Code is C4's optional 4th level — the code elements (classes, interfaces, enums, functions, objects) that implement a component — best **generated from source on demand**, not hand-authored. The desktop app can see local source, so it advances the Code abstraction in three concrete steps:

| Capability | Hosted/web baseline | Desktop MVP | Desktop roadmap |
|---|---|---|---|
| `code` link on a component | URL only | **Opens local file / inline preview** | — |
| Code elements (symbols) | not modeled | **Indexed** (tree-sitter → SQLite) | — |
| Component → source mapping | manual link | **Generated** from the scan | refined via LSP |
| L4 Code *view* (auto-laid-out) | — | data exists, view not rendered | **Rendered** (xyflow + ELK over indexed symbols) |

The key point: by indexing symbols in the MVP (for generation and jump-to-code), the data needed for true L4 views already exists, so rendering them later is a UI task, not a new parsing project. The desktop variant is the form factor where C4's 4th abstraction stops being theoretical.

---

## 5. The model file

The file format, location (`c4/model.yml`, `c4/model.generated.yml`, and `c4/schema.json`), identity/slug rules, relationship shorthand, defaults, and the validation pipeline are defined in [`c4lens-desktop-spec.md`](../spec/c4lens-desktop-spec.md). The same bundled schema validates in the Rust core (via the `jsonschema` crate) and in any editor wired to `schema.json`.

### Delta: the generated overlay (D-P3)

Generation introduces one new file alongside the authored model:

```
c4/
  model.yml            # authored by humans/agents — source of truth, wins on conflict
  model.generated.yml  # accepted generated overlay — same schema, regenerated idempotently
  schema.json          # JSON Schema — validates both files
```

**Effective model = merge(model.generated.yml, model.yml)** with the authored file winning:

- Elements are unioned by slug; authored fields win for the same element, while generated child containers/components remain available unless those child slugs are also authored.
- Relationships are unioned and de-duplicated; authored relationships are additive.
- Generated elements carry a provenance marker (`generated: true`, schema-additive) so the UI can badge them and the merge can reason about them. A later "tombstone" to suppress a specific generated element is roadmap.
- Slugs for generated elements are **path-derived and stable** (e.g., a container slug from its manifest directory). Moving a file changes its slug — handled as delete+add in the overlay; authored relationships referencing a moved generated element surface as dangling refs, which validation flags. **Rename/move detection that preserves slugs across moves is post-MVP** (§10); for the MVP, validator-flagged dangling refs are the intended behavior.

This keeps the important guarantees — diffable YAML, PR-reviewable changes, file-as-truth, idempotent import — while making generation safe to re-run on every scan.

---

## 6. Architecture

### Process model & data flow

```
        ┌──────────────────────────── Tauri app (one process tree) ────────────────────────────┐
        │                                                                                        │
  repo/ │   RUST CORE  (main)                                       RENDERER  (webview)          │
  ├ src │   ┌─────────────────────────────────┐                    ┌──────────────────────────┐ │
  │     │   │ Watcher (notify)                │                     │ Model store (TS)         │ │
  ├ c4/ │   │ Parser + Validator (jsonschema) │  ── invoke ──▶      │   │                      │ │
  │ ├ model.yml ─────────────────────────────┐│  ◀── events ──     │ ViewDeriver (TS, reused) │ │
  │ ├ model.generated.yml  ◀── apply ─────────┘│      (model,       │   │                      │ │
  │ └ schema.json                              │       progress,    │ ELK layout (elkjs)       │ │
  │     │   │ Scanner/Generator               │       validation)   │   │                      │ │
  │     │   │   (tree-sitter, ignore/walkdir) │                     │ xyflow navigator         │ │
  └─────┘   │ Indexer ──▶ SQLite (rusqlite)   │                     │ detail panel + jump-to   │ │
            └─────────────────────────────────┘                    └──────────────────────────┘ │
        │       source of truth: model files + codebase                                          │
        │       derived/cached:  SQLite index + derived-model cache                              │
        └────────────────────────────────────────────────────────────────────────────────────────┘
```

### Components

**Rust core:**

- **Watcher** (`notify`): watches `c4/model.yml`, `c4/model.generated.yml`, `c4/schema.json`, and indexed source paths; triggers re-import or incremental re-index; emits repo-scoped events.
- **Parser + Validator** (`serde_yaml_ng`/`serde_norway` for YAML — the original `serde_yaml` is archived — + the `jsonschema` crate): runs the validation pipeline from the implementation spec. Shared by the GUI, the watcher, and the CLI.
- **Scanner/Generator** (`tree-sitter` + per-language grammar crates; `ignore`/`walkdir` for a `.gitignore`-respecting walk): builds the code index and produces `model.generated.yml` (heuristics in §12.A).
- **Indexer → SQLite** (`rusqlite`, bundled engine): persists files/symbols/imports/element→source mappings and the derived-model cache, keyed by content hash for incremental rescans (schema in §12.B).
- **Command/event surface** (§ "IPC contract"): the typed bridge between renderer and core.

**Renderer:**

- **Model store** (lightweight — Zustand, or React context if that suffices): holds the validated effective model, navigation state, and index query results.
- **ViewDeriver** (TypeScript): given a scope (context / container / component view), returns nodes and edges, aggregating lower-level relationships to the correct boundary.
- **Layout** (`elkjs`): client-side ELK, deterministic per input, cached by source, scope, layout options, and measured node dimensions.
- **Navigator** (xyflow): pan/zoom/minimap, rich C4 cards, drill-down, dependency highlighting.
- **Detail panel**: element metadata, relationships, **and the desktop additions** — inline source preview and an "open in editor" affordance.
- **Export**: xyflow → SVG/PNG, written to disk via a native save dialog (`tauri-plugin-dialog`/`-fs`) instead of a browser download.

### IPC contract (commands ↔ events)

```
COMMANDS (renderer → core, via invoke)
  open_repo(path?) -> RepoHandle          # path optional; falls back to native folder picker
  get_model() -> EffectiveModel           # merged, validated graph (model.yml ⊕ generated)
  validate_model() -> ValidationReport
  scan_codebase(opts) -> ScanSummary      # build/refresh the SQLite index
  generate_model(opts) -> GenerationDiff  # produce a reviewable overlay candidate
  apply_generated(selection) -> ()        # write the accepted overlay to disk
  get_element_code(slug) -> CodeRef        # { path, range, snippet } from the index
  open_in_editor(path, line?) -> ()        # via tauri-plugin-opener / shell
  search(query) -> SearchResults
  export_view(format, scope) -> SavedPath  # svg|png, via save dialog

EVENTS (core → renderer, via emit/listen)
  model-changed        # repoId + validation/source hash; renderer refetches
  scan-progress        # repoId + { done, total } during a scan
  index-updated        # repoId + summary; incremental re-index completed
  validation-failed    # repoId + report; UI keeps the last good model
```

These operations are expressed as local IPC instead of HTTP. In-app deep-linking is handled by an in-webview router (memory/hash history) keyed by `(scope, selected)`; a `c4lens://` custom URI scheme for cross-launch deep links is roadmap.

### Why There Is No Domain ERD

The validated model lives in memory in the renderer because it is small and fully derivable from files. SQLite is instead the **code index** (§12.B). A derived-model cache table is optional and exists only to skip re-parsing on launch; it is rebuilt from the files whenever `source_sha` changes.

---

## 7. Navigator UX

The navigator has a center canvas, top breadcrumb (the IcePanel "Belong To" up/across idea), right detail panel, drill-down, pan/zoom/fit/minimap, select -> detail, hover/select -> dependency highlighting + focus mode, search/jump-to, deep links, light/dark, and C4 card visual language. Desktop-specific behavior:

- **Drill-down.** Double-clicking a system opens its container view; double-clicking a container opens its component view. The detail panel also exposes the same action as a visible keyboard-accessible button (`Open containers` / `Open components`) so the interaction is discoverable. Drill-down changes the C4 scope; it still renders scoped children plus relevant outside dependency neighbors. Breadcrumb/scope navigation is the canonical up/back path.
- **Jump-to-code (real).** The detail panel's `code` link opens the actual file at the right line in the user's editor (`open_in_editor`), reveals it in Finder/Explorer, **and** can render an inline syntax-highlighted preview pulled from the index — no network, no GitHub URL.
- **Generation UX.** A "Generate / refresh from code" action runs `scan_codebase` + `generate_model` and presents a **reviewable diff** of `model.generated.yml` before writing (`apply_generated`). The user accepts the whole diff or per-element; nothing touches `model.yml`. A progress indicator is driven by `scan-progress`.
- **Provenance badges.** Elements that came from the generator are visually marked (from the `generated: true` provenance), so it's always clear what was authored vs derived.
- **Native chrome.** Native menus, a folder picker to open a repo, native save dialogs for export, and OS notifications on watch-triggered refresh.

---

## 8. Tech stack

Every dependency earns its place.

| Layer | Web-app analogue | **Desktop (Tauri) choice** | Why |
|---|---|---|---|
| Shell | Server app | **Tauri 2** | Small, native, Rust core (D-P1). |
| Core language | Ruby | **Rust** | File IO, parsing, code intelligence (D-P1). |
| UI runtime | Browser (any) | **OS webview** (WKWebView/WebView2/WebKitGTK) | Ships the same React app natively. |
| Store | SQLite (server, ActiveRecord) | **SQLite embedded (`rusqlite`, bundled)** | Code index + cache; never the truth (D-P4). |
| Client bridge | Inertia.js over HTTP | **Tauri commands/events (IPC)** | No server/API needed (D-P6). |
| UI framework | React | **React** | xyflow's reference framework. |
| Canvas | xyflow / React Flow | **xyflow / React Flow** | Pan/zoom + rich DOM cards. |
| Layout | ELK (`elkjs`, client) | **ELK (`elkjs`, renderer)** | Deterministic client-side graph layout (D-P5). |
| Bundler | Vite (`vite_rails`) | **Vite** | Standard JS build for the webview. |
| State | Inertia props | **Zustand** (or React context) | Small renderer store (D-P5). |
| Schema validation | `json_schemer` (Ruby) | **`jsonschema` crate (Rust)** | Same schema, core-side + CLI (D-P7). |
| YAML | Psych (Ruby stdlib) | **`serde_yaml_ng` / `serde_norway`** | `serde_yaml` is archived; pick a maintained fork. |
| Code parsing | — | **`tree-sitter` + grammar crates** | Multi-language symbol extraction. |
| Repo walk | — | **`ignore` / `walkdir`** | Fast, `.gitignore`-respecting scan. |
| File watch | (roadmap) | **`notify`** | Live refresh of model + sources. |
| Open files | — | **`tauri-plugin-opener` / `-shell`** | Jump-to-code into `$EDITOR`. |
| Dialogs/FS | — | **`tauri-plugin-dialog` / `-fs`** | Folder picker, save-export. |
| Updates | — | **`tauri-plugin-updater`** (signed) | Cross-platform auto-update. |
| Styling | Tailwind (optional) | **Tailwind (optional)** | Works for the renderer without adding a design-system dependency. |
| Tests | Minitest | **`cargo test` (core) + Vitest (renderer)** | Per-layer testing. |
| Auth | None on localhost | **None (local single-user)** | Nothing to authenticate. |

**Deferred renderer dependency — Effect.** `effect` / Effect-TS was reviewed
as a possible TypeScript runtime for typed async workflows, cancellation,
retries, service composition, and richer client-side error modeling. It is not
part of Phases 0-1 because the app's reliability-critical work is intentionally
in Rust: filesystem safety, parsing, validation, scanning, indexing,
generation, watching, writer locks, and CLI behavior. The renderer should stay
plain React/TypeScript while it mainly owns view derivation, ELK layout,
xyflow interaction, and thin Tauri command calls. Reconsider Effect in Phase 2
only if the generation review UI grows enough client-side orchestration
complexity — progress streams, cancellation, retries, concurrent command state,
or a real renderer service layer — to justify adopting a second runtime model.

---

## 9. Workflows

### AI Agent Authoring

The file-based flow is simple: the agent reads `c4/schema.json`, writes/updates `c4/model.yml`, runs `c4lens validate`, commits; the app picks up the change via the watcher. The agent (or the user) can run `c4lens generate` to scaffold `c4/model.generated.yml` from the codebase first, then hand-curate the authored `model.yml` on top — turning "write the model from scratch" into "review and refine a generated draft."

### Generation workflow (new)

1. Open a repo (folder picker) → the core scans it (`scan_codebase`) and builds the SQLite index.
2. **Generate** → `generate_model` returns a reviewable candidate diff for `model.generated.yml` without writing it.
3. **Review** the diff in-app (accept all / per-element).
4. **Apply** → `apply_generated` writes the accepted overlay to disk; the effective model updates live.
5. The human/agent refines `model.yml`; authored content always wins the merge.

### Human Authoring

Edit `c4/model.yml` with the schema wired into the editor for autocomplete and inline validation.

### CI Gate

`c4lens validate` runs in CI and exits non-zero on an invalid model. `c4lens generate --check` detects generation drift from the committed overlay.

---

## 10. Phased delivery

**MVP = Phases 0–2.** Generation lands in MVP (Phase 2) because the user prioritized code-derived modeling; it depends on the index built in Phase 1.

- **Phase 0 — Skeleton.** Tauri 2 + React + Vite + xyflow; open a folder; render one hard-coded sample model in the navigator to prove the render path inside the webview. Verify canvas behavior in WKWebView/WebView2.
- **Phase 1 — Core + index.** YAML parse + `jsonschema` validation in the Rust core; `ViewDeriver` (relocated to the renderer) + the `Navigator/Show` view with drill-down, breadcrumb, detail panel, deep links; `notify` watch + live refresh; **SQLite code index** (files/symbols/imports) via tree-sitter + `ignore`; **jump-to-code** (open in editor + inline preview); `c4lens validate` CLI.
- **Phase 2 — Generation + polish.** **Code→model generation** to `model.generated.yml` with overlay merge, provenance badges, and the review-diff UX (`c4lens generate`); dependency highlighting + focus mode; search/jump-to; SVG/PNG export via save dialog; light/dark; layout caching; packaging (`.dmg`/`.msi`/`.AppImage`) + signed auto-update.
- **Phase 3+ — Roadmap.** **Rendered L4 Code-level views** (xyflow + ELK over indexed symbols — cheap, since the data exists); LSP-backed relationship inference (precise call/usage edges); generated-slug rename/move detection (preserve slugs when files move); multi-repo workspace; **local MCP server** the app exposes to agents; curated named views; multi-file models; tags/perspectives; flows; Mermaid/Structurizr export; `c4://` deep links.

---

## 11. Risks & open questions

Core risks: auto-layout aesthetics at scale, large-model rendering, slug stability vs renames, file/cache drift (always rebuild from source), client-side ELK cost, and desktop packaging. Desktop-specific:

- **Cross-platform webview parity (Tauri).** The canvas renders well in WKWebView/WebView2 but Linux WebKitGTK is the weak spot. *Mitigation:* test the canvas early (Phase 0) on each target; the renderer is portable to Electron (D-P1) if Linux parity becomes blocking.
- **Generation accuracy & non-destructive merge (the big one).** Heuristic generation will be imperfect, and the overlay merge must never clobber authored content. *Mitigation:* overlay file + provenance + review-diff before write (D-P3); treat the round-trip as the headline risk (IcePanel documents exactly this hazard).
- **Relationship inference quality.** Cheap signals (intra-repo imports, manifest deps) miss runtime/dynamic edges and over/under-connect. *Mitigation:* flag generated relationships, keep them in the overlay, defer precise inference to LSP (Phase 3).
- **Index staleness & incremental rescan.** Keeping the SQLite index correct as files change. *Mitigation:* content hashes drive change detection, mtime/size are advisory metadata only, `notify` drives incremental updates, and full rescan is always available.
- **Large-repo scan performance.** First scan of a big monorepo. *Mitigation:* `ignore`-based parallel walk, tree-sitter is fast, scan off the UI thread with `scan-progress`; cache so it's a one-time cost.
- **tree-sitter grammar coverage.** One grammar per language; uncovered languages degrade to file/dir-level generation only. *Mitigation:* ship a core set (JS/TS, Python, Rust, Go, Java, Ruby), fall back to directory heuristics otherwise; document coverage.
- **Code signing / notarization.** macOS notarization and Windows signing are required for friction-free install/update. *Mitigation:* budget signing in Phase 2 packaging; `tauri-plugin-updater` expects signed artifacts.
- **Security of reading arbitrary repos.** The app reads (and parses) whatever folder it's pointed at. *Mitigation:* strictly local, no network egress; Tauri capability allowlist scoped to the chosen repo; canonicalize all repo paths; reject symlink/control-file escapes; never execute scanned code.
- **Generated-slug stability (decided).** Path-derived slugs change when files move, surfacing as dangling refs in authored relationships. MVP relies on validator flagging; rename/move detection is deferred to post-MVP (§10).
- **Open — overlay merge semantics at the edges.** Element suppression (tombstones), per-field overrides vs whole-element replacement, and merging generated *relationships* against authored ones need exercising on a real, messy repo early in Phase 2.

---

## 12. Appendix

### A. Generation heuristics (MVP)

Best-effort signals → C4 elements; everything generated is provenance-marked and lands in the overlay for review.

| Source signal | Detects | → c4lens |
|---|---|---|
| `package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`/`requirements.txt`, `Gemfile`, `pom.xml`/`build.gradle` | a buildable/runnable unit | **container** (tech from the manifest) |
| `Dockerfile`, `docker-compose` service | a deployable unit | **container** (`kind` inferred from base image) |
| top-level source dirs / packages / modules within a container | building blocks | **component**s |
| internal imports between modules (tree-sitter) | intra-system coupling | **relationship**s (component/container level) |
| declared external deps / SDK clients (`aws-sdk`, `stripe`, `pg`, `redis`, …) | external systems/stores | **external system** + relationship (flagged) |
| data-store drivers (`pg`, `mysql`, `redis`, `mongo`, …) | persistence | container `kind: store` |

Relationships are surfaced once at the level they're detected; the renderer's `ViewDeriver` aggregates them to higher boundaries, so generation doesn't need to emit redundant edges.

### B. SQLite code-index schema (representative)

```sql
repos(id, root_path, vcs, head_sha, scanned_at)

files(id, repo_id -> repos, path, lang, content_sha, mtime, size,
      UNIQUE(repo_id, path))                       -- content_sha drives incremental rescan

symbols(id, file_id -> files, kind, name, qualified_name,
        start_line, end_line, parent_symbol_id -> symbols)   -- tree-sitter output; powers L4 (roadmap) + jump-to-code

imports(id, file_id -> files, target_module, target_path,
        resolved_file_id -> files, kind)           -- raw material for relationship inference

element_sources(element_slug, repo_id -> repos,
                file_id -> files, symbol_id -> symbols, path_glob)  -- maps C4 elements -> code

model_cache(repo_id -> repos, source_sha, derived_json)  -- optional: skip re-parse on launch; rebuilt on source_sha change
```

Keyed by `content_sha`, with `mtime` and size retained as advisory metadata. Everything here is derivable from the codebase + model files; SQLite is a cache (D-P4).

### C. Example generated overlay

```yaml
# c4/model.generated.yml  — regenerated by `c4lens generate`; do not hand-edit (edit model.yml instead)
# yaml-language-server: $schema=./schema.json
name: Internet Banking            # overridden by model.yml if present
systems:
  internet_banking:
    name: internet_banking
    generated: true
    containers:
      api:
        name: api
        tech: Ruby on Rails        # from Gemfile + config
        code: app/
        generated: true
        components:
          accounts: { name: accounts, code: app/controllers/accounts_controller.rb, generated: true }
relationships:
  - { from: api, to: db, description: "imports pg", generated: true }   # inferred; flagged for review
```

The user/agent keeps the authored `c4/model.yml`; the merge (D-P3) lets authored elements override any generated element of the same slug.

### D. Spec Cross-References

- **JSON Schema and model examples**: [`c4lens-desktop-spec.md`](../spec/c4lens-desktop-spec.md) §§5-6.
- **Validation, merge, path, and YAML rules**: [`c4lens-desktop-spec.md`](../spec/c4lens-desktop-spec.md) §§3, 8-9.
- **`ViewDeriver` boundary aggregation**: [`c4lens-desktop-spec.md`](../spec/c4lens-desktop-spec.md) §10.
- **IPC, scanner, generation, watcher, and CLI contracts**: [`c4lens-desktop-spec.md`](../spec/c4lens-desktop-spec.md) §§12-17.
- **Glossary additions**: **Overlay** — the generated `model.generated.yml`, merged under the authored model; **Code index** — the SQLite projection of the scanned codebase; **Provenance** — the `generated: true` marker distinguishing derived from authored content.
