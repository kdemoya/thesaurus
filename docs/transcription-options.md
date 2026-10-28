# F46: Transcription quality and provider choices

**Status: selection, local model management and initial cloud adapters implemented (2026-10-19); runtime acceptance pending.** Base remains the automatic offline default. The new choices apply after saving Audio setup in the updated app. No live cloud request, recording, model download or audio evaluation was performed during this implementation.

## Delivered behavior

- Shared browser/native **Audio setup → Transcription** selection, language policy, optional speech detection, readiness states and explicit model downloads. Existing Base/custom settings deserialize unchanged and old sessions retain their original execution semantics.
- Normal, Large and Silero v6.2.0 use pinned Hugging Face revisions, byte lengths and SHA-256 hashes. Downloads stream to private partial files, resume using validated HTTP ranges, support cancellation, check disk availability, verify the full hash and atomically publish outside the signed app bundle. Restart marks active transfers interrupted; resume is explicit. No model is downloaded at startup or in response to Start recording.
- New sessions pin their selected route, weight path/hash, VAD path/hash and language policy. Local automatic detection locks after the first nonempty result; **Multilingual** intentionally detects each chunk. Previous text from the same track is supplied as bounded local context. Structured local segments and remote word/token metadata are retained with chunk provenance. Each published transcript still creates an ordinary immutable document revision.
- Local CLI execution now lives behind the first-party `native.transcription` capability, with provider leases and recording/queue idle guards. Audio ownership, downloads, disclosure and transcript publication remain in the host. The user-plugin native-request allowlist does not expose transcription execution, audio files or credentials.
- Deepgram Nova-3 uses the completed-file listen API with model-improvement opt-out requested. Soniox uses Async v5: upload, create once, poll, retrieve and delete job/file. The company option accepts an explicit **Deepgram-compatible listen URL**, using Token authentication; it is not a generic arbitrary transcription protocol.
- Remote setup uses Keychain or environment references, a destination-specific audio authorization and a daily aggregate **audio-minute** cap. Both tracks count, rounded up to whole seconds per chunk; uncertain attempts continue to count. These are reserved audio seconds, not provider invoices or a currency guarantee. Credentials for company endpoints saved through the UI use endpoint-scoped Keychain names.
- Upload authorization is checked before delivery and between Soniox operations. Changing Audio setup applies to new recordings; an existing session retains its previously authorized route and pinned limit. **Revoke audio uploads now** revokes all saved audio routes and blocks queued/future sends. A request already in flight may finish. Source-sharing permission and reasoning account selection remain separate.
- Durable receipts reserve before sending, record IDs and outcomes, and prevent duplicate sends. HTTP redirects and automatic application retries are disabled for provider operations. Unknown delivery after a timeout/crash is marked uncertain and never resent. Soniox cleanup failures are visible and can be retried as deletion-only operations using the original API key; changing accounts cannot turn another account's 404 into a successful cleanup receipt. A lost upload/create reply may leave an unknown resource: inspect the provider account when no identifier was returned.

### Setup

1. Open **Meetings → Audio setup** and choose a provider.
2. For **Normal/Large**, download its weight file plus **Silero speech detection** if enabled. Base continues to work while these download. Model byte size is not its RAM requirement; there is no claim that Large can keep up on the connected machine.
3. For **Deepgram/Soniox**, save your provider API key to the host Keychain, or enter an advanced `env:NAME`/`keychain:NAME` reference. Review destination, provider policy and pricing, set the daily limit, and authorize audio uploads. Saving checks credential availability, without making a paid provider request.
4. Save audio setup, then use the existing direct **Start recording** action. A not-ready local model or revoked cloud grant rejects Start/Resume with an explanation. Changing setup does not migrate an active recording to a new model.

