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

`factory-core` owns the existing FSD scanner, directory hierarchy, plan data and exports. It has no UI, HTTP runtime, WebView or platform credential dependency. `factory-workbench` owns window state and drawing. `factory-server` reuses the core, retaining its previous API for the previous client.

The source hierarchy is recursive and preserves folders without an index. A directory with a recognized `index.*` is marked as a public API group; slices follow the existing scanner's discovered boundaries, including grouping folders above a slice. Direct FSD layers (`app`, `shared`) have no slice level. A group shows its real children, index path and recursive file count. File import records identify the dependencies that cross the selected directory's boundary. This is structural information rather than an AI description of responsibilities.

The repository screen is a compound dependency graph. Expanding a layer, slice or group turns its node into a container enclosing its children; other layers and siblings remain in the same graph. Each observed import is projected onto its deepest visible endpoints. Collapsing a group aggregates its external imports at its boundary, and hides internal imports until it is expanded again. Counts represent unique importing/provider file pairs, not the number of imported symbols. Search reveals the required ancestor chain without collapsing unrelated groups.

The native scene caches this projection and its layout until the repository or expansion state changes. An orthogonal visibility-grid router keeps import lines outside node bodies and expanded group titles. Group boundaries, selection highlighting, zoom and scroll preserve the graph context while inspecting nested structures.

The scanner accepts JavaScript / TypeScript source and the browser also lists common UI assets. Hidden/build/test fixture folders and symlinks are skipped. The hierarchy is limited to 32 levels and 50,000 entries; the existing import scanner has its own depth limits. Source roots outside the selected repository are rejected by the hierarchy API.

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

The native client uses egui/eframe with the Glow renderer. No WebView is created. Directory scanning, network requests and native file dialogs run on worker threads and wake the UI on completion. Repaints are event-driven, except while a progress indicator or delayed autosave needs a frame. No permanent polling timer is used. Resource usage still requires measurements on representative repositories and each target platform.

Windows and macOS build checks are configured in CI. Windows release packaging yields a portable ZIP; macOS packaging combines Intel and Apple Silicon binaries in an ad-hoc signed `.app`. Developer ID signing, notarization and Android packaging are outside this change. The retained Tauri Android build is a legacy client, not the new native UI.
