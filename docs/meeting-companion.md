# F43: Meeting companion

## Start recording

Click **Start recording** in the native tray dropdown or **Meetings → Recording companion**. It immediately requests recording with the saved audio setup, creates a timestamped recording, and leaves the project and meeting series unassigned. No meeting form or extra confirmation precedes the request. The browser and desktop control the same Rust service session.

Recording comes preconfigured in the complete macOS app:

- **Microphone:** the system default input. It is resolved when Start/Resume opens an input. Choosing a specific microphone instead keeps that device selection; an unavailable selected device is not silently replaced.
- **System audio:** enabled where the connected audio provider supports it (macOS 13+). The tray and Meetings view describe this scope before Start: all apps except Thesaurus.
- **Transcription:** bundled offline whisper.cpp with the multilingual Whisper Base model and automatic language detection. No API token, executable path, model download or setup form is required to start.
- **Chunks:** 20 seconds by default.

**Settings → Audio & transcription** contains optional customization. Change the microphone, turn system audio on/off, choose a transcription preset/provider, or use **Custom local model** and **Advanced recording settings** for executable/model paths and chunk duration. Save changes for subsequent recordings. Existing saved custom settings remain in effect. Packaged paths are resolved against the current app, including when resuming a recording after moving the app.

Audio preferences group microphone and system audio separately from transcript model and language. **Local models & speech detection** contains optional model downloads, speech detection and hardware information. **Advanced recording settings** contains the processing interval and recording diagnostics. The save bar shows unsaved changes and confirms successful saves without closing Settings.

Base and the engine are packaged while building the app. Normal, Large and Silero weights download only through an explicit Settings action, independently of recording; first launch never starts a download. `scripts/build-transcription.sh` pins and verifies the engine source, model and licenses; the resulting engine has only system-library dependencies. The bundled engine uses CPU/Apple Accelerate. Model/engine overrides remain explicitly local, trusted executables.

macOS can request necessary permissions when recording starts. Opening the widget or launching the service never starts recording. Start records immediately after the input is ready; it does not wait for the first transcription chunk.

After **Stop** and queued transcription finish, use **Meeting details** to rename the recording and optionally assign a project and recurring meeting series. The transcript source is then attached to the selected project, and a rename updates its ordinary document revision/search index. Assignment adds source history and cannot move an already assigned recording to another project. Unassigned recordings remain in Sources and the session list; they are not silently assigned to the previous meeting's project.

Microphone capture supports macOS 12+. System audio uses ScreenCaptureKit and requires macOS 13+. The recorder attaches audio outputs only; it does not deliver or retain screen images. The local browser controls recording on the service's computer, using that computer's inputs and permissions.

## Recording and transcription

The Rust service is the sole recorder owner. A SQLite constraint and serialized control path allow one starting/recording/closing session across all clients. Start/control/note requests have idempotency keys; stale control versions reject rather than controlling a newer session. Switching surfaces does not start a second recording.

The UI and menu bar show starting, recording, pausing, stopping, paused and stopped states. The elapsed clock counts recording time, excluding deliberate pauses. **Pause** closes both inputs; **Resume selected inputs** explicitly reopens the same selected inputs. **Stop** ends the session permanently. Start another session to change a microphone or audio scope.

Pause/Stop asks the platform bridge to close inputs and flush its partial chunks. It waits up to five seconds before killing a stalled bridge, recording an explanation that the final partial chunk may be missing. Completed queued chunks continue transcribing after Pause/Stop. A four-hour limit, failed input, failed transcription or full processing queue pauses recording with an explanation. The service never switches inputs or falls back to a remote transcription service.

Microphone and system audio are converted separately to mono 16 kHz PCM WAV chunks. Each transcript line identifies its recording offset and track. Offsets are approximate chunk start times. These labels do not identify individual speakers; the same speech may appear on both tracks when the microphone hears the meeting speakers. Headphones can avoid that overlap. Chunk boundaries do not perform diarization or guarantee uninterrupted sentence recognition.

The queue holds at most four chunks across all live sessions; one live-chunk transcription runs at a time. Each WAV is bounded to 4 MB, local transcription to 300 seconds, and the accumulated transcript to 190 KB. The older file-transcription workflow has its own processing slot. Temporary audio is held under private `live-audio/` storage and removed after processing. Restart removes unprocessed temporary audio and reports the interruption; transcript and notes remain. Parent death closes a lifetime pipe so the bridge does not keep recording after its service exits. A service restart never resumes audio automatically.

