# Suggested reusable procedures

F34 proposes a saved recipe from repeated successful work. Open **Focus → Procedures** in the browser or native app. The feature does not execute an agent, consume model allowance, write agent skill files or install anything automatically.

## Establish successful work

Choose a completed runtime run and inspect its original prompt and output. Name the procedure it represents, explicitly decide whether the work was successful, and record why. Two different completed runs with the same user-assigned procedure name are required before a suggestion appears.

Runtime completion by itself is not counted as success. Repeated reviews of a single run do not count as separate successful runs. Reviews can be revised or withdrawn with an optimistic version and immutable audit history. Grouping by a user-defined name is a user assertion of a common procedure, not automatic semantic clustering.

## Review a concrete reusable recipe

A suggestion retains supporting run IDs, model routes, source scopes, outcome reviews and bounded output excerpts. Inspect the full original run when an excerpt is truncated. Initial instructions come from the first supporting run's prompt, with known recipe source snapshots removed when their delimiter is present; generic prompts can contain work data and require careful editing.

Before installation, review and edit the recipe name, instruction template, declared `{{input}}` placeholders, source scope, selected ACP connection, output format, character budget and daily run limit. The installed scope must cover the suggestion's historical supporting sources within the current project. This prevents source information in an inherited prompt from silently losing its permission scope.

Enter a review note and confirm that you reviewed the proposed instructions and supporting runs. **Install reviewed recipe** atomically creates the saved recipe, its initial version, the suggestion-to-recipe link, immutable review history, workspace events and idempotency receipt. It does not run the recipe. Open **Recipes** to preview the concrete prompt and separately approve model usage.

Dismissal is durable. New successful reviews update the evidence without reopening a dismissed suggestion. Withdrawing success reviews can make support insufficient; existing installed recipes remain visible and manageable in Recipes. Installation always refreshes and checks the supporting reviews and expected suggestion version.

## CLI

```sh
thesaurus procedure review PROJECT RUN_A 'Weekly rollout review' --successful --note 'Useful cited dependency review'
thesaurus procedure review PROJECT RUN_B 'Weekly rollout review' --successful --note 'Repeated successfully with current sources'
thesaurus procedure list PROJECT
thesaurus procedure get PROCEDURE
thesaurus procedure reviews PROJECT
thesaurus procedure decide PROCEDURE dismiss --expected-version 1 --note 'Too specific to reuse'
thesaurus procedure history PROCEDURE
```

Use `--expected-version` when revising a run outcome review. Omit `--successful` to record a non-successful outcome or withdraw a previous success. `procedure install PROCEDURE --json REQUEST` accepts the concrete `ProcedureInstallRequest`: expected suggestion version, name, template, declared inputs, source IDs, connection, output format, character budget, maximum runs per day, review note and optional idempotency key. The CLI creates a key if absent; supply one to safely retry an exact installation.

## API

| Method | Route |
| --- | --- |
| GET | `/api/v1/projects/{id}/procedures` |
| POST | `/api/v1/projects/{id}/procedures/refresh` |
| GET | `/api/v1/projects/{id}/procedure-reviews` |
| POST | `/api/v1/procedure-reviews` |
| GET / PATCH | `/api/v1/procedures/{id}` |
| POST | `/api/v1/procedures/{id}/install` |
| GET | `/api/v1/procedures/{id}/history` |

All procedure endpoints require the authenticated user because supporting prompts and outputs are private review material. Run sources must belong to the selected project; historical supporting scope is retained when reviews change. Source destinations and usage approval are rechecked by the normal recipe execution workflow.

Lists and histories accept `offset` and `limit` (1–50) and return `next_offset`. Suggestions retain up to 20 supporting-run excerpts (1,000 characters each) with omission counts; all current outcome reviews remain available through paging. Suggestion lists and histories are bounded to 300 KB. Recipe limits match the saved-recipe workflow: template 6,000 bytes, name 200 bytes, up to 12 declared inputs, 1–50 project sources, character budget 1,000–14,000, and 1–100 runs per day. The additive `procedure_*` tables retain reviews, suggestions and immutable histories.
