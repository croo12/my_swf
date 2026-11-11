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

## Output contract

`Plan` is the existing Planning Workbench schema v0.3: `nodes[]` (id/path/description/inputs/outputs/x/y), `edges[]` (id/from/to/type), `requests[]`, `version`. The server verifies all edge endpoints, unique node IDs, FSD paths, and I/O array types before saving or accepting AI output.

The current UI arranges modules horizontally inside always-visible `app / pages / widgets / features / entities / shared` lanes. `app` and `shared` have direct segments; all other layers have `slice / segment` levels.

## Trust boundary

- Rust verifies machine-readable graph contracts. AI generates **proposals**, never direct writes to a persisted approved Plan.
- Herdr executes exclusively on Mac Studio (even when users operate the Android or Windows app).
- Each AI job has its own **detached Git worktree**, plus durable logs and proposal artifacts in `data/jobs/<uuid>/`. Keep this directory out of production Git tracking.
- Mac Studio listens on loopback; Tailscale Serve adds tailnet-scoped HTTPS. Every API route requires a separate bearer token. Do not use Funnel.
- The source browser graph can still be edited offline; the two transport paths are deliberately separated.

## Next incremental changes

1. Implement AST-resolution using Oxc instead of regex heuristics for imports/re-exports/tsconfig aliases.
2. Replace complete AI graph replacement with validated minimal patches and a visual diff.
3. Add stable plan IDs and optimistic revision locking for multiple devices.
4. Hook the approved PlanningArtifact into Implementation and Verification stages.
