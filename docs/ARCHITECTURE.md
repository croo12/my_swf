# Native workbench

이 단계에서 설계의 작성자는 사용자입니다. 앱은 실제 저장소를 보여주고 사용자가 작성한 계획을 보존·내보냅니다.

```text
Rust native desktop (Windows / macOS)
  Repository: Layer → Slice → index group → nested groups / files
       │ actual files and import names
       ▼
  Human-authored Plan → JSON (re-edit) / Markdown (AI handoff)
       │
       └─ local draft autosave (no credentials)

Repository data sources
  local folder ─────────── factory-core::repository::analyze
  HTTPS Factory server ── GET /api/repository/tree ── same core
```

## Ownership

`factory-core` owns the existing FSD scanner, directory hierarchy, plan data and exports. It has no UI, HTTP runtime, WebView or platform credential dependency. `factory-layout` owns scene geometry, selection boundaries, hierarchy arrangement and routing. It uses `emath` for coordinates and has no dependency on eframe or the desktop event loop. `factory-workbench` owns window state, UI modules, painting and the comparison CLI. `factory-server` reuses the core, retaining its previous API for compatibility.

Every Rust file under `apps/native` is limited to 400 physical lines, counting comments and blank lines. Its Cargo build script checks the complete directory recursively, including build scripts, tests and unreferenced new modules; generated `target` output is excluded. Watching the crate directory also catches newly added files. CI runs the same guard in a separate job before any router setup and repeats it through normal native builds. UI responsibilities are split into `app/`, `canvas/` and `repository_graph/`; reusable behavior belongs in shared crates. The retired Tauri source is available in Git history; app icons live in `apps/native/assets/icons`.

The source hierarchy is recursive and preserves folders without an index. A directory with a recognized `index.*` is marked as a public API group; slices follow the existing scanner's discovered boundaries, including grouping folders above a slice. Direct FSD layers (`app`, `shared`) have no slice level. A group shows its real children, index path and recursive file count. File import records identify the dependencies that cross the selected directory's boundary. This is structural information rather than an AI description of responsibilities.

The repository screen is a compound dependency graph. Expanding a layer, slice or group turns its node into a container enclosing its children; other layers and siblings remain in the same graph. Each observed import is projected onto its deepest visible endpoints. Collapsing a group aggregates its external imports at its boundary, and hides internal imports until it is expanded again. Counts represent unique importing/provider file pairs, not the number of imported symbols. Search reveals the required ancestor chain without collapsing unrelated groups.

The layout stacks FSD layers vertically. Each layer's immediate children occupy one horizontal row without wrapping, as do siblings inside a nested group. Selecting a card packs it and its directly connected cards first, docks unrelated cards to the right and dims them while retaining the complete hierarchy. Layer positions and expansion state remain intact; clearing selection restores the original ordering and opacity. A card click selects incoming/outgoing relationships; disclosure buttons expand or collapse it. Layer headers only expand/collapse, never select connections. Collapsing an ancestor focuses that ancestor, except a collapsed layer clears the selection. The canvas enables mouse dragging on both scroll axes, in addition to wheel and scrollbar navigation.

Ctrl+wheel zoom is scoped to the repository graph viewport (Cmd+wheel also works on macOS). It uses the same bounded scale as the toolbar slider and adjusts the scroll offset so the world point under the pointer stays fixed, except when constrained by a canvas boundary. Ordinary wheel scrolling and inspector scrolling do not alter graph scale. Zoom only changes painting and viewport offset; it does not trigger layout or native routing again.

The scene first arranges all cards according to the one-row hierarchy and focus rules. It then routes only selected dependencies with the native Libavoid backend used by ELK's fixed-node routing integration. Card coordinates and sizes are input constraints, not output of ELK Layered. Leaf bodies and expanded-group headers become obstacles. Distinct explicit ports connect each route. Endpoint nudging is disabled to preserve those pins. All projected facts remain present. Empty/layer selection skips the native router entirely, and expanded-group selection only shows dependencies crossing its boundary.

The bundled elk-rs 0.12.2 Libavoid provider implements metadata but not routing, so `elk_routing.rs` directly speaks the upstream TypeFox/libavoid-server v0.3.1 stdin/stdout protocol instead of treating the no-op as a successful layout. The native helper is launched hidden on Windows, receives numeric geometry without source paths, and exits at the end of the request. Output is drained concurrently, bounded in size, validated for complete IDs, finite orthogonal paths and fixed endpoints, and subject to a 15-second deadline. There is no silent fallback to the previous router.

Windows distribution is a single executable; macOS distribution is a single app bundle. `factory-layout/build.rs` embeds the prepared router, notices, licenses, complete corresponding upstream source archives and preparation scripts. The payload's SHA256 determines its cache subdirectory. `embedded_router.rs` prepares these resources automatically on first use in the user's local cache using unique temporary files and atomic replacement; concurrent instances can share it. Subsequent routing reuses the cache and compares the executable against the embedded bytes, repairing deletion or corruption. No neighboring executable, build checkout, runtime download or manual extraction is required. The process remains a short-lived native helper, not an in-process library. `FACTORY_LIBAVOID_SERVER` still supports an independently modified replacement. `--export-router-sources <directory>` exposes the bundled source/licenses, and `--check-router` enables single-file distribution smoke checks. macOS prepares a signed universal router before embedding it.

