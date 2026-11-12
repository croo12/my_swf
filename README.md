# Software Factory · Planning Workbench

**v0.1 multi-platform source project** — Tauri 2 client (macOS / Windows / Android), Rust + Axum server (Mac Studio), and a real Herdr TUI / Codex planning adapter.

This is a working *source starter*, not a prebuilt signed `.app`, `.msi`, or `.apk`. Build on the corresponding OS (Android builds additionally need Android SDK/NDK). The JavaScript syntax, package/config files, and mocked desktop/mobile UI interactions have been checked. Rust/Android compilation and live Herdr integration have **not** been verified in the artifact-generation environment.

## Layout

```text
software-factory-multiplatform/
├── apps/client/                 # One shared Planning Workbench UI (React + TypeScript, React Compiler)
│   ├── src/                     # Feature-Sliced Design layers; imports only point downward
│   │   ├── app/                 # Entry point and global styles
│   │   ├── pages/workbench/     # The single screen, composed from widgets
│   │   ├── widgets/             # workbench-header, repository-overview, plan-canvas, command-bar
│   │   ├── features/            # edit-graph, record-request, analyze-repository, add-slice-to-plan, switch-view, connect-server, server-sync, plan-transfer, plan-job
│   │   ├── entities/            # plan (schema, ELK layout, validation, stores, module/edge UI), repository (slice-level scan)
│   │   └── shared/              # api (Factory transport), lib (store), ui (toast)
│   ├── package.json             # Tauri / Vite commands, plus `npm run check` (tsc, oxlint, steiger, oxfmt)
│   └── src-tauri/               # Native Rust client for macOS/Windows/Android
├── crates/factory-server/
│   └── src/
│       ├── main.rs              # API, persistence, Herdr job execution
│       └── graph.rs             # Filesystem FSD structure/import scanner
├── scripts/macos/              # launchd helper
└── docs/                        # Architecture and API
```

## A. Start the app on your MacBook (UI works without a server)

