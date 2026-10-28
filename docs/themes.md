# Themes

Use **Theme** at the bottom of the workspace sidebar to change appearance. The meeting companion keeps this control in its header. The shared Rust UI provides the same control in the desktop app and local browser.

| Choice | Appearance |
| --- | --- |
| Terminal | Original charcoal, orange and violet palette; default for new installations |
| Midnight | Dark blue surfaces with pale violet and cyan accents |
| Phosphor | Dark green terminal palette with amber and mint accents |
| Paper | Warm light surfaces with dark violet, rust and teal accents |
| Follow system | Terminal in dark mode; Paper in light mode; updates when system appearance changes |

Selecting a choice previews it throughout the workspace immediately. The dialog also contains representative action, selection, success and evidence states. **Apply theme** saves the choice. **Cancel**, the close button and **Escape** restore the previously saved choice. Keyboard focus stays within the dialog and returns to the opening control when it closes.

## Persistence

Preferences use the local storage key `thesaurus.theme.v1`. They survive restarts in the browser profile or desktop webview that saved them. Desktop and browser settings are independent; they do not modify workspace data, agent memory or provider credentials. Browser tabs at the same origin synchronize saved choices through storage events, unless a theme preview is open. That preview can still be applied or cancelled deliberately. Invalid or missing stored values fall back to Terminal. Storage errors are shown in the dialog, and Apply does not claim a preference was saved if storage rejected the write.

## Design

Every existing UI surface uses semantic CSS variables, including form fields, document viewers, notices, source rows, task tables, project focus, dialogs and the key bar. Selection has a distinct surface and an orange leading edge in action tables. Links, warning states, accepted work and successful processing retain their semantic accents. Theme changes preserve monospaced typography and keyboard navigation, with quiet dividers and restrained accents instead of competing panel frames.

The workspace fills the window. Its grouped sidebar, page title, Capture button, page sections and shortcut bar stay visible while the page content scrolls independently. Settings uses a single scrolling right pane: its title, tabs, preferences and Save area move together, without an inner form scroller. Navigation does not use a horizontal scroller; page sections wrap at smaller widths. Shorter windows reduce navigation spacing. Setup and the meeting companion keep their dedicated content layouts.

Sources, Memory, Activity, Connections, Meetings, Threads and Plugins separate everyday work from supporting tools through page sections. Switching sections preserves in-page form drafts; leaving the page follows the existing page lifecycle. Work emphasizes the action list and selected evidence, with workspace counts and recent context in a secondary column. Processing details and advanced model configuration are available through disclosure controls. Adding a tool should follow this hierarchy rather than prepend another full panel to the default view.

Settings uses the same terminal palette with bounded, gently grouped preference cards. Labels and short help text sit beside consistently sized controls; audio inputs and transcript choices are separate groups. Local model downloads, speech detection, hardware information and recording diagnostics live in expandable sections. Audio and automation forms show unsaved changes and a saved confirmation, and saving audio keeps Settings open. Opening audio preferences does not mount the recording companion or its session polling.

Built-in palettes are defined in `apps/ui/style.css`; preference validation, system appearance observation and the preview/apply lifecycle are in `apps/ui/src/themes.rs`. There is no theme download, external font request or model usage. Custom palette import and cross-device synchronization are outside this implementation.

## Acceptance evidence

The isolated browser walkthrough traversed 24 views in each built-in palette with no runtime exceptions. Preview/cancel/apply/reload, system transitions, keyboard focus, invalid-value fallback and injected storage-write failure/recovery passed. Representative semantic text contrast minima were 4.67:1 for Paper, 4.98:1 for Terminal, 4.82:1 for Midnight and 5.14:1 for Phosphor; these measurements do not constitute a complete accessibility audit. The resumed native walkthrough rendered 24 views/subviews; Paper preview/apply survived a full process quit/relaunch, and the original Terminal choice was restored. Native OS-wide appearance settings were not changed. See [verification](backlog-verification.md).

### Settings refinement (2026-10-27)

The shared Wasm check, formatting check, optimized web build and signed debug macOS bundle passed. All 18 generated web assets match the embedded native resources. The restarted native app rendered all four Settings tabs. Audio input/transcript grouping, expanded local model and recording disclosures, the fixed save area and WebKit switch rendering were checked visually. Changing the processing-interval draft from 20 to 21 enabled Save and showed “Unsaved changes”; restoring 20 returned to the clean state without saving. No recording, model request, download or saved preference change was made. Browser pairing was blocked by automatic approval review, so save/reload behavior in an isolated browser was not retested.

The follow-up scroll refinement replaces the audio form's inner scroller with one scroll surface for the entire Settings right pane, including the title, tabs and Save area. Other workspace pages retain their existing scroll layout. The optimized UI and signed native bundle passed, and all 18 embedded web assets match the distribution. Native Page Down and Home moved the full Settings pane between the expanded model list/Save area and the heading while the sidebar and shortcut bar stayed in place. Switching Settings tabs returns to the top. Mouse-wheel automation was unavailable (`noWindowsAvailable`), so the interactive scroll check used keyboard navigation. No preferences were changed.