## Context, notes and actions

- **Quick note / bookmark:** save timestamped user-authored notes and bookmarks for the selected session. Alt+B bookmarks while focus is inside the meeting panel. The series association does not automatically accept a recurring-meeting baseline. Notes persist separately from the transcript; they are not fabricated transcript quotes. They guide the summary generated with the saved meeting model.
- **Live transcript:** each completed chunk updates one stable local transcript source and immutable document revisions. Local extraction updates candidate action items. The source is attached to the chosen project, subject to existing project scope limits.
- **Context:** inspect the project's saved accepted brief, its staleness label, exact cited evidence and open/candidate actions. These are saved project records, not an automatic live model conversation. Review candidate actions individually before accepting them.
- Transcript imports are local-only. Each live update keeps external sharing off. After stopping and processing the queue, sharing can be changed through the ordinary source controls. Finished meetings are summarized automatically using the model saved during onboarding or in Settings. This narrow transcript-and-notes route does not enable general source sharing or grant an external agent memory access. Audio uploads occur only when a remote transcription route was explicitly saved and its separate audio grant remains active. The installed executable is trusted local software with the user's privileges.

## Automatic meeting summaries

After **Stop**, the service waits for the final transcription chunk and its document revision, then generates **Key points**, **Decisions**, **Action items**, **Open questions**, and **Risks and blockers** automatically. The meeting screen can be closed. There is no per-meeting model picker, access-approval checkbox or required summarize button.

**Settings → Automation** controls automatic summaries and the summary model. The default follows the primary model selected during onboarding; local mode creates a bounded extractive digest with exact source citations and no model request. ACP, Codex CLI, Claude CLI and compatible APIs use their configured accounts. Saving a model is a standing preference for subsequent recordings and their transcript-and-notes processing. The connection is pinned when recording starts, so changing defaults cannot redirect an existing recording. Turning automation off also disables queued summaries; a run already in progress can finish.

Each finding includes an exact transcript quote and immutable source revision. Owners and deadlines remain unspecified unless supported by the cited line; track labels do not identify speakers. Notes guide emphasis without replacing transcript evidence. Action items become candidates for **Accept action** or **Dismiss**. Existing matching actions retain their review state when a summary is regenerated.

The durable queue reserves each automatic attempt once, using the normal connection budget. Occupied runtime slots wait in the queue. Failed, canceled or interrupted provider attempts are shown and never resent automatically. **Regenerate summary** or **Create summary** uses saved preferences in one click, including recordings made before automation was introduced. Earlier summaries remain in history. A changed transcript revision marks older results stale and prevents accepting their older candidates.

The saved summary route permits only this meeting's tool-free transcript-and-notes prompt. General source sharing remains off unless changed separately. Per-source destination blocks revoke the route, including during processing. The app does not switch to a different account when a model fails or a daily limit is reached.

The host provides the complete available transcript in a bounded, tool-free prompt. It does not expose a filesystem, terminal or memory MCP to the summarizing agent. The transcript is capped by recording at 190 KB; the numbered transcript, title, capture warning and notes together must fit within 256 KB. Exceeding this bound fails before usage reservation, with no silent truncation. Provider context limits may be smaller and return a failure. API requests have a 120-second timeout and an 8192-token output budget; output is bounded to 64 KB and 30 findings per section. Invalid JSON, citations, owner/date evidence or incomplete provider output create a failed summary, with no new action candidates. A restart marks interrupted requests and reconciles completed results locally without invoking a model again. Verified quotes establish the source location, not the correctness of a model's interpretation; review conclusions against their evidence.

**Prior delivery status (2026-10-19, before automatic processing):** native service, optimized shared Wasm web distribution and separate macOS bundle builds passed; formatting, shell syntax, packaged service/web hashes and bundle signature passed. With explicit user approval, the app/service was activated while recording, transcription, capture and agents were idle. The existing stopped recording and other data counts were preserved. The previously selected Codex adapter is configured with a three-request daily limit and ChatGPT-only authentication, without API fallback. The native Meetings view displays the summary controls and selected connection. No automated tests, live model request or transcript disclosure occurred; generation quality and result-review interactions remain unverified. Source sharing is still local-only until the user changes it.

## Native surfaces and shortcuts

