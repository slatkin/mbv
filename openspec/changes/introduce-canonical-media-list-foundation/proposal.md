## Why

Issue #641 found destination-sized TuiRealm components duplicating list state, painting, scrolling, and hit geometry. This foundation is the first independently reviewed slice stacked on PR #606's `feat/migrate-tui-to-tuirealm` branch (which remains blocked).

## What Changes

Add provider-neutral row and viewport vocabulary and two embedded plain TuiRealm controls: `WideMediaList` (fixed-height, one-column Hero-on-left rail mechanics) and `InlineMediaBrowser` (single-column selected-row replacement). Re-home the existing `render_plain_rows` and its working generic Emby/TV behaviour rather than rewriting it. Compose the controls into generic Emby, Movies, Emby homevideos/podcast browsing, narrow TV, and the Wide TV right rail. Preserve non-hero two-column policy, provider workspaces/images/effects, shell authority, keyboard routing, and typed parent translation.

The mounted parent owns the mouse subscription and `MouseGestureState`; the embedded control owns view-populated `HitRegions<Target>`. No registry identity, second router, independent subscription, callback/provider framework, or effects are introduced. `Target` is stable and opaque to the control. Queue progress is a bounded prepared presentation value only; Queue adoption is a later slice.

## Scope and sequencing

This is planning for one slice, not implementation. It depends on the revised mouse contract and independent Feed prerequisite where applicable, and targets `feat/migrate-tui-to-tuirealm`; it must remain a distinct PR/rollback boundary from PR #606 and other slices. Split `browser.rs` and `tv_workspace.rs` ownership-preservingly before or with wiring and enforce the 800-line gate.

Before replacing TV handoff, characterize current TV Wide→Narrow→Wide selection, cursor, scroll, and selected-row screen offset. Implementation is visual-first: visual correction and user live confirmation precede any UI test modification or addition. After confirmation, add/update focused tests and rendered evidence. Do not claim visual verification from screenshots or inference.

## Impact

No Service, Player, queue, protocol, persistence, configuration, dependency, or provider API change. Planned files are under `src/app/components/`, `src/app/render/`, shell adapters, focused characterization/render tests, and ownership-preserving splits. `CONTEXT.md` terminology is established before code; umbrella and task checkboxes are untouched by this slice.
