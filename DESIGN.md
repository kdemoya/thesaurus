---
name: "Thesaurus"
description: "The Quiet Console: a calm, precise interface for private work memory."
colors:
  bg: "#1b1b27"
  ink: "#d2cfdb"
  muted: "#aaa6bb"
  dim: "#aaa6bb"
  line: "#50516c"
  row: "#282739"
  selected: "#38354d"
  orange: "#ffa064"
  violet: "#bb9af3"
  cyan: "#7dc4cf"
  green: "#a6c78b"
  on-accent: "#191824"
  surface: "#252435"
  recessed: "#181821"
  line-soft: "#37364d"
  hover: "#333145"
  notice-bg: "#332a29"
  overlay: "#10101acc"
  midnight-bg: "#101b29"
  midnight-ink: "#dae6f4"
  midnight-muted: "#a4b4ca"
  midnight-dim: "#9eb0ca"
  midnight-line: "#526783"
  midnight-line-soft: "#30445e"
  midnight-row: "#1b2a40"
  midnight-selected: "#293f60"
  midnight-orange: "#ffb579"
  midnight-violet: "#b4b6ff"
  midnight-cyan: "#8bdaed"
  midnight-green: "#a8d99b"
  midnight-on-accent: "#101b29"
  midnight-surface: "#19273c"
  midnight-recessed: "#0d1725"
  midnight-hover: "#2c3b54"
  midnight-notice-bg: "#342822"
  midnight-overlay: "#050c16cc"
  phosphor-bg: "#101b14"
  phosphor-ink: "#d8ebda"
  phosphor-muted: "#a3b9a6"
  phosphor-dim: "#a3b9a6"
  phosphor-line: "#54705b"
  phosphor-line-soft: "#344b3a"
  phosphor-row: "#1c2b20"
  phosphor-selected: "#2c4331"
  phosphor-orange: "#efbc76"
  phosphor-violet: "#b3d998"
  phosphor-cyan: "#8bd5bd"
  phosphor-green: "#b3d998"
  phosphor-on-accent: "#101b14"
  phosphor-surface: "#1b2b21"
  phosphor-recessed: "#0c150f"
  phosphor-hover: "#293c2e"
  phosphor-notice-bg: "#30291c"
  phosphor-overlay: "#050d07cc"
  paper-bg: "#f4f1e9"
  paper-ink: "#292b32"
  paper-muted: "#57596a"
  paper-dim: "#5e6070"
  paper-line: "#8a8995"
  paper-line-soft: "#d0cdd2"
  paper-row: "#e8e4df"
  paper-selected: "#d8e0ed"
  paper-orange: "#8d3b0e"
  paper-violet: "#644290"
  paper-cyan: "#075f70"
  paper-green: "#246a36"
  paper-on-accent: "#fffdf8"
  paper-surface: "#eae6df"
  paper-recessed: "#fffcf6"
  paper-hover: "#ddd8e2"
  paper-notice-bg: "#f7dfc7"
  paper-overlay: "#39354288"
typography:
  page-heading:
    fontFamily: "\"JetBrains Mono\", \"SFMono-Regular\", Consolas, monospace"
    fontSize: "20px"
    fontWeight: 400
    lineHeight: 1.3
    letterSpacing: "-0.6px"
  section-heading:
    fontFamily: "\"JetBrains Mono\", \"SFMono-Regular\", Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.55
  body-root:
    fontFamily: "\"JetBrains Mono\", \"SFMono-Regular\", Consolas, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.55
  control:
    fontFamily: "\"JetBrains Mono\", \"SFMono-Regular\", Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
  supporting:
    fontFamily: "\"JetBrains Mono\", \"SFMono-Regular\", Consolas, monospace"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.55
  metadata:
    fontFamily: "\"JetBrains Mono\", \"SFMono-Regular\", Consolas, monospace"
    fontSize: "10px"
    fontWeight: 400
    lineHeight: 1.55
rounded:
  section-flat: "0"
  compact-surface: "3px"
  control: "4px"
  settings-field: "5px"
  settings-card: "7px"
  settings-switch: "20px"