The native menu bar icon toggles a compact dropdown directly beneath the icon. It has no title bar, resize handles or dashboard navigation, sizes to its contents (352 points wide, capped at 460 points high), and dismisses on outside click or Escape. It shows recording status/time, pause/resume/stop, and a quick note/bookmark field. The dropdown is preloaded while hidden; opening it refreshes the shared state. Its menu exposes the workspace, Pause, Stop and **Pause recording and quit**. Stop ends the active session, or the most recently updated paused session when nothing is recording. The icon shows `REC` while starting, recording or closing, and `!` when it cannot read service status.

| Shortcut | Action |
| --- | --- |
| Cmd/Ctrl+Shift+M | Open tray dropdown while the native app is active |
| Cmd/Ctrl+Shift+P | Pause recording while the native app is active |
| Cmd/Ctrl+Shift+S | Stop recording while the native app is active |
| Alt+B | Bookmark from inside the meeting panel |

These are app shortcuts, not system-wide registered hotkeys.

**Start recording** in the dropdown sends one direct start request. **Audio setup…** opens the saved input/transcription configuration separately. **Add recording details** opens the workspace for later organization. Opening the dropdown never starts recording.

**Pin widget** opens the separate floating companion at its remembered edge (right by default). The tray dropdown always stays attached to the icon, independently of widget position. **Panel** in the detailed companion hides on focus loss. **Dock left/right** keeps the widget visible above other windows and remembers its screen edge and vertical position. **Compact/Expand** changes the companion's height and detail density. **Workspace** opens the full Meetings view. All surfaces share the existing terminal palettes and theme preference.

Closing a native window hides it, retaining the menu bar controls and recording session. Quitting pauses recording and confirms the control request before removing those controls; queued transcription can finish through the selected provider in the independent service. Closing a browser tab does not stop recording; use Pause/Stop explicitly.

## Transcription choices

**Settings → Audio & transcription** offers bundled Base, **Local · Normal** (Whisper Large v3 Turbo Q5_0), **Local · Large** (full Whisper Large v3), Deepgram Nova-3, Soniox Async v5, a company Deepgram-compatible endpoint and custom local paths. Download the selected local model and, when enabled, Silero speech detection before saving. Existing recordings retain their model/artifact, language and authorized audio route. Use the explicit revoke action to stop uploads; changing defaults applies to new recordings.

Cloud providers require a separate API key/reference, daily audio-minute limit and explicit audio-upload authorization. Both tracks count toward the aggregate UTC-day limit, including uncertain attempts. **Revoke audio uploads now** blocks future sends; an in-flight request may finish. Audio is processed as completed chunks, not a continuous word-by-word stream. **Transcription delivery receipts** shows outcomes, provider IDs and pending Soniox cleanup. Local source-sharing policy still applies independently to transcript reasoning.

See [F46 implementation, provider references and remaining quality work](transcription-options.md). Build/compilation alone does not establish real microphone accuracy, model capacity, provider entitlement, or runtime acceptance.

## Permission persistence

If macOS repeatedly asks to record the screen and audio, see [macOS permissions and persistent build signing](macos-permissions.md). That system-audio dialog is separate from microphone authorization. Reusing one certificate across updates preserves code identity; changing the signer may require one fresh OS grant.

## Build and implementation

```sh
./scripts/build-audio.sh debug
./scripts/build-transcription.sh debug
./scripts/build-desktop.sh --debug
```

The complete desktop build includes the bridge beside the service executable and the engine/model in its adjacent `transcription/` folder; no Swift, CMake, Python or separately installed whisper.cpp is needed on the target Mac for recording. Building requires Apple's Swift compiler/SDK. Standalone development services find `thesaurus-audio` beside their executable. Restart an older running service to load the new endpoints.

The recording-first update changes the background service as well as the interface. Restart the service and reopen the app to load it; quitting the desktop app alone leaves the independent service running. Stop any recording and let its transcription queue finish before restarting. Browser sessions need pairing again after a service restart.

Rust implements durable sessions, recorder ownership, controls, queueing, transcription orchestration, memory import and shared web UI. `crates/core/src/capture/record.swift` is the small AVFoundation/ScreenCaptureKit boundary. The helper and app contain microphone/system-capture usage descriptions. Signed distribution and OS permission attribution still require the existing release workstream.

