# Thesaurus

<!-- impeccable:product-schema 1 -->

## Platform

web

The primary interface is shared web UI, available through an authenticated local browser and a Tauri desktop shell. The initial installable product targets macOS on Apple Silicon. Native capture, windows, notifications and OS integrations support that shared interface; this is not a separate native mobile design language.

The browser and macOS app should present the same interface and behavior for shared workflows. Design changes must be reviewed in both hosts, accounting for native window and platform behavior, and desktop packages must contain the same built UI assets as the corresponding web release.

## Users

The primary user is an individual balancing meetings, project work and multiple AI agents. They need to recover relevant context, understand what has changed, and continue work across tools without repeatedly reconstructing the same background.

This audience and work situation were confirmed during product initialization. A narrower profession or customer segment has not been selected.

## Product Purpose

Thesaurus provides private work memory across projects and agents. It captures work context, keeps evidence retrievable, and helps the user review actions, decisions and contributions over time.

Meeting capture and accomplishment tracking support that central purpose. Bragbook is one built-in plugin, rather than the whole product.

Success means the user can find relevant evidence and carry useful, authorized context into their next task or agent session. No quantitative success target has been established.

## Positioning

Keep one persistent local work memory alongside the user's existing tools and agents. Source revisions, exact citations, reviewed records and scoped access provide the mechanism for continuity.

Connected agents retain their own conversations, authentication and model choices. Sharing memory with an agent and authorizing Thesaurus to use a model are separate decisions. These are product mechanisms, not claims of uniqueness or superiority over other products.

## Operating Context

- Work arrives through transcripts, recordings, notes, selected repository context and explicitly configured sources.
- The user captures or imports context, inspects cited suggestions, accepts or dismisses proposed work, and revisits project briefs, commitments and work threads.
- Existing agents can retrieve selected memory through MCP; CLI and local API access use the same service and policies.
- Bragbook collects scoped accomplishment candidates, supports personal review and edits, and exports selected accepted highlights.
- A persistent Rust service owns the workspace. The desktop, local browser and connected agents can use that same workspace independently.
- The existing stack is Rust, Leptos/WebAssembly, Tauri and SQLite. Build and runtime details belong in [development](docs/development.md).

## Capabilities and Constraints

- Useful local capture, extraction, search and review do not require a configured reasoning model. Model connections and external destinations are optional and explicit.
- Recording and source sharing require user control. Grants are scoped and revocable; source policy must be rechecked when information is delivered or exported.
- Evidence remains inspectable at its original source revision. A suggested task, accomplishment or model statement does not establish completion, personal contribution or measured impact.
- Configured, available and successfully verified are distinct states. Do not present executable discovery or saved credentials as proof that an account or capability works.
- Plugins and lifecycle hooks are confirmed product direction. Native integrations use first-party providers; executable third-party plugins are trusted local code, not a security sandbox.
- Workspace data is local SQLite without application-level encryption. Do not equate local storage with encryption or claim the app prevents all data from leaving the device when external routes are enabled.
- The current release is a local alpha. Public distribution, fresh-Mac acceptance and real-device recording checks have remaining gates in [release readiness](docs/release-readiness.md).
- The project uses MIT licensing. The intended public repository is `kdemoya/thesaurus`; publishing or pushing requires the owner's review. An installable commercial offering is possible, but packaging, pricing and service commitments beyond the current alpha are undecided.
- Hosted multi-user collaboration and cross-device synchronization are not current shipping capabilities.

## Brand Commitments

The product name is **Thesaurus**. **Bragbook** names its accomplishment plugin. Existing app icon assets are in [apps/desktop/src-tauri/icons](apps/desktop/src-tauri/icons).

The [onboarding brief](docs/onboarding.md) records the owner's existing dot-letter/number direction for onboarding. Keep that commitment scoped to onboarding. The shared interface is described as **The Quiet Console** in [DESIGN.md](DESIGN.md); no broader rebrand has been established.

## Evidence on Hand

- [README](README.md): implemented capabilities and local entry points.
- [Implementation plan](docs/implementation-plan.md): earlier confirmed architectural and product decisions; distinguish implemented behavior from roadmap proposals.
- [Bragbook](docs/bragbook.md): collection/review/export rules and recorded functional and visual verification.
- [Onboarding](docs/onboarding.md): supported starting goals, readiness distinctions and existing accessibility behavior.
- [Release readiness](docs/release-readiness.md): actual local alpha verification and outstanding acceptance gates.
- [Synthetic fixtures](fixtures): demonstration material, not customer evidence.

No customer testimonials, adoption figures or quantified productivity claims have been supplied as approved proof. Synthetic checks must remain identified as synthetic.

## Product Principles

1. Preserve continuity across projects and agents while keeping the user in control of context sharing.
2. Make evidence and uncertainty inspectable before a suggestion becomes an accepted record or exported claim.
3. Keep core local workflows useful independently of an external model account.
4. Work with the user's existing tools and agents, preserving their separate permissions, identities and lifecycles.

## Open Decisions

- A narrower initial customer segment, measurable activation/retention targets and a validated market differentiation claim.
- Commercial packaging, pricing and support commitments.
- A formal accessibility conformance target or specialized audience requirements beyond existing implementation behavior.