spacing:
  control-inset-block: "4px"
  form-label-gap: "6px"
  compact-content-inset: "8px"
  control-group-gap: "12px"
  section-heading-gap: "16px"
  section-top-inset: "18px"
  page-tab-gap: "20px"
  medium-page-gutter: "22px"
  toolbar-top-inset: "24px"
  page-gutter: "28px"
  work-column-gap: "32px"
components:
  button-default:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "4px 11px"
  button-primary:
    backgroundColor: "{colors.selected}"
    textColor: "{colors.ink}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "4px 11px"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.orange}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "4px 11px"
  button-compact:
    backgroundColor: "transparent"
    textColor: "{colors.orange}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "2px 10px"
  button-text:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "4px 11px"
  input-standard:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.section-flat}"
    padding: "6px 8px"
  nav-selected:
    backgroundColor: "{colors.selected}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "5px 10px"
  settings-badge:
    backgroundColor: "{colors.row}"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "3px 7px"
  panel-ruled:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.section-flat}"
    padding: "18px 0 0"
  source-excerpt:
    backgroundColor: "{colors.recessed}"
    textColor: "{colors.ink}"
    rounded: "{rounded.section-flat}"
    padding: "8px 12px"
  settings-card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.settings-card}"
---

# Design System: Thesaurus

## Overview

**Creative North Star: "The Quiet Console"**

Thesaurus is a quiet place to recover context and continue working. A persistent sidebar, monospaced text, restrained color and fine dividers give private work memory a stable, inspectable form. The user chose “The Quiet Console” as the system’s description; calm, precise and quietly technical are editorial descriptions of the existing interface.

The workspace is compact and flat. Headings establish place without competing with actions, metadata stays close to its subject, and source passages remain visibly connected to their citations. Settings use bounded groups; onboarding carries the more expressive dot lettering. These are distinct patterns within the same interface.

**Key Characteristics:**

- Persistent navigation and a stable workspace frame.
- One monospaced family, with hierarchy carried by size, spacing and tone.
- Flat tonal surfaces, fine rules and inset selection markers.
- Compact controls with explicit keyboard focus.
- Citations and source passages presented as part of the work.

This is a scan of the existing shared browser/Tauri interface, not a redesign. The authority is the effective cascade in [the base stylesheet](apps/ui/style.css), [workspace styling](apps/ui/workspace.css), and [mobile navigation](apps/ui/mobile-navigation.css), plus [workspace navigation](apps/ui/src/workspace.rs), [theme behavior](apps/ui/src/themes.rs), [settings](apps/ui/src/settings.rs) and [Bragbook](apps/ui/src/bragbook.rs). Preserved desktop and narrow Bragbook captures were compared with the current CSS; Midnight, Phosphor and Paper were extracted from code without a fresh rendered check. The native app opened to an empty workspace-connection window during this documentation pass, so it supplied no fresh workspace evidence.

The frontmatter is the normative snapshot of extracted values. Unprefixed colors are Terminal defaults and keep the CSS variable names without the leading dashes. The other palettes use theme prefixes for portability; the app still switches the original variables together. Typography, radius and spacing keys are documentation aliases for measured CSS values, not existing custom-property scales. Component tokens describe resting styles; minimum sizes, borders and states are specified below and in the sidecar specimens.

## Colors

The default palette pairs violet-charcoal surfaces with soft pale text, warm apricot attention and lavender accents. Descriptive color names explain the Terminal palette; the original variable names remain the implementation contract.

### Primary

- **Warm Apricot** (`orange`): brand text, keyboard focus, selection rules, review states and attention. Enabled settings Save controls and checked switches also use this fill with `on-accent` text or thumb.

### Secondary

- **Soft Lavender** (`violet`): accepted states, project accents, progress and annotated source lines. Text selection pairs this fill with `on-accent`.

### Tertiary

- **Source Cyan** (`cyan`): source-opening links and code accents.
- **Ready Sage** (`green`): local status, completion and model readiness.

### Neutral

