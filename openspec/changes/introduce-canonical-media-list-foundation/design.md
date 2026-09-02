## Context

The TuiRealm migration has destination parents but list state and geometry remain duplicated. The audit confirmed that the shared controls and `ListCore` are real, Wide TV genuinely composes `WideMediaList`, and Queue later proves the same pattern. However, applicable Browser paths retained parent-owned cursor/scroll, reached the bespoke Wide painters through `render_generic_movies_home_video_rows_with_ctx` (which routes to `render_letter_grouped_rows` for any library at or above 50 items or with a letter filter, and to `render_plain_rows` otherwise), and constructed an `InlineMediaBrowser` while painting at Narrow. The target composition is:

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

The active variant alone owns live cursor/scroll. At a breakpoint transition the parent passes `ViewportAnchor { selected_target, selected_row_offset }`, where offset is the zero-based screen-row offset from viewport top to the selected ordinary row. The receiving control preserves it where possible and clamps it otherwise. Ordinary content refresh preserves target and locally clamps; persisted resting position remains shell-owned and is written only at navigation events. Canonical Browser input excludes cursor/scroll by construction; merely retaining those fields and ignoring them is not sufficient. Any position input retained for the non-hero two-column carve-out is isolated to that path and cannot reach either canonical control. Representative stateful evidence covers target and offset across same-destination Browser and TV cross-destination Wide→Narrow→Wide transitions and proves an ordinary content push does not adopt stale shell cursor/scroll.

### D4 — Mouse is out of scope for this slice

This slice adds no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation, and it does not touch the existing bespoke `*HitRegion` paths, which stay wired and untouched. `restore-mouse-support` (#638) lands after every canonical slice and owns all mouse work, including adding `HitRegions<Target>` to `WideMediaList`/`InlineMediaBrowser` and the per-surface row-hit migration. Keyboard precedence remains solely in `router.rs`/`key_policy.rs`.

### D5 — Composition and visual proof

The slice composes persistent `WideMediaList` and `InlineMediaBrowser` controls into every applicable Wide and Narrow Browser path for hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, the Emby podcast channel list, and TV Series browsing. Non-hero two-column Emby catalogs keep their existing two-column arrangement policy. Within the foundation's Browser destinations, `render_generic_movies_home_video_rows_with_ctx` and both painters it routes to — `render_letter_grouped_rows` and `render_plain_rows` — remain only for that legacy policy and are unreachable from an applicable Wide rail; paths owned by later slices remain untouched. `WideMediaList` absorbs letter grouping through `MediaListRow::Heading`/`Spacer` so no applicable Wide path needs a second grouped painter. The implementation deletes per-frame canonical-control construction and independent Narrow replacement geometry. Representative tests, automated gates, source-level one-painter evidence, live Wide/Narrow review, and acceptance run as one uninterrupted slice; visual defects found during review are fixed as bugs before rerunning affected gates.

### D6 — File and branch gates

Split `src/app/components/browser.rs` and `tv_workspace.rs` before or with wiring while preserving ownership; no source file exceeds 800 lines. This PR stacks on `feat/migrate-tui-to-tuirealm`, stays separate from PR #606 and sibling slices, and is independently reversible. The invalid Home/Feeds commit chain and worktree remain unaccepted evidence; correction starts from accepted baseline `d426e057` or an equivalent isolation that excludes those edits.

## Risks and mitigations

- Selection/scroll jumps: characterization plus target/offset anchor.
- Structural rows become selectable: separate display rows from selectable index.
- Queue API leaks authority: bounded prepared percentage only.
- Duplicate painters or geometry: source evidence proves applicable Wide paths cannot reach `render_generic_movies_home_video_rows_with_ctx` (and therefore neither grouped nor plain bespoke painter) and Narrow paths do not construct controls or rebuild replacement geometry. Absence of one named symbol is not accepted as this evidence.
- Refresh restores stale shell state: a stateful test proves ordinary content pushes preserve the control-owned target and scroll.
- Vacuous tests: fixtures must contain metadata, grouping, focus, progress, and images relevant to the path.