References: [Apple capture sample](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos), [Tauri system tray](https://v2.tauri.app/learn/system-tray/).

## Acceptance status

### Preconfigured recording refinement (2026-10-19)

The default setup now uses the system microphone, supported system audio and packaged offline transcription. Existing custom settings are preserved; configuration is optional and path overrides are advanced controls. The engine/model and their license notices are included by the desktop build. Rust/service/all-target and shared Wasm compilation passed, and the native audio bridge compiled. The optimized web distribution and separate macOS bundle built successfully (296.18 MiB); its signature and packaged service/audio/web/transcription assets match the build outputs. The new default path has not been exercised with real audio in this increment; previous synthetic recording acceptance remains recorded in [plugin acceptance](plugin-acceptance-audit.md).

Pinned upstream sources: [whisper.cpp v1.9.5](https://github.com/ggml-org/whisper.cpp/releases/tag/v1.9.5), [converted Whisper model repository](https://huggingface.co/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1), and [Whisper model license](https://github.com/openai/whisper/blob/v20250625/LICENSE).

### Recording-first refinement

Direct recording start, independent saved audio settings and post-recording title/project/series editing are implemented. Native/Wasm compilation, optimized web build, service build, debug desktop bundle build (140.00 MiB), formatting and inline JavaScript syntax checks passed. All 13 packaged web assets and the packaged service/audio executables match their build outputs. No recording, permission change or test suite was run for this refinement; interactive recording and reconciliation acceptance remain pending. The running service and app were not restarted.

The optimized shared Wasm web build, native Rust compilation, Swift bridge compilation and full debug macOS bundle build passed (139.00 MiB). Packaged helper/service hashes match their build outputs, and the app bundle retains `dev.thesaurus.desktop` with both usage descriptions. Final formatting passed. Implementation and compilation are recorded separately from runtime acceptance. This increment has not recorded microphone/system audio, changed OS permissions, invoked a live model or run test suites. Interactive permission prompts, actual dual-input audio quality, final-chunk flush, device disconnect behavior, tray/window interactions and dock restoration remain to be exercised on the installed Mac. Earlier F01–F42 acceptance results do not establish these new paths.

### Tray interaction refinement

The tray dropdown uses its own small webview/component; it no longer embeds the full meeting setup/history in a normal decorated window. Coordinates come from the tray icon rectangle and are constrained to that monitor's working area. Keyboard opening uses the same anchor. Input focus and notes persist while hidden; Escape closes the dropdown (or returns from quick start), and a second tray click dismisses it. The detailed/pinned companion remains available as a secondary action. This refinement has compilation/build evidence only; interactive native behavior remains pending.

The refinement’s final optimized Wasm build, native compilation, JavaScript syntax checks and formatting checks passed. The rebuilt macOS bundle is 139.61 MiB; every packaged web asset matches the final distribution. Quit/reopen the native app to load the new tray surface. No live recording, OS permission change or test suite occurred during this refinement.

### Full-screen Spaces

The first attempt added collection flags to the regular Tauri window. The user confirmed that it still failed over full-screen applications after reopening the rebuilt bundle. Compilation alone did not establish runtime correctness.

The replacement hosts the existing tray webview content in a newly allocated `NSPanel` subclass, with `NonactivatingPanel` set at initialization. It makes that panel key without activating `NSApplication` or calling Tauri's regular window show/focus methods. Tauri retains the hidden webview owner for IPC and lifecycle. The panel owns presentation, content size, tray anchoring, visibility and dismissal on loss of key status. Keyboard focus is transferred with the content view. The native object is never changed into another class at runtime.

The panel opts into all Spaces, full-screen auxiliary behavior and no full-screen tiling. On macOS 13+, it additionally uses `canJoinAllApplications`; macOS 12 uses the older all-Spaces/full-screen auxiliary flags. It uses the popup-menu window level. Native objects remain on the main thread.

References: [Apple: NSPanel](https://developer.apple.com/documentation/appkit/nspanel), [nonactivatingPanel](https://developer.apple.com/documentation/appkit/nswindow/stylemask-swift.struct/nonactivatingpanel), [canJoinAllApplications](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/canjoinallapplications).

Native compilation, formatting and the debug macOS bundle build passed for the replacement (139.85 MiB); all 13 packaged web assets match the existing distribution. Interactive full-screen display, Space retention, keyboard input and outside-click dismissal remain to be checked. Native UI inspection timed out during this change; the user's running app was not restarted.

## Automatic processing verification

Core tests cover final-chunk gating, cited candidate creation, pinned destinations, disabling queued work, concurrent delivery, idempotent retries, restart recovery and destination revocation. Service-worker tests use synthetic transcripts and a local mock API to check automatic local/model execution without a UI, one outbound attempt, and visible provider failure without resubmission. These checks do not invoke a real provider or record real audio.
