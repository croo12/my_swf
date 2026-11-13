# Test prompts

Fixed planning requests for checking a release. Paste them into the request box unchanged, so results from different versions can be compared. A different wording gives a different plan, and then a change in the result cannot be attributed to the server.

When a prompt has to change, add a new entry with a new id instead of editing an old one, and note in the log which id was used.

## Prompts

### P1 — cancel a running job (this repository)

Repository: this project (`FACTORY_REPO_PATH` = this checkout). Plan: empty. Target: whole plan.

```text
실행 중인 Herdr 계획 작업을 취소하는 기능을 추가해줘. 작업 상태 표시줄에 취소 버튼이 있어야 하고 서버에도 취소 API가 필요해.
```

What it exercises: a `modify` module in `features/plan-job`, a server request (`METHOD /path` interface) that must reach `shared/api`, and a widget that depends on the feature. The request also asks for a server API, which cannot be planned; the proposal must contain client modules only.

### P2 — cancel a running job, short form (this repository)

Repository: this project. Plan: empty. Target: whole plan.

```text
plan-job feature에 실행 중인 계획 작업을 취소하는 기능을 추가하는 계획을 세워줘
```

What it exercises: the same change named by its slice, without mentioning the server.

### P3 — report sentence execution (gebra.v2-server)

Repository: `gebra.v2-server` on Mac Studio, FSD root `services/client/src`. Plan: empty. Target: whole plan.

```text
(not recorded yet: paste the exact request used on Mac Studio here)
```

What it exercises: several modules in one slice (`features/report-sentence`) and one widget (`widgets/ngs-report-editor`). The v0.1.6 answer to this request contained a types-only module, a public-API module that only re-exported, and outputs that restated interface names; from v0.1.7 these are rejected.

## Log

One line per run: date, server version, prompt id, branch @ commit shown in the job status, attempts used, outcome.

| Date | Server | Prompt | Based on | Attempts | Outcome |
| --- | --- | --- | --- | --- | --- |
|  |  |  |  |  |  |
