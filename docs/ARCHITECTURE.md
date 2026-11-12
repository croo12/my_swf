# Planning stage contract

```text
macOS / Windows / Android            Mac Studio
┌───────────────────────────┐        ┌──────────────────────────┐
│ Tauri 2 Planning Canvas   │        │ Factory Server (Rust)    │
│ - FSD layer/slice/segment │ <----> │ - bearer-token API       │
│ - editable I/O            │ HTTPS  │ - Plan save and approval │
│ - explicit AI acceptance  │ Serve  │ - FSD scanner            │
└───────────────────────────┘        │ - Herdr adapter + jobs   │
                                     └───────────┬──────────────┘
                                                 │ local Herdr CLI
                                          ┌──────▼───────────┐
                                          │ Herdr TUI server │
                                          │   Codex agent    │
                                          │ Git worktree     │
                                          └──────────────────┘
```

## HTTP API (all Bearer-authenticated)

| Method | URL | Payload / Result |
| --- | --- | --- |
| GET | `/api/status` | Stage info / current repository |
| GET | `/api/plan` | `{ plan }` saved draft |
| PUT | `/api/plan` | Plan JSON, response `{ saved:true }` |
| POST | `/api/approve` | `{ approved:true, file }` snapshot path |
| POST | `/api/repository/analyze` | `{ plan }` verified directory/import graph |
| POST | `/api/jobs` | `{ instruction, currentPlan }` -> `{ id,status }` |
| GET | `/api/jobs/{id}` | `{ job, proposedPlan }` (`proposedPlan` on success) |

## Plan document

A plan holds the modules to build or change and where each is placed. `nodes[]`: id, name, kind (`new` | `modify` | `existing`), path (layer/slice/segment, or layer/segment in app and shared), description (responsibility), behaviors, `interfaces[]` (name, description, inputs, outputs), `serverCalls[]` (method, path, description), files. `edges[]`: id, from, to, type. Also `requests[]`, `version`, and an optional `summary`. Positions are not stored; the client lays the graph out. Before saving, the server checks unique node ids, FSD paths, interface names and array types, and edge endpoints.

## Planning job: structured answer, static validation

The AI is not trusted to follow instructions about shape or rules.

1. **Schema.** Codex runs as `codex exec --output-schema`, so it can only answer in the shape of `planner::output_schema()`: `summary`, `modules[]` (id, name, kind, layer, slice, segment, responsibility, behaviors, `interfaces[]` with name/description/inputs/outputs, `serverCalls[]` with method/path/description, files) and `dependencies[]` (from, to). `layer` is an enum of the six FSD layers, so code outside them cannot be expressed.
2. **Typed parse.** The answer is deserialized into Rust types that reject unknown fields and unknown enum values.
3. **Static rules** (`planner::validate`), checked against the repository scan and the disk:
   - Every module has an id (unique), a name, a responsibility, and at least one file.
   - Sliced layers need a slice; app and shared must not have one. Slice, segment and file names are clean relative paths.
   - `modify` and `existing` modules are at a placement the scanner found, and every file they name exists there. `new` modules name files that do not exist yet. Only client source file types are accepted.
   - `new` and `modify` modules state at least one behavior and expose at least one interface. An interface needs a unique name; its inputs and outputs may be empty.
   - A module that sends requests to the server is placed in `shared/api` or reaches a `shared/api` module through its dependencies. Server call paths start with `/`; the method is one of GET, POST, PUT, PATCH, DELETE.
   - Dependencies refer to modules in the plan, are not duplicated, do not start at an `existing` module, and form no cycle. A dependency points to a lower layer or stays inside one slice.
   - Every `existing` module is depended on by another module.
4. **Rejection.** Any violation fails the job with the full list (also written to `rejected.json`). Only a proposal with no violations becomes `proposed-plan.json`.

Each job keeps its files in `data/jobs/<uuid>/`: `prompt.txt`, `schema.json`, `result.json` (raw answer), `exit.json`, and `proposed-plan.json` or `rejected.json`.

## Trust boundary

- Rust verifies machine-readable graph contracts. AI generates **proposals**, never direct writes to a persisted approved Plan.
- Herdr executes exclusively on Mac Studio (even when users operate the Android or Windows app).
- Each AI job runs Codex with a **read-only sandbox** in the repository, started inside a Herdr workspace by `factory-server run-codex <job-dir>`. Job artifacts are kept in `data/jobs/<uuid>/`; keep this directory out of Git tracking.
- Mac Studio listens on loopback; Tailscale Serve adds tailnet-scoped HTTPS. Every API route requires a separate bearer token. Do not use Funnel.
- The source browser graph can still be edited offline; the two transport paths are deliberately separated.

## Next incremental changes

1. Implement AST-resolution using Oxc instead of regex heuristics for imports/re-exports/tsconfig aliases.
2. Replace complete AI graph replacement with validated minimal patches and a visual diff.
3. Add stable plan IDs and optimistic revision locking for multiple devices.
4. Hook the approved PlanningArtifact into Implementation and Verification stages.
