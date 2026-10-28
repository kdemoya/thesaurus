# Action previews and recovery (F39)

Open **Activity → Preview actions** to prepare a concrete local effect, inspect before/after fields, approve exactly those effects, execute and inspect the durable receipt. A preview alone changes no target record. Execution checks the recorded target fingerprint again.

## Registered providers

| Provider | Proposed effects | Recovery |
| --- | --- | --- |
| `task_status` | Change one task status, increment version, set execution timestamp, append task history/event/receipt in one SQLite transaction | Restore the recorded previous status as another version, only if the task still matches the exact execution result |
| `delete_activity_frame` | Delete exactly one retained PNG, OCR text and its search entry atomically with a receipt | Unsupported; no undo control is offered |

Receipts retain actual before/after state, contributor and execution time. Frame-deletion receipts retain only identifiers, capture time, size and text digest; they do not retain deleted image/OCR content. Repeated execution keys replay the original receipt without repeating effects. A preview cannot execute twice under another key. Undo has its own version and idempotency key; newer task changes prevent undo. Original receipt versions remain immutable in history.

These are the currently registered local providers. External Slack, repository publishing and other remote write providers are not registered, so the app cannot preview or execute those effects. Providers must supply a real effect preview and explicit recovery support before registration. Nothing in a model suggestion, source document or shared-memory grant authorizes an action.

## CLI/API

```sh
thesaurus action preview task_status TASK_ID --expected-version 2 --status completed
thesaurus action execute PREVIEW_ID --fingerprint FINGERPRINT --approve-effects
thesaurus action receipts
thesaurus action undo RECEIPT_ID --expected-receipt-version 1 --approve-effects
```

- `GET/POST /api/v1/actions/previews`
- `POST /api/v1/actions/previews/{id}/execute`
- `GET /api/v1/actions/receipts`
- `GET /api/v1/actions/receipts/{id}`
- `POST /api/v1/actions/receipts/{id}/undo`

All operations require the user client; agent grants cannot approve, execute or undo actions. Capture retention may remove a frame after preview; execution then fails without a partial deletion or success receipt.

See [verification](backlog-verification.md) for current exercised paths.
