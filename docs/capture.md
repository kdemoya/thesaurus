# Independent audio processing and selected activity capture

Live microphone/system-audio sessions and the menu bar widget are covered in [F43: Meeting companion](meeting-companion.md). The file-import and selected-window workflows below remain independently controlled.

## F06: local audio, separate reasoning

Capture offers two independent destinations:

1. **Audio processing:** an explicitly selected installed whisper.cpp executable plus existing local model. Thesaurus copies the selected WAV into private temporary processing storage, invokes that local executable with fixed `--model`, `--file`, `--output-txt`, `--output-file`, `--no-timestamps`, and `--no-prints` arguments, and imports the transcript as a normal cited source. It adds that source to the chosen project. Thesaurus implements no audio upload, model download, paid transcription endpoint or automatic fallback.
2. **Reasoning:** an independently selected configured ACP connection. Transcription completion never invokes it. A separate explicit approval starts one transcript-only request through the existing runtime permission/cancellation/account-limit flow. The source must separately allow external agent access; the transcription form makes that permission visible. Raw audio is not included in the reasoning request or memory grant.

These flags match the [official whisper.cpp CLI documentation](https://github.com/ggml-org/whisper.cpp/blob/master/examples/cli/README.md). The configured executable is trusted local software running with the user's privileges; Thesaurus cannot enforce the behavior of a substituted executable. Use an installed whisper.cpp CLI and local model. Input must be RIFF/WAVE, at most 50MB. Output transcript is at most 200KB, processing is limited to 300 seconds, and one local transcription runs at a time. The adapter accepts supported WAV encodings as determined by the installed whisper.cpp build; conversion/recording is separate.

The audio form shows both destinations before processing. Jobs persist queued/processing/completed/failed status, transcription text, original local paths, resulting document/source, optional selected reasoning connection and associated runtime IDs. Audio lists show the most recent 50 jobs, with direct retrieval by ID for older jobs.

Temporary audio and adapter output files are removed after processing. A service restart marks unfinished jobs interrupted and removes incomplete temporary storage. It does not silently retry transcription or reasoning. User-provided original WAV files are untouched. Completed transcripts remain normal project memory and follow ordinary source retention/deletion controls, independently from temporary audio cleanup.

## F07: explicit selected-window activity

Activity capture currently supports macOS. The user clicks List windows, chooses a named app window and a project, selects an interval/retention/count budget, and explicitly starts capture. No screen listing, screenshot, microphone recording or OCR operation runs merely because settings exist, the daemon starts, or the page opens. App-wide status shows active capture and exposes Pause. The session belongs to the background service: closing a browser or desktop window does not pause it. Use Pause capture or Stop session explicitly; restarting the service pauses all sessions and requires explicit resume.

Rust calls [CGWindowListCopyWindowInfo](https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo(_:_:)) to enumerate named visible layer-zero windows and captures only the chosen window via the system screencapture utility. Before every frame, window ID, process ID, app name and title must still match. Window closure, title/app changes or OS permission errors pause the session, requiring an explicit new selection/resume. There is no fallback to full-screen capture. Use the explicit Request macOS capture permission button if needed; it calls CGRequestScreenCaptureAccess. macOS may require Screen Recording permission for the daemon/terminal process before named windows are visible.

Optional local OCR uses a small native Swift bridge to [Apple Vision VNRecognizeTextRequest](https://developer.apple.com/documentation/vision/vnrecognizetextrequest). It recognizes a single captured PNG and returns text locally; an installed Swift toolchain is required. If Vision's selected hardware fails, the bridge retries that same image using [supported CPU compute devices](https://developer.apple.com/documentation/vision/vnrequest/setcomputedevice(_:for:)); macOS 12–13 use the older CPU-only option. Both attempts remain local and are bounded together to 180 seconds, including cold Swift/model initialization. The rest of capture, orchestration, retention, storage and API are Rust. Failure of both OCR paths pauses capture. Retained OCR is searchable within the selected session using local SQLite FTS; it is not automatically shared with agents or sent to a model.

The bridge supports standard Apple Swift/SDK pairings. Older compilers use the legacy CPU option without referencing modern symbols; Swift 5.9 or newer requires a macOS 14 or newer SDK. A custom newer compiler paired with an older SDK is not supported. The legacy branch is inspected for compatibility; native runtime verification currently covers the installed modern toolchain only.

Limits:

- One active selected window at a time; 15–3600 seconds between captures.
- Retention 1–168 hours, maximum 1–100 frames per session.
- At most 4MB per PNG, 100KB OCR text, 200MB retained PNG storage globally.
- Frame count/expiry maintenance runs after capture, before inspection and on startup; the daemon maintenance task also purges once a minute.
- An in-flight screenshot/OCR may finish after Pause, but its result is discarded when the session generation/state changed. No later capture is scheduled from that worker.
- Service restart pauses all sessions. No configuration or previous active flag reactivates recording.

Inspect retained frames in Capture, search their OCR text, view the PNG, and explicitly delete a frame plus its OCR/search entry. Pausing preserves already retained frames until expiry/deletion; stopping prevents that session from resuming. Retention deletes application records and images from SQLite, rather than promising secure erasure of WAL pages or backups.

No global keyboard logging, browser history collection, microphone/system audio recording, automatically activated screen recording, auto-enrollment of apps or remote OCR is implemented.

## CLI

```sh
thesaurus capture transcribe --file audio-request.json
thesaurus capture audio
thesaurus capture audio-get AUDIO_ID
thesaurus capture reason AUDIO_ID --connection codex --approve-usage
thesaurus capture windows
thesaurus capture start --file activity-request.json
thesaurus capture sessions
thesaurus capture control SESSION_ID --expected-version 1 pause
thesaurus capture control SESSION_ID --expected-version 2 resume --approve-capture
thesaurus capture frames SESSION_ID --query 'deployment'
thesaurus capture image FRAME_ID
thesaurus capture delete-frame FRAME_ID
```

Audio request:

```json
{
  "project_id": "PROJECT_ID",
  "title": "Planning meeting",
  "audio_path": "/absolute/path/meeting.wav",
  "whisper_command": "/absolute/path/whisper-cli",
  "model_path": "/absolute/path/ggml-base.en.bin",
  "reasoning_connection": "codex",
  "allow_external": false,
  "approve_local_processing": true
}
```

`allow_external: false` keeps the transcript local. Enable source access explicitly before asking an external reasoning connection to retrieve it. Choosing a reasoning connection does not override source permission.

Activity request (use window values returned by the explicit windows command):

```json
{
  "project_id": "PROJECT_ID",
  "window": {"window_id": 123, "process_id": 456, "app": "Selected app", "title": "Exact selected title"},
  "interval_seconds": 60,
  "retention_hours": 24,
  "max_frames": 50,
  "use_local_ocr": true,
  "approve_capture": true
}
```

Both file commands generate an idempotency key if omitted. Supply a stable key in JSON for request retries. Capture activation and control are user-only endpoints and use versioned/idempotent writes.

## API

| Method | `/api/v1` path | Purpose |
| --- | --- | --- |
| GET/POST | `/capture/audio` | List jobs / approve local transcription |
| GET | `/capture/audio/{id}` | Job and transcript |
| POST | `/capture/audio/{id}/reason` | Separate approved reasoning request |
| POST | `/capture/permission` | Explicit macOS permission request |
| GET | `/capture/windows` | Explicit macOS window enumeration |
| GET/POST | `/capture/sessions` | Sessions / explicit start |
| POST | `/capture/sessions/{id}/control` | Versioned pause/resume/stop |
| GET | `/capture/sessions/{id}/frames?q=...` | Retained frames / local OCR search |
| GET | `/capture/frames/{id}/image` | PNG encoded as base64 JSON |
| DELETE | `/capture/frames/{id}` | Delete retained image and OCR |

All capture endpoints require the local user. Agents cannot start/resume recording, enumerate windows, access PNGs or change retention. User-approved transcript sources can be shared through the ordinary scoped memory API.

## Verification

Synthetic runtime verification exercised the installed-process adapter with a silent WAV and fixed mock whisper output, verified source import with no automatic reasoning and temporary-file cleanup, and exercised retained synthetic PNG/OCR deletion and count/age retention. The resumed native pass then exercised an actual synthetic Thesaurus window: retained PNG, real Vision OCR/FTS, native indicator/Pause, window-closure rejection, real active-session restart recovery and in-flight discard. No real work window or audio was captured; no OS permission changed. Cold OCR took 113 seconds in the recorded integrated run; recognition quality for arbitrary content is not established. See [verification](backlog-verification.md#native-retry-completed-acceptance).

### Native acceptance procedure

The user initially deferred these checks, then resumed them on October 17, 2026. The recorded pass closed the gates below using the isolated `.local/backlog-review-20261017` workspace and a synthetic window. For future repeats, confirm an available desktop session and keep production workspaces and other windows out of the capture scope. Retention expiry/deletion have separate synthetic-storage evidence; they do not require collecting work data.

| Check | Evidence needed |
| --- | --- |
| Native rendering and persistence | Launch the current bundled app against the isolated workspace; inspect the shared interface, apply a theme, quit/reopen that client and confirm the saved selection. |
| Explicit activation | Opening Capture and listing windows must retain no new frame. Select the synthetic window, inspect its app/title/project and limits, then explicitly start. If macOS permission is needed, let the user complete that OS interaction. |
| Window capture and OCR | Inspect the retained PNG for the selected window only; locate a distinctive synthetic sentence through OCR search and inspect its retained text. Record any recognition error rather than assuming perfect OCR. |
| Visible pause | Confirm the app-wide indicator, pause while capture is active and inspect subsequent retained-frame counts. A frame already processing when Pause was accepted must not be retained. |
| Changed window identity | Change or close the selected synthetic window. The session must pause without retaining a frame from a different window, with an explanation requiring explicit selection/resume. |
| Restart and retention | Restart the isolated service; confirm paused recovery and no automatic capture. Inspect retention expiry and stop/delete behavior using only the synthetic session. |

Record the app build, session version/state, timestamps and exact observed results. A screenshot of a native window or a successful mock OCR response alone cannot establish this entire checklist.