ELK Layered remains available in the comparison CLI through `elk-layout` 0.12.2 (openedges/elk-rs). `INCLUDE_CHILDREN` lays out the complete visible hierarchy with cross-boundary dependencies. Its shape and edge coordinates explicitly use root coordinates, and the adapter retains every projected node/edge and joins all edge sections. This free layout is a separate comparison from the fixed-card interactive view.

Layout requests use one worker thread and coalesce queued expansion/focus requests. Generation IDs discard stale results. Layout failures are shown explicitly. The old visibility-grid router remains in the comparison CLI; the interactive graph uses the native Libavoid routes directly. Neither engine promises a crossing-free drawing. The comparison CLI deliberately uses the complete graph for both engines, independently of the interactive selection filter, and exports input/output JSON, before/after SVG and measured crossings, shared segments, node intersections, containment and layout time.

Rendering adds a dark gap at crossings so independent edges do not look like junctions. Outgoing relationships use purple and incoming relationships use teal, including selected expanded groups. Edge labels and hover details follow the same selection rule. Clearing selection hides all lines. This display filter does not mutate the repository graph or the plan.

The scanner accepts JavaScript / TypeScript source and the browser also lists common UI assets. Hidden/build/test fixture folders and symlinks are skipped. The hierarchy is limited to 32 levels and 50,000 entries; the existing import scanner has its own depth limits. Source roots outside the selected repository are rejected by the hierarchy API.

The scan includes separate `fileReferences` for direct file imports and re-exports, including slice/root index files. Repository projection and the inspector use these facts so an index card connects both to its importers and to the files it re-exports. The original `records` retain resolved providers for plan generation and interface seeding. Older server snapshots without `fileReferences` fall back to `records`. The inspector's public-entry link reveals and selects the actual index file.

## Plan files

JSON retains the previous `schemaVersion: "0.3"`, `version`, `summary`, `nodes`, `edges` and `requests`. Nodes retain id/name/kind/path/description/files/interfaces; edges retain id/from/to/type/uses. Extra fields are retained during JSON migration. The importer also accepts a `{ plan: ... }` wrapper. Unknown schema payloads without nodes and edges are rejected, as are duplicate IDs, dangling relations and unsafe relative paths.

Files are relative to each module's placement. Existing groups are copied into the plan with their observed files and imported interface names. Responsibilities, behavior, input/output types and any new interfaces are authored by the user. Static shape/path checks do not claim architectural correctness. The old server's stricter AI proposal checks do not gate local manual editing.

JSON is the full editable record. Markdown includes the summary, modules, responsibilities, files, interface contracts, relation kinds/used names, and recorded request text. Exported Markdown is a handoff document; exporting never calls an AI service.

## HTTP

| Method | Route | Behavior |
| --- | --- | --- |
| GET | `/api/repository/tree` | Bearer-authenticated recursive directory tree plus observed import scan; does not pull, save or execute jobs |

Response: `{ repository, sourceRoot, tree, scan, interfaceKinds }`. Each tree entry contains `{ name, path, kind, index, fileCount, children }`. Paths are relative to the FSD source root with `/` separators. `interfaceKinds` uses declaration patterns from the existing scanner; interface behavior and input/output types remain empty for the user to write. The native client supports HTTPS, with loopback HTTP for local use, and does not follow redirects with credentials.

The existing `/api/plan`, `/api/repository/analyze`, `/api/repository/modules`, `/api/plan/check`, `/api/jobs` and approval routes remain available to the old client. Their behavior is documented in [the legacy planning contract](LEGACY-PLANNING.md); the native app does not call AI planning jobs.

## Runtime and platform scope

The native UI has headless regression tests under `apps/native/src/app/tests`, using `egui_kittest` 0.37 with its eframe integration. The harness constructs the real `Workbench`, drives its accessible widgets and inspects plan state and scene geometry without creating an OS window or capturing the desktop. It retains accessibility-name and missing-glyph checks and installs the same system-font fallback used by native eframe. No snapshot renderer or GPU is required for these interaction tests.

`FileAccess` injects the session path and a `FileDialogs` implementation. Production uses the original OS locations and rfd dialogs; each test supplies an isolated temporary profile and scripted selections. Tests do not change process environment variables. Repository tests scan a small on-disk FSD fixture and exercise the actual background layout worker and embedded router. Readiness checks account for pending requests, expansion revisions and selection changes; they wait with a deadline rather than relying on a fixed number of frames for asynchronous completion. App editing and autosave timing remain unchanged. These tests do not claim OS-window, system-dialog, keychain or GPU-renderer coverage.

The native client uses egui/eframe with the Glow renderer. No WebView is created. Directory scanning, network requests and native file dialogs run on worker threads and wake the UI on completion. Repaints are event-driven, except while a progress indicator or delayed autosave needs a frame. No permanent polling timer is used. Resource usage still requires measurements on representative repositories and each target platform.

Windows and macOS build checks are configured in CI, including routing from an executable copied alone into an isolated folder. Windows release packaging publishes one `.exe`; macOS packaging combines Intel and Apple Silicon binaries in an ad-hoc signed `.app`. Developer ID signing, notarization and Android packaging are outside this change. The previous Tauri client has been removed; native Android packaging is not yet implemented.
