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

The active variant alone owns live cursor/scroll. At a breakpoint transition the parent passes `ViewportAnchor { selected_target, selected_row_offset }`, where offset is the zero-based screen-row offset from viewport top to the selected ordinary row. The receiving control preserves it where possible and clamps it otherwise. Ordinary content refresh preserves target and locally clamps; persisted resting position remains shell-owned and is written only at navigation events. Canonical Browser input excludes cursor/scroll by construction, achieved by splitting the content channel from the position channel rather than by mutating the shared `LibraryListRenderCtx`. A position-free `BrowserContent` is the only input of `BrowserComponent::set_content`, so an ordinary content push has no field in which stale position could travel; merely retaining fields and ignoring them is not sufficient. Position re-seeding is a separate explicit push (`apply_position`), gated on a change of browse identity — nav-stack depth, level `parent_id`, `letter_filter`, `sort_by`/`sort_order`, `unplayed_only`, and the selected feed/home-video group. Within one identity nothing crosses, which is what makes pagination, loading completion, refresh, and the component's own cursor echo safe, and which closes the per-frame `feed_home_video.{video_cursor,video_scroll}` channel. `ViewportAnchor` remains the distinct breakpoint/target seam and is not merged into `apply_position`. Persisted resting position remains shell-owned and is written only at navigation events and teardown; that write-back is sanctioned, not a mirror. Any position input retained for the non-hero two-column carve-out is reconstituted inside the component from control-owned cursor/scroll at a single private site and cannot receive shell position. Representative stateful evidence covers target and offset across same-destination Browser and TV cross-destination Wide→Narrow→Wide transitions and proves an ordinary content push does not adopt stale shell cursor/scroll.

### D4 — Mouse is out of scope for this slice

This slice adds no mouse subscription, `MouseGestureState`, `HitRegions<Target>`, or parent-to-child point delegation, and it does not touch the existing bespoke `*HitRegion` paths, which stay wired and untouched. `restore-mouse-support` (#638) lands after every canonical slice and owns all mouse work, including adding `HitRegions<Target>` to `WideMediaList`/`InlineMediaBrowser` and the per-surface row-hit migration. Keyboard precedence remains solely in `router.rs`/`key_policy.rs`.

### D5 — Composition and visual proof

The slice composes persistent `WideMediaList` and `InlineMediaBrowser` controls into every applicable Wide and Narrow Browser path for hero-bearing generic Emby catalogs, Movies, the Emby homevideos feed view, the Emby podcast channel list, and TV Series browsing. Non-hero two-column Emby catalogs keep their existing two-column arrangement policy. Within the foundation's Browser destinations, `render_generic_movies_home_video_rows_with_ctx` and both painters it routes to — `render_letter_grouped_rows` and `render_plain_rows` — remain only for that legacy policy and are unreachable from an applicable Wide rail; paths owned by later slices remain untouched. `WideMediaList` absorbs letter grouping through `MediaListRow::Heading`/`Spacer` so no applicable Wide path needs a second grouped painter. The implementation deletes per-frame canonical-control construction and independent Narrow replacement geometry. Representative tests, automated gates, source-level one-painter evidence, live Wide/Narrow review, and acceptance run as one uninterrupted slice; visual defects found during review are fixed as bugs before rerunning affected gates.

### D6 — File and branch gates

Split `src/app/components/browser.rs` and `tv_workspace.rs` before or with wiring while preserving ownership; no source file exceeds 800 lines. This PR stacks on `feat/migrate-tui-to-tuirealm`, stays separate from PR #606 and sibling slices, and is independently reversible. The invalid Home/Feeds commit chain and worktree remain unaccepted evidence; correction starts from accepted baseline `d426e057` or an equivalent isolation that excludes those edits.

### D7 — `LibraryListRenderCtx` field stripping is deferred, not done here

`LibraryListRenderCtx` is not Browser-only: it is embedded in `MusicWideRenderCtx` and `TvWideRenderCtx`, and its `cursor`-reading helpers (`selected_item`, `with_cursor_scroll`) resolve playback targets for TV, Music, `detail.rs`, and inline search. Removing `cursor`/`scroll` from it would cascade across roughly nineteen non-test files, including Music painters owned by `migrate-music-audiobookshelf-to-canonical-lists`, and would break this slice's independent-reviewability and reversibility gate. The field strip is therefore deferred until Music and TV have both migrated to canonical controls; it is not a task of this change and its absence is not evidence of a missing invariant, because the invariant is enforced by `BrowserContent` instead. The non-hero two-column carve-out, TV, Music, `detail.rs`, and inline search keep the existing type unchanged.

## Risks and mitigations

- Selection/scroll jumps: characterization plus target/offset anchor.
- Structural rows become selectable: separate display rows from selectable index.
- Queue API leaks authority: bounded prepared percentage only.
- Duplicate painters or geometry: source evidence proves applicable Wide paths cannot reach `render_generic_movies_home_video_rows_with_ctx` (and therefore neither grouped nor plain bespoke painter) and Narrow paths do not construct controls or rebuild replacement geometry. Absence of one named symbol is not accepted as this evidence.
- Refresh restores stale shell state: `BrowserContent` has no position field, and a stateful test proves ordinary content pushes at an unchanged browse identity preserve the control-owned target and scroll.
- Browse identity under-specified, so a navigation event fails to re-seed: the identity enumerates depth, `parent_id`, `letter_filter`, `sort_by`/`sort_order`, `unplayed_only`, and feed group, and task 4.1(c) tests drill-in, go-back parent restore, letter-filter reset, saved-position restore, and feed group switching.
- Vacuous tests: fixtures must contain metadata, grouping, focus, progress, and images relevant to the path.
