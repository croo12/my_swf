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
│   │   ├── widgets/             # workbench-header, plan-canvas, command-bar
│   │   ├── features/            # edit-graph, record-request, connect-server, server-sync, plan-transfer, plan-job
│   │   ├── entities/plan/       # Plan schema v0.3, layout and validation, plan/selection stores, node and edge UI
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

1. `⌕ 저장소 분석` — read actual `src/app/pages/widgets/features/entities/shared` directory layout and resolvable imports. Existing canvas replaced **only with explicit confirmation**. Static analysis does not invent I/O contracts.
2. Edit the graph, input/output contracts, and dependencies directly on the canvas.
3. Write a requirement in the command box and choose `✦ Herdr 계획 요청`. The server creates a detached Git worktree at `data/jobs/<id>/worktree`, starts Herdr/Codex, and asks it to create `.factory/proposed-plan.json`.
4. Wait for a completed job. The UI polls for status and shows `제안 그래프 적용`. **No AI output automatically overwrites your graph**. You must explicitly accept it.
5. `서버 저장` → `계획 승인` creates an immutable timestamped snapshot under `data/approved/`.

The Herdr pane is retained for inspection. If the agent is blocked, the job may fail or time out; use Herdr TUI to inspect the pane and address the prompt. A failed/missing JSON output is not treated as successful execution.

**Important**: The AI job runs from `HEAD` in a detached worktree; uncommitted changes in the original repository are not copied. The static scanner reads the working tree. This difference is intentional and documented. `data/jobs/*/worktree` is not automatically removed. Inspect and clean up old worktrees with `git worktree list` / `git worktree remove` after jobs are no longer needed.

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
- The static scanner is intentionally conservative: import aliases beyond `@/`, package exports, barrel re-exports, and TypeScript type resolution need a more complete analyzer.
- Planning's AI contract is JSON + schema validation, not a guarantee of architectural quality. Current AI revisions replace the entire graph *only after human approval*, not yet a minimal patch/diff merge.
- Herdr job status is persisted, but ongoing processes are **not automatically resumed after a Factory server restart**; interrupted jobs should be inspected in Herdr.
- No rich multiuser edit conflict management, automatic worktree cleanup, push/PR integration, or delivery stage yet.

See `docs/ARCHITECTURE.md` for the stage contract and API endpoints.