- **Violet Charcoal** (`bg`), **Raised Charcoal** (`surface`) and **Recessed Ink** (`recessed`): the workspace, bounded groups and sidebar/source wells.
- **Pale Ink** (`ink`), **Muted Lilac** (`muted`) and **Dim Lilac** (`dim`): primary content, supporting text and metadata. Some themes intentionally share muted and dim values.
- **Fine Slate** (`line`) and **Quiet Divider** (`line-soft`): field borders and low-emphasis separators.
- `row`, `selected` and `hover`: context-specific interaction fills. `notice-bg` supports attention notices; `overlay` dims the workspace behind dialogs. `on-accent` supplies contrasting content on solid accent patches.

Terminal, Midnight, Phosphor and Paper share geometry. Midnight shifts toward blue; Phosphor toward green; Paper uses warm light surfaces and darker accents. Preserve separate semantic variables when values coincide, including Phosphor’s violet and green. Terminal is the default/fallback; Follow system resolves to Terminal in dark mode and Paper in light mode. Theme preview is reversible until Apply.

**The Role Before Hue Rule.** Use the existing theme variable for its implemented role. The variable named orange also carries focus, attention and review states; it is not an exclusive primary-action color.

The sidecar’s eight-step OKLCH ramps are synthesized swatch references for the design panel. They are not additional app colors or an implemented tonal scale; source hex values in the frontmatter remain authoritative.

## Typography

**Shared font:** JetBrains Mono, followed by SFMono-Regular, Consolas and monospace. One regular face is bundled with swap loading. Root text uses the `body-root` role; most operational copy is explicitly smaller. Headings and strong text generally retain regular weight, so hierarchy depends on spacing, size and color.

The frontmatter captures the current page heading, section heading, body default, standard control, supporting copy and metadata roles. Workspace page headings step down at the narrow breakpoint. Settings additionally request medium weight for selected headings, but no separate medium face is supplied and font synthesis is disabled.

Long passages receive more leading: action quotations (1.9), memory copy (1.8) and text areas (1.7). Bragbook prose and titles are bounded by a reading width (72ch); settings help uses a shorter measure (46ch). Source documents pair compact text (12px) with line rows (23px) and smaller line numbers (10px).

**The Scoped Expression Rule.** Keep the dot alphabet and its entrance animation in onboarding. Workspace headings and operational copy use the shared monospaced family.

Onboarding renders its own five-by-seven dot alphabet as decorative SVG, alongside semantic HTML text. Its brief settle animation lasts (440ms) with deterministic staggering up to (306ms). Reduced motion disables that animation and leaves the dots visible. No general workspace transition scale is implemented.

## Layout

The workspace occupies the viewport (100dvh), with a persistent sidebar (180px), a bottom key bar (32px), a fixed page toolbar and scrolling content. The outer shell is flush and unframed. Standard toolbar insets are (24px 28px 0); content insets are (26px 28px 32px), with direct children capped at (1400px). Settings instead use a single scrolling main surface for their heading, preferences and Save controls, with a content cap (900px).

The Work view places a flexible main area beside an aside (210px), separated by (32px). Brief and plugin views use a leading column (220px) with a gap (28px). Top page-section controls wrap naturally. Spacing follows these observed relationships; there is no implemented global mathematical spacing scale.

- At widths up to (1100px), the sidebar becomes (160px), horizontal page gutters become (22px), and secondary columns narrow.
- At widths up to (850px), Work stacks with a two-column aside beneath its main area; Brief and plugin views become one column.
- At widths up to (600px), the main workspace replaces the sidebar with a compact menu and navigation drawer. Keyboard hints hide, page gutters become (14px), page headings wrap and the Work aside becomes one column. Bragbook review stacks source evidence above the editor.
- Settings fields stack up to (760px); switch rows keep the label and switch beside each other. Onboarding has its own compact and stacked layouts at (850px) and (620px).
- Short windows compress the sidebar at heights up to (760px), then hide group labels and local status up to (570px). Some earlier narrow rules remain active up to (500px), including horizontally scrolling task tables and reduced dialog insets.

## Elevation & Depth

The current workspace has no ambient or drop-shadow elevation system. Recessed and filled tones, one-pixel dividers and a translucent dialog overlay establish separation. The existing box shadows are inset state markers: selected navigation/table rows (2px orange), cited source lines (3px orange), and annotations (2px violet). A cited line takes precedence over an annotation marker.

**The Flat Surface Rule.** Separate workspace regions with tone, spacing and fine rules. Keep the existing inset markers as state indicators rather than treating them as elevation shadows.

