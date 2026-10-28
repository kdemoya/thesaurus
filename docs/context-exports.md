# Portable work context exports (F40)

Open **Sources → Export portable work context**. Select sources, choose Markdown or JSON, prepare and inspect a snapshot, then download it. Both clients use the same local service endpoint; no upload is performed.

Exports contain selected current documents, immutable source revisions, tasks and their exact citations, effective observations, original observations in history, correction/conflict history, scope, observation dates and snapshot event watermark. Records depending on any excluded source are excluded in full. Suppressed/corrected originals remain visible in the explicit history portion of the export, preserving the audit trail.

JSON uses `thesaurus-context-export/v1`. Markdown includes readable line-numbered documents, tasks and observations plus the complete selected JSON snapshot/history. Document dates are observation dates; they do not assert when a claim became true in the real world.

```sh
thesaurus export --source SOURCE_ID --format markdown --output context.md
thesaurus export --source SOURCE_A,SOURCE_B --format json --output context.json
```

Files created by the CLI are private (0600). Without `--output`, the CLI prints the response including its body as JSON. Browser downloads follow the browser's normal download location. Keep the exported file in the same trust boundary as its sources.

`POST /api/v1/exports` takes `{"source_ids":["SOURCE_ID"],"format":"json"}`. Standard source grants and destination permission checks apply before any source is read. The entire export is captured in one SQLite read transaction. Payloads above 2MB or rendered output above 4MB fail explicitly; narrow the selected scope to export a smaller snapshot. No content is silently truncated.

Verification is tracked in [backlog progress](backlog-progress.md).