Prerequisites: Node 20+, Rust (rustup), Xcode Command Line Tools and macOS Tauri system dependencies. Follow [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your machine.

```sh
unzip software-factory-multiplatform.zip
cd software-factory-multiplatform/apps/client
npm install
npm run desktop
```

The **Software Factory** window opens. It contains the existing v0.3 FSD-layer graph, node/I-O editor, and JSON import/export. You can use these locally while the Mac Studio server is offline.

Build a standalone macOS application:

```sh
npm run desktop:build
# or: npm run tauri -- build --bundles app
```

## B. Start the server on Mac Studio

Copy/clone the same project on Mac Studio. Install Rust, Git, Herdr TUI and Codex CLI. **Log Codex into the existing ChatGPT subscription as the same macOS user that runs Herdr.**

Use your actual work repository path (must be a Git repository for AI jobs). On Mac Studio, in **Terminal 1** start the Herdr runtime:

```sh
herdr server
```

In **Terminal 2**, configure and start the Factory API:

```sh
cd software-factory-multiplatform
export FACTORY_REPO_PATH="$HOME/projects/my-react-project"
export FACTORY_DATA_DIR="$PWD/data"
export FACTORY_API_TOKEN="$(openssl rand -hex 32)"
export FACTORY_BIND="127.0.0.1:8787"
echo "Save this API token securely: $FACTORY_API_TOKEN"
cargo run -p factory-server --release
```

**Keep the token**; enter exactly the same value in the Tauri app's `⚙ 서버 연결` dialog. Never commit the token. For a persistent setup, see the optional launchd helper below.

Herdr CLI is controlled by the server (`workspace create`, `agent start`, `agent prompt --wait`). If `herdr` isn't on the launchd PATH, set `HERDR_BIN` to its **absolute path**. The server and `herdr server` must run under the same user/session. Check `herdr status server` before running AI jobs.

### Secure networking without manually opening an SSH tunnel

On Mac Studio, if both machines are on the same tailnet, run:

```sh
tailscale serve --bg 8787
tailscale serve status
```

The command displays an HTTPS hostname like `https://mac-studio.example.ts.net`. This is **Tailscale Serve, not Funnel**. On MacBook, open `⚙ 서버 연결`, enter that HTTPS hostname and `FACTORY_API_TOKEN`, then click **연결 확인**. If Serve prompts you to enable HTTPS certificates on the tailnet, follow that setup.

The server binds only to loopback by default, requires a bearer token for every API route and can be accessed over Tailscale Serve. The desktop app keeps the token in the OS credential store (Windows Credential Manager / macOS Keychain), so you enter it once; the URL is saved locally. In a plain browser or on Android the token lasts only for the session.

## C. Using Planning

The app has two views, switched in the header.

**저장소 (repository)** is a read-only overview of what exists. `서버 ▾ → 저장소 분석` makes the server read the real FSD layout and the imports it can resolve, and shows them at slice level. Select a slice to see what it uses and what uses it. Nothing can be edited here, and analysis never changes the plan.

**계획 (plan)** is where work is designed. A plan starts empty: it holds the modules to build or change, and where each one is placed.

1. Write what you want built in the request box and press `✦ 계획 요청`. The server runs Codex in a Herdr workspace, read-only, and asks for the modules: responsibility, behaviors, its interfaces (what it offers to other modules and the requests it sends to the server, each with a kind and with inputs and outputs where it has any), placement (layer / slice / segment), files, and dependencies.
2. The answer is constrained to a JSON Schema and then checked by the server against fixed rules and against the repository (see `docs/ARCHITECTURE.md`). **A proposal that breaks any rule is rejected as a whole**, and the job reports every violation. Nothing is repaired or guessed.
3. A proposal that passes shows `제안 그래프 적용`. **No AI output changes your plan by itself**; you accept it explicitly, and you can undo it.
4. Edit modules on the canvas: click to edit, drag into another layer to change the placement. `＋ 모듈` adds a module by hand. The graph is laid out automatically (ELK): only the layers the plan touches are shown, relations are drawn as orthogonal lines, and selecting a module highlights its relations. Requests to the server appear as a server box below the layers, reached from the `shared/api` modules.
5. `서버 ▾ → 서버에 저장` then `계획 승인` creates an immutable timestamped snapshot under `data/approved/`.

A request is addressed to the selected module or to the whole plan; the target is shown next to the request box.

Codex runs with a read-only sandbox in the repository's working tree, so it sees uncommitted changes and cannot modify files. Only client code inside the FSD layers can be planned; the answer schema has no way to express other code. The Herdr workspace of each job is kept for inspection.

## D. Windows client

Copy/clone the same project to Windows, install Rust / Visual Studio C++ Build Tools / WebView2 and Node according to [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/):

```powershell
cd software-factory-multiplatform\apps\client
npm install
npm run desktop          # development
npm run desktop:build    # Windows installer from Windows host
```

Connect the Windows Tauri app to the **same Mac Studio** over the Windows Tailscale client. You do not need Herdr installed on Windows.

## E. Android client

Install Android Studio, Java, Android SDK/NDK and Rust Android targets using [Tauri's Android instructions](https://v2.tauri.app/start/prerequisites/). From a configured macOS / Windows / Linux development machine:

```sh
cd software-factory-multiplatform/apps/client
npm install
npm run android:init     # generates native Android project, once
npm run android:dev      # emulator or connected Android device
npm run android:build    # APK; signing required for distribution
```

Install and connect **Tailscale for Android** on the phone. Enter the Mac Studio's Tailscale Serve **HTTPS** URL in the app. The same graph UI is horizontally scrollable on smaller screens; a tablet/landscape orientation is more comfortable. Android acts as a remote **client**: Rust Factory and Herdr execute on Mac Studio, never on the phone.

## F. Mac Studio server from releases, with automatic updates (optional)

Instead of building on Mac Studio, run the server binary published with each GitHub Release. Mac Studio checks for a new release every 5 minutes and switches to it, so **pushing a release tag deploys the server**. Mac Studio pulls the release; nothing connects in from GitHub. Rust and a clone of this repository are not needed.

One-time setup on Mac Studio (Apple Silicon):

```sh
mkdir -p ~/.config/software-factory
curl -fsSL https://raw.githubusercontent.com/croo12/my_swf/main/scripts/macos/server.env.example -o ~/.config/software-factory/server.env
# Edit server.env: set FACTORY_REPO_PATH, FACTORY_DATA_DIR, and a FACTORY_API_TOKEN from `openssl rand -hex 32`.

curl -fsSL https://raw.githubusercontent.com/croo12/my_swf/main/scripts/macos/release-server.sh -o /tmp/release-server.sh
bash /tmp/release-server.sh install
```

This installs the latest release to `~/.local/share/software-factory-server/` and two user LaunchAgents: the server itself, and the update check. Settings stay in `server.env` and are never touched by updates.

- An update waits while a planning job is running, because restarting the server orphans the job.
- The previous binary is kept as `factory-server.previous` next to the current one.
- Logs are in `~/Library/Logs/SoftwareFactory/`.
- `bash ~/.local/share/software-factory-server/bin/release-server.sh update` checks immediately; `uninstall` removes both LaunchAgents.

**Herdr still needs its own supervised `herdr server` process** under the same user for automated plans. Start that separately or manage it with your own launchd service; don't run two copies of the same Herdr session.

To build from source on Mac Studio instead, use `./scripts/macos/install-server-launchd.sh` from a clone. Both scripts register the same server LaunchAgent, so use one or the other.

## Current limits

- No verified production build / code signing in this delivery environment.
- One selected repository per server instance; not a general project registry yet.
- The static scanner works on text patterns, not a TypeScript AST. It finds the FSD source root inside the repository (set `FACTORY_SOURCE_ROOT`, relative to the repository, to choose it yourself), treats the first folder below a layer that has an index file as the slice, and resolves relative imports, `@/`, and layer aliases such as `@features/...`. Imports of a slice's public API are traced through its index re-exports to a segment; names it cannot trace are counted in `analysis.unresolvedImports` and produce no edge. Other `tsconfig` aliases, package exports, and type resolution are not handled.
- Validation guarantees the shape of a proposal and its consistency with the repository, not its architectural quality. An accepted proposal replaces the whole plan *only after human approval*; there is no minimal patch/diff merge yet. A rejected proposal is not retried automatically.
- Job status is persisted, but running jobs are **not resumed after a Factory server restart**; inspect interrupted jobs in Herdr. Old Herdr workspaces are not closed automatically.
- No rich multiuser edit conflict management, push/PR integration, or delivery stage yet.

See `docs/ARCHITECTURE.md` for the stage contract and API endpoints.
