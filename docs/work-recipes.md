# Saved work recipes (F32)

Recipes save a repeatable project analysis with a named template, declared editable inputs (`{{input_name}}`), selected project sources, ACP agent connection, requested Markdown/JSON format, prompt character budget and daily run limit.

Open **Recipes** (`0`) to create or edit a recipe. Saving appends a version. Saved versions are retained; archiving prevents new previews and runs. Source scope must remain inside its project. Inputs are expanded once, so input text cannot introduce a second template substitution.

## Preview and run

1. Supply every declared input on a saved revision.
2. Choose **Preview context and usage**. Inspect the exact prompt, pinned document revisions, approximate input tokens, omitted documents/characters, connection and remaining daily runs.
3. Explicitly approve this use of the connection and selected source disclosure, then run.
4. Inspect streamed progress in Activity. The recipe receipt retains the exact inputs, saved recipe version and context delivered to the runtime.

Changed recipe settings, document revisions or disclosure flags invalidate the preview. Runtime reservation checks the signature and recipe daily limit within the same transaction as the connection daily limit and run creation. A repeated request with the same idempotency key cannot start another runtime. No fallback connection is selected.

Whole documents that do not fit the prompt budget are omitted and counted. This preserves exact source lines; choose narrower sources or a larger budget when useful evidence is omitted. Token counts are conservative estimates, not provider billing measurements. Daily run limits count attempts, including interrupted/failed runs. Markdown/JSON is requested in the analysis prompt; model output remains untrusted and is never executed or treated as a task decision.

## CLI

```sh
thesaurus recipe list PROJECT_ID
thesaurus recipe save recipe.json
thesaurus recipe save edited-recipe.json --id RECIPE_ID
thesaurus recipe get RECIPE_ID --version 1
thesaurus recipe preview RECIPE_ID --expected-version 1 --inputs-json '{"topic":"rollout"}'
thesaurus recipe run RECIPE_ID --expected-version 1 --inputs-json '{"topic":"rollout"}' --signature PREVIEW_SIGNATURE --approve-usage
thesaurus recipe runs RECIPE_ID
```

Example `recipe.json` (replace IDs with your selected project, source and configured ACP connection):

```json
{
  "project_id": "PROJECT_ID",
  "expected_version": 0,
  "name": "Rollout review",
  "template": "Review {{topic}}. Cite blockers, decisions and actions.",
  "inputs": ["topic"],
  "source_ids": ["SOURCE_ID"],
  "connection": "codex-acp",
  "output_format": "markdown",
  "character_budget": 8000,
  "max_runs_per_day": 5,
  "archived": false
}
```

The CLI supplies an idempotency key when omitted. API requests must include it. Creation expects version 0; edits require the current version.

## API

- `GET /api/v1/projects/{id}/recipes`
- `POST /api/v1/recipes`
- `GET/PATCH /api/v1/recipes/{id}` (`?version=N` reads history)
- `POST /api/v1/recipes/{id}/preview`
- `POST /api/v1/recipes/{id}/run`
- `GET /api/v1/recipes/{id}/runs`

Save, preview, run and receipt history require the user client. Scoped agents can read permitted recipe definitions only when they can read every project source. Preview/run never relax source destination controls.

Verification is tracked in [backlog progress](backlog-progress.md). No live paid or subscription request is required to save/preview a recipe.

## Retrieval policy and live-only sources

Recipe previews retain selected sources and effective retrieval policy/upstream versions locally. Only index/hybrid sources enter the prompt and agent memory grant. Cached live-only content is excluded, with explicit omitted source/document/character counts; the wire prompt uses anonymous counts rather than omitted source identifiers or policy configuration. Live reads need a separate approved retrieval. Source policy and connector-version changes invalidate the preview, including index → live → index changes, and are checked again at launch. Disclosure approval applies to the included sources; omitted private sources do not grant any access.
