# Test prompts for gebra.v2-server

The planning requests entered in the app to plan against `gebra.v2-server` (FSD root `services/client/src`). Paste them into the request box unchanged. A different wording gives a different plan, and then a change in the result cannot be attributed to a change in the server.

When a prompt has to change, add a new entry with a new id instead of editing an old one, and note in the log which id was used.

## Prompts

Each entry: the exact text, whether the plan was empty, and the target (whole plan or a module).

### G1 — refactor features/variant-filter

Plan: empty. Target: whole plan.

```text
features/variant-filter 의 로직을 깔끔하게 정리하고 싶습니다. 리팩토링 해주세요.
```

## Requests already sent

The server keeps the text of every planning request. On Mac Studio, this prints them oldest first:

```sh
source ~/.config/software-factory/server.env
for f in $(ls -tr "$FACTORY_DATA_DIR"/jobs/*/input-plan.json); do
  date -r "$f" '+%Y-%m-%d %H:%M'
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["instruction"])' "$f"
  echo
done
```

## Log

One line per run: date, server version, prompt id, branch @ commit shown in the job status, attempts used, outcome.

| Date | Server | Prompt | Based on | Attempts | Outcome |
| --- | --- | --- | --- | --- | --- |
|  |  |  |  |  |  |
