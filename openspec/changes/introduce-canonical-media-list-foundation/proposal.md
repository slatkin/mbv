## Why

Issue #641 found destination-sized TuiRealm components duplicating list state, painting, and scrolling. This foundation is the first independently reviewed slice stacked on PR #606's `feat/migrate-tui-to-tuirealm` branch (which remains blocked).

## What Changes

Add provider-neutral row and viewport vocabulary and two embedded plain TuiRealm controls: `WideMediaList` (fixed-height, one-column Hero-on-left rail mechanics) and `InlineMediaBrowser` (single-column selected-row replacement). Within `BrowserComponent`, keep the bespoke Wide painters — `render_letter_grouped_rows` and `render_plain_rows`, both reached through `render_generic_movies_home_video_rows_with_ctx` — only for the unchanged non-hero two-column policy, and extend `WideMediaList` to absorb letter grouping so applicable Wide paths need neither; paths owned by later slices remain untouched. Compose persistent `WideMediaList` and `InlineMediaBrowser` controls into the applicable Wide and Narrow paths for hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, the Emby podcast channel list, and TV Series browsing. Preserve non-hero two-column policy, provider workspaces/images/effects, shell authority, keyboard routing, and typed parent translation.

Mouse is out of scope for this slice. `restore-mouse-support` (#638) lands after every canonical slice and adds `HitRegions<Target>` and delivery to the landed controls. No registry identity, second router, independent subscription, callback/provider framework, or effects are introduced. `Target` is stable and opaque to the control. Queue progress is a bounded prepared presentation value only; Queue adoption is a later slice.

## Scope and sequencing

This is planning for one slice, not implementation. It has no mouse dependency (`restore-mouse-support` #638 lands after all canonical slices); it depends on the independent Feed prerequisite where applicable, and targets `feat/migrate-tui-to-tuirealm`; it must remain a distinct PR/rollback boundary from PR #606 and other slices. Split `browser.rs` and `tv_workspace.rs` ownership-preservingly before or with wiring and enforce the 800-line gate.

Before replacing TV handoff, record the current TV Wide→Narrow→Wide selection, cursor, scroll, and selected-row screen offset. Implementation, representative stateful and rendered tests, automated gates, review, and acceptance form one uninterrupted slice; there is no pre-test visual-approval checkpoint. Live Wide/Narrow review remains required, and any defect found there is fixed as a bug before the affected tests and gates are rerun. Do not claim visual verification from screenshots or inference.

## Impact

No Service, Player, queue, protocol, persistence, configuration, dependency, or provider API change. Planned files are under `src/app/components/`, `src/app/render/`, shell adapters, focused characterization/render tests, and ownership-preserving splits. `CONTEXT.md` terminology is established before code. Tasks invalidated by the Browser ownership audit are reopened, while completed foundation and Wide TV work remains recorded.