Dialogs use the workspace background, an orange border and a dimming overlay, without blur or an elevation shadow. General dialogs are bounded by width (650px) and viewport height (92dvh); document, keyboard-help and appearance dialogs have their own widths.

## Shapes

The form language is compact, lightly rounded and ruled. Ordinary workspace panels are square, transparent sections with a top divider. Shared action buttons have small corners; settings fields and connection cards are slightly softer; settings groups use the largest common card corners. The frontmatter records each measured radius separately rather than presenting them as an interchangeable scale.

Borders are generally fine (1px). Page-section selection uses an underline (2px), navigation and source states use leading rules, and settings switches use a small pill silhouette (34px × 20px). Keep these differences tied to their components.

## Components

### Buttons

Compact, explicit actions. Shared buttons use `control` type, the `control` radius and a minimum height (28px). Default controls are transparent with a line border. General primary controls use the selected surface and ink text; hover adds the hover surface with orange text and border. Secondary controls use orange text. Compact controls retain their more specific padding. Text controls use muted text and a transparent border, adding the row fill on hover.

Enabled buttons in the settings Save bar are a scoped exception: orange fill, contrasting `on-accent` text and a larger minimum height (32px). Disabled shared buttons use half opacity and a wait cursor; the existing hover selectors still apply. There is no added press animation.

### Inputs / Fields

Standard fields are square, surface-filled and line-bordered, with compact padding. Settings fields instead use the background tone, the settings-field radius and minimum height (32px). Placeholders remain muted at full opacity. Text areas resize vertically. Preserve associated labels and help. This extraction establishes no new invalid-field style.

### Navigation

The sidebar groups compact, left-aligned buttons. Selected items use the selected fill, ink text and an orange leading marker; unselected hover uses the row fill. Shortcut keys stay secondary and hide at narrow widths. The page-section selector uses a thin orange underline and pressed buttons in a labeled group; it is not an ARIA tablist.

### Cards / Containers

The ordinary panel has a soft top rule, transparent background, square corners and top breathing room. Its heading stays in normal flow, and focus within does not brighten the entire section. Settings groups use a filled, softly rounded card with dividing rows; connection cards use their own measured radius and inset. These bounded settings patterns do not replace ordinary workspace sections.

### Settings Badge / Switch

The small settings badge uses muted type (9px), a row fill and a soft border. It is a local informational pattern, not an established global filter-chip system. The checkbox-based switch has an associated label and help; checked state changes the fill and thumb position immediately, without a transition. Disabled switches use half opacity and a default cursor.

### Source Evidence

Cyan source-opening controls sit beside revision and line references. Their hover treatment is an underline; focus follows the shared outline. Bragbook quotations sit on a recessed surface with a fine leading rule. Entry titles and source passages wrap rather than truncate. Document inspection uses line numbers and distinct cited/annotated markers so the saved evidence remains locatable.

### Keyboard Focus / Dialogs

Interactive controls and summaries receive the shared orange focus outline (1px, offset 2px); document lines use an inset outline. Keep the implemented theme dialog’s focus trap, Escape/cancel behavior and focus restoration when reusing that pattern. These observations do not establish a completed accessibility audit or a universal target-size standard.

## Do's and Don'ts

### Do:

- Do use the active theme’s complete palette and retain distinct role tokens even when their values coincide.
- Do preserve the thin orange focus outline and the leading rule on selected navigation and cited lines.
- Do let Bragbook headings and source passages wrap; keep their established reading widths.
- Do use the transparent, top-ruled panel for ordinary workspace sections and the filled card for settings groups.
- Do retain visible labels, source references and the existing reduced-motion path.

### Don't:

- Don't restore the superseded outer terminal frame, horizontal main navigation or zebra table rows when extending the current workspace.
- Don't turn every primary action into an orange-filled button; that fill is a scoped settings Save treatment.
- Don't apply onboarding dot lettering or its animation to ordinary workspace headings.
- Don't treat the measurement aliases or synthesized preview ramps as implemented CSS scales.
- Don't describe the bundled font as having a supplied medium or bold face, or claim accessibility conformance from this extraction.