Deepgram currently uses the default global `api.deepgram.com` endpoint; a specific regional processing guarantee is not asserted. Soniox currently uses its United States `api.soniox.com` endpoint. Regional route selection is follow-up work; do not assume a regional API key redirects these explicit URLs. See [Deepgram regions](https://developers.deepgram.com/reference/regional-endpoints) and [Soniox regions](https://soniox.com/docs/data-residency).

### Current limits and remaining work

The initial cloud adapters process **completed chunks**, not continuous WebSocket streams. They do not maintain cross-chunk speaker identities. The local engine still launches/reloads per chunk and uses CPU/Apple Accelerate. A four-chunk queue pauses recording on backpressure; there is no silent downgrade or cloud fallback. Explicit language choice and same-track context help continuity, but audio overlap, speech-aware capture boundaries, acoustic duplication removal, signal meters and persistent loaded-model reuse remain to implement.

Processed audio is still deleted. Retained-audio final passes and audio-specific word-error/latency/cost metrics in F42 are not implemented. Older transcript-only sessions cannot be retranscribed from original audio. Complete guided F45 onboarding, OpenAI/AssemblyAI adapters and alternate local engines remain follow-up work. The underlying personal/project evaluation framework is unchanged.

**Acceptance:** service, desktop and Wasm compilation succeeded during implementation. Optimized web distribution, service/native builds and formatting completed successfully. A separate updated bundle is staged; activation is deferred because the normal workspace is recording. No automated tests were added or run. Model download/resume/corruption interaction, recording accuracy/speed, browser/native interaction and live provider cleanup/budget behavior require explicit runtime verification. No accuracy or real-time performance result is inferred from compilation.

### Code and API

- Contracts: `crates/contracts/src/transcription.rs`; host catalog, local dispatch and cloud adapters: `crates/core/src/capture/transcription/`.
- Local execution provider: `crates/core/src/plugins/native/transcription.rs`; UI: `apps/ui/src/transcription_setup.rs`.
- Admin-only endpoints: `GET /api/v1/transcription`, `POST /api/v1/transcription/models/{normal|large|vad}` (`download`/`cancel` action), `POST /api/v1/transcription/credential`, `POST /api/v1/transcription/revoke`, per-session `GET /api/v1/live-meetings/{id}/transcription-receipts`, and deletion-only `POST /api/v1/live-meetings/{id}/transcription-receipts/{chunk}/cleanup`.
- Recording selections use the existing idempotent `/api/v1/live-meetings/settings` endpoint. Model actions are fenced by the durable transfer state; credential values are not included in settings or receipt responses.

The remaining sections preserve the broader accepted product requirements. Use the delivery list above to distinguish current behavior from that target.

## Preset mapping

Offer the same transcription selection during onboarding and later in Audio settings:

| Choice | Model/provider | Intended use | Approximate model download |
| --- | --- | --- | --- |
| **Local · Normal** — recommended local preset | Whisper Large v3 Turbo, Q5_0 quantized weights through whisper.cpp | Balance recognition quality, resource use and live transcription latency | 547 MiB |
| **Local · Large** | Full Whisper Large v3 through whisper.cpp | Quality-focused processing for machines with more resources; a future final-pass workflow can use it when live processing cannot keep up | 2.9 GiB |
| **Cloud** | Deepgram Nova-3 or Soniox initially; OpenAI and AssemblyAI follow | Users who prefer remote processing or cannot keep a local model running comfortably | No local recognition-model download |
| **Company endpoint** | An explicitly configured, supported organization-hosted transcription endpoint | Centralized GPU capacity and organization-controlled processing | Depends on the endpoint |

Normal/Large are user-facing quality presets, not literal upstream model-size names. Both local models are multilingual. The artifacts and quantization are pinned in the implemented catalog. Minimum/recommended hardware and quality/latency guidance still require representative measurements. Download sizes are weights only, not RAM requirements or total installed footprint. VAD and optional accelerator assets need their own size accounting. Larger models are not guaranteed to win on every language or recording.

The initial mapping favors a smaller Turbo download for Normal and the original full model for Large. The upstream Turbo model reduces decoding layers from 32 to four and trades some quality for speed. The current bundled Base model should not be relabeled Normal. Existing custom paths/selections remain available under Advanced settings and are preserved through migration. Parakeet/FluidAudio remains an additional local provider candidate after the first presets.

Sources: [whisper.cpp model artifacts and sizes](https://github.com/ggml-org/whisper.cpp/tree/master/models), [Whisper Large v3 Turbo model description](https://huggingface.co/openai/whisper-large-v3-turbo).

## Target: local model readiness and storage

- Recommend Normal for the local path. Show the actual engine/model version, download size, installed footprint, supported acceleration and readiness state before selection. Offer Large with its additional resource/latency tradeoff.
- Use passive hardware information to inform the recommendation; do not promise real-time processing from RAM capacity alone. An optional, explicit local check can measure processing speed, memory use and whether both microphone/system tracks can keep up with capture.
- Download only after the user chooses the model. Provide progress, cancellation, resumable transfers where supported, pinned revisions, verified hashes and atomic publication. Canceled, partial or corrupt downloads never become ready models.
- Store downloaded weights in a private service-owned model cache **outside the signed application bundle**. Changing a model must not modify or re-sign the app or invalidate macOS capture permissions. Built-in assets can remain read-only; the UI does not ask users to enter executable/model paths for ordinary presets.
- Keep existing model files usable while another model downloads. Model changes apply to a new recording; an active session pins its engine, model artifact, language policy and processing parameters through Pause/Resume. Show which session uses an older selection. A separate final-pass operation can choose a different ready model.
- Distinguish not downloaded, downloading, ready, unavailable, and repair needed. Enable the recording workflow once its selected prerequisites are ready. Start remains a direct action with the prepared defaults, with meeting metadata reconciled afterward. No repeated setup form is required.
- If a selected model cannot keep up, show the backlog and offer an explicit switch for the next recording or a final-pass workflow. Do not silently downgrade the model or upload audio to a cloud provider.

## Target: quality pipeline

Both local presets need more than a larger weight file:

1. Speech activity detection (initial candidate: Silero VAD), silence/noise handling, and visible input-level/clipping feedback. Avoid generating transcript text from non-speech while preserving quiet speech.
2. Explicit language preferences, including automatic/multilingual handling where supported. Preserve a stable meeting-level language policy instead of re-detecting independently on every short chunk. Evaluate mixed-language speech separately from single-language speech.
3. Speech-aware chunk boundaries, overlap/context continuity and duplicate removal without losing words. Preserve source offsets when segmentation or VAD removes silence. Keep microphone/system tracks distinguishable and handle acoustic duplication between them.
4. Bounded live processing with partial/final states, explicit gaps/errors, cancellation and service-owned model lifetime. Reuse a loaded local model where supported rather than paying cold-start costs on every chunk.
5. Optional private audio retention with a clear retention/deletion choice to support a final higher-quality pass. The current recorder deletes processed chunks, so earlier transcript-only sessions cannot be retranscribed from the original audio. Final-pass text creates a new cited transcript revision and marks prior derived results stale; it does not overwrite accepted conclusions silently.

## Target: cloud providers and plugin boundaries

Keep transcription independent of the primary model/agent used for summaries and other reasoning. Implement local, cloud and company endpoints behind a common Rust-service transcription contract. Extend the existing plugin capability/grant model where needed; arbitrary plugin code must not gain audio access or provider credentials merely by being installed. Recording ownership, disclosure decisions, session receipts and transcript revisions remain host-owned.

- Start with Deepgram and Soniox adapters, then evaluate OpenAI and AssemblyAI. Capability metadata distinguishes live input, completed-file processing, partial/final output, languages, timestamps, speaker labels, terminology hints and model-specific restrictions.
- Onboarding offers bring-your-own API credentials and budgets; managed billing can be a later option. Prefer the host credential store/reference mechanism, with destination-scoped resolution. An installed agent subscription does not itself authorize a separate audio API request.
- Show the selected audio destination, applicable processing region, documented retention settings, usage estimate and budget before enabling a cloud route. Treat training/retention policies as provider/account-specific facts to verify, not blanket guarantees.
- Use explicit cloud disclosure authorization for recording audio; the transcript's source-sharing toggle alone is insufficient. Remember an authorized route with clear controls to revoke it. Starting capture does not authorize an unselected external fallback.
- Preserve session-level streaming context, handle speaker-label revisions, and expose reconnect gaps and uncertain delivery. Separate microphone/system streams may increase billable audio time; cost estimates and receipts account for their actual billing units. Do not silently resend uncertain billable audio.
- Normalize provider output into immutable transcript revisions with engine/model/version, source track, timestamps, language and speaker-label provenance where available. A diarization label is not a verified person's identity.

API contracts: [Deepgram completed-file listen](https://developers.deepgram.com/reference/speech-to-text/listen-pre-recorded), [Soniox async create](https://soniox.com/docs/api-reference/stt/transcriptions/create_transcription), [Soniox upload](https://soniox.com/docs/api-reference/stt/files/upload_file), [Soniox model versions](https://soniox.com/docs/stt/models).

Provider research: [Deepgram](https://deepgram.com/pricing), [Soniox](https://soniox.com/pricing), [AssemblyAI](https://www.assemblyai.com/pricing), [OpenAI transcription workflows](https://developers.openai.com/api/docs/guides/transcription). Availability, supported features and prices are verified again when implementing each adapter.

## Remaining delivery and acceptance

1. **Implemented; runtime checks pending:** transcription-provider contracts, model catalog/cache and Normal/Large settings/readiness UI, with old settings and pinned session artifacts preserved.
2. **Partially implemented:** VAD, language policy and bounded prior-track context. Finish persistent loaded models, overlap/segmentation/duplication controls and acceleration options, then measure whether each preset can keep up with both selected tracks.
3. **Initial chunk adapters implemented; live acceptance pending:** Deepgram and Soniox with audio authorization, credentials, audio-minute limits and durable receipts. Add continuous streaming, regional selection and later adapters through the same host contract.
4. Add optional retained-audio final passes and quality comparisons through F42's project/personal evaluation workflow.

Acceptance should compare the same consented audio against a human reference, including English, Spanish, mixed-language speech, names/company terms, accents, silence, background noise and overlapping speakers. Record word error rate, dropped/invented text, speaker attribution where supported, boundary duplicates, latency, queue depth, memory/power and cloud cost. Evaluate Normal versus Large on identical input before claiming the Large preset is more accurate. Pin model/engine/quantization/VAD/language configuration and audio provenance for repeatability; external evaluation requires its own authorized destination and budget.

Verify cancellation/corrupt downloads, offline readiness, old-setting migration, consistent native/browser selection, active-session pinning and future app updates without losing the signing identity. These runtime acceptance requirements remain open; no new audio, model download or provider request was performed during implementation.
