## Context

The TuiRealm migration has destination parents but list state and geometry remain duplicated. `render_plain_rows` is the proven generic Emby/TV fixed-row path. The target composition is:

```text
shell Model -> mounted destination AppComponent
  -> embedded plain WideMediaList or InlineMediaBrowser
    -> render component
```

The parent supplies prepared content and receives typed intents; it retains Service, Player, image, workspace, persistence, and effect authority.

## Decisions

### D1 — Two embedded controls, not a framework

`WideMediaList<Target>` owns fixed row placement, selectable indexing, cursor, scroll, viewport, scrollbar, semantic row rendering, and internal row geometry for painting and scrolling (no mouse hit-resolution API; `restore-mouse-support` adds that later). It is one-column and fixed-height; it is used by Hero-on-left rails and may later serve Queue fixed rows. It does not accept column-count or Inline detail options. `InlineMediaBrowser<Target>` owns the same list mechanics plus selected-row replacement, variable-height admission, fallback to the ordinary row, and replacement paint geometry. Shared private helpers are allowed; no third public widget abstraction is.

### D2 — Provider-neutral prepared model

The public model is a small closed vocabulary: selectable `Item { target, primary, trailing, semantic_state }`, non-selectable `Heading { text }`, and `Spacer`. Semantic state includes ordinary, played, active with optional integer `0..=100` progress, and disabled. Targets are stable, cloneable, opaque parent identities. Queue supplies only a bounded percentage; no ticks, runtime, source, credentials, callbacks, raw styles, or provider effects cross the boundary.

### D3 — Responsive handoff

The active variant alone owns live cursor/scroll. At a breakpoint transition the parent passes `ViewportAnchor { selected_target, selected_row_offset }`, where offset is the zero-based screen-row offset from viewport top to the selected ordinary row. The receiving control preserves it where possible and clamps it otherwise. Ordinary content refresh preserves target and locally clamps; persisted resting position remains shell-owned and is written only at navigation events. Characterize current TV behaviour before replacement by source-reading and manual observation only, adding no test or fixture, and match it absent an approved correction. The metadata-bearing characterization fixture is added only after user live visual approval (tasks 4.1/4.2).

### D4 — Mouse is out of scope for this slice

This slice adds no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation, and it does not touch the existing bespoke `*HitRegion` paths, which stay wired and untouched. `restore-mouse-support` (#638) lands after every canonical slice and owns all mouse work, including adding `HitRegions<Target>` to `WideMediaList`/`InlineMediaBrowser` and the per-surface row-hit migration. Keyboard precedence remains solely in `router.rs`/`key_policy.rs`.

### D5 — Composition and visual proof

The slice composes hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, the Emby podcast channel list, narrow TV Series browsing, and Wide TV's right rail. Non-hero two-column Emby catalogs keep their existing two-column arrangement policy and are not forced onto `InlineMediaBrowser`; non-hero two-column browsers remain unchanged. `render_plain_rows` is re-homed/parameterized first with characterization output preserved. Visual correction and user live confirmation happen before any UI test or fixture changes; then focused rendered tests and representative metadata/state fixtures are added. Each migrated surface provides one-painter evidence and Wide/Narrow manual evidence. No bespoke exception is permitted without an umbrella design update.

### D6 — File and branch gates

Split `src/app/components/browser.rs` and `tv_workspace.rs` before or with wiring while preserving ownership; no source file exceeds 800 lines. This PR stacks on `feat/migrate-tui-to-tuirealm`, stays separate from PR #606 and sibling slices, and is independently reversible.

## Risks and mitigations

- Selection/scroll jumps: characterization plus target/offset anchor.
- Structural rows become selectable: separate display rows from selectable index.
- Queue API leaks authority: bounded prepared percentage only.
- Duplicate painters or geometry: one-painter evidence and source search.
- Vacuous tests: fixtures must contain metadata, grouping, focus, progress, and images relevant to the path.
