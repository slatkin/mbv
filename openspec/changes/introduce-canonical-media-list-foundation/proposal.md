## Why

Issue #641 found destination-sized TuiRealm components duplicating list state, painting, and scrolling. This foundation is the first independently reviewed slice stacked on PR #606's `feat/migrate-tui-to-tuirealm` branch (which remains blocked).

## What Changes

Add provider-neutral row and viewport vocabulary and two embedded plain TuiRealm controls: `WideMediaList` (fixed-height, one-column Hero-on-left rail mechanics) and `InlineMediaBrowser` (single-column selected-row replacement). Re-home the existing `render_plain_rows` and its working generic Emby/TV behaviour rather than rewriting it. Compose the controls into hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, the Emby podcast channel list, narrow TV, and the Wide TV right rail. Non-hero two-column Emby catalogs keep their existing two-column arrangement policy and are not migrated by this slice. Preserve non-hero two-column policy, provider workspaces/images/effects, shell authority, keyboard routing, and typed parent translation.

Mouse is out of scope for this slice. `restore-mouse-support` (#638) lands after every canonical slice and adds `HitRegions<Target>` and delivery to the landed controls. No registry identity, second router, independent subscription, callback/provider framework, or effects are introduced. `Target` is stable and opaque to the control. Queue progress is a bounded prepared presentation value only; Queue adoption is a later slice.

## Scope and sequencing

This is planning for one slice, not implementation. It has no mouse dependency (`restore-mouse-support` #638 lands after all canonical slices); it depends on the independent Feed prerequisite where applicable, and targets `feat/migrate-tui-to-tuirealm`; it must remain a distinct PR/rollback boundary from PR #606 and other slices. Split `browser.rs` and `tv_workspace.rs` ownership-preservingly before or with wiring and enforce the 800-line gate.

Before replacing TV handoff, characterize current TV Wide→Narrow→Wide selection, cursor, scroll, and selected-row screen offset by source-reading and manual observation of the running app only — adding no test or fixture. Implementation is visual-first: visual correction and user live confirmation precede any UI test or fixture modification or addition, including the metadata-bearing characterization fixture. After confirmation, add/update focused tests and rendered evidence. Do not claim visual verification from screenshots or inference.

## Impact

No Service, Player, queue, protocol, persistence, configuration, dependency, or provider API change. Planned files are under `src/app/components/`, `src/app/render/`, shell adapters, focused characterization/render tests, and ownership-preserving splits. `CONTEXT.md` terminology is established before code; umbrella and task checkboxes are untouched by this slice.
