> Status: complete (2026-09-01), retained as historical context for the
> canonical media-list campaign. User acceptance is recorded in `tasks.md`. The
> test-first task ordering is historical fact and MUST NOT be copied into new
> canonical UI work, which is visual-first — see the historical prerequisite
> note in `tasks.md`.

## Context

The Normal-geometry Emby homevideos feed view group picker painter lost the
expansion that `fbc6888e:src/app/render/components/home_feed.rs:118-127`
computed: `render_feed_group_picker_content` (`list_narrow.rs:334+`) has
`let selected_h = 1;`, so its own `render_compact_detail_with_ctx` call —
already present, with `y: row + 3`, `height: h.saturating_sub(5)` and
`SELECTED_BLOCK_SIDE_PADDING` inset — is rejected by the banner painter's
zero-height guard every frame. The shell computes the banner in
`narrow_browse_extras` and pushes it as `NarrowInlineHero::Movie`, so the
height input is already in hand.

The Wide-geometry picker is out of scope for this change: it removes the
bespoke `render_wide_feed_layer` so Wide falls through to the shared
Hero-on-left path, and `compose-canonical-media-lists` (slice 2) owns the
substantive Wide list control.

**As delivered.** The in-place approach recorded in D1–D4 and tasks 3.1–3.4
(patch `selected_h` inside `render_feed_group_picker_content`, port the scroll
clamp into that function) was the first landed fix (`27b87423`). It was then
SUPERSEDED by `051bf75a`, which deleted `render_feed_group_picker_content` and
`render_wide_feed_layer` entirely (~325 lines) and routed the Emby homevideos
feed view picker through the shared paths: narrow via
`render_narrow_browse_with_ctx` → `render_plain_rows` /
`render_letter_grouped_rows`, Wide via `render_wide_movies` (Hero-on-left).
Only `paint_feed_group_pills_row` and the `feed_selected_height` field
(consumed in `render_narrow_browse_with_ctx`) survive from the first approach.
D1–D4 and tasks 3.x are retained as the record of that first attempt, not the
end state.

One divergence is already live and is out of scope here: the migrated picker
paints a ` N items` + `▁` header that legacy
`render_feed_home_video_group_view` never had (`b029fec3`), shifting every row
down by two — visible by comparing `9bc9bd29`'s narrow capture with the
constant as checked in today. It stays; baselines regenerate around it.

Constraint: `compact_banner_layout_with_overview` is `&mut self` on `App`
(image-cache lookup + fetch trigger), so the layout must stay shell-resolved;
the render components are `App`-free by the TUI boundary and must not fetch
while painting.

## Goals / Non-Goals

**Goals:**
- One expression for the expanded selected-row height at Normal geometry:
  `banner.content_rows() + 5`.
- Restore legacy's accumulated-height scroll clamp for this surface.
- Baselines that actually exercise the expansion path.

**Non-Goals:**
- No change to the generic (non-feed-group) narrow inline hero path, which
  already sizes and paints correctly.
- No new fetch, no ctrl protocol, no key routing.

## Decisions

**D1 — Publish the height from the shell, do not recompute it in the painter.**
Add `feed_selected_height: u16` to `NarrowBrowseExtras`, computed once in
`App::narrow_browse_extras` for the Normal-geometry picker as
`banner.content_rows() + 5` from the same `CompactBannerLayout` already built
for `NarrowInlineHero::Movie`, using the picker's own panel width. It is not
consumed at Wide geometry.
Alternative considered: `content_rows: u16` inside
`NarrowInlineHero::Movie`. Rejected — the generic narrow path derives its rows
with `content_rows_with_title(HERO_TITLE_ROWS * …) + HERO_BLOCK_EXTRA_ROWS`
and shares that variant, so stuffing a picker-specific number there invites a
second divergence. A plain extras field keeps the picker's budget explicit and
leaves the shared variant alone.

**D2 — Keep the published picker height explicit.**
The picker keeps its content-derived height in `NarrowBrowseExtras`; the
component does not recompute banner layout while painting.

**D3 — Port the legacy scroll clamp verbatim.**
`render_feed_group_picker_content` replaces
`if selected_h > list_area.height { offset = selected; }` with legacy's loop:
seed `offset = min(stored_scroll, last)`, clamp up to the cursor, then advance
while the accumulated heights from `offset..=selected` exceed the viewport.
Because only the selected row is tall here, `item_heights` collapses to
`1 + (idx == selected) * (feed_selected_height - 1)` — no allocation needed.
The landed offset must be returned so `BrowserComponent` records it as its
scroll (the component owns scroll; the shell projects it into
`FeedHomeVideoState::video_scroll` via the existing Library-position path in
`types_library_tab.rs`), which is what makes the clamp sticky rather than
recomputed per frame.

**D4 — Drop the unconditional trailer block.**
The `▔`-row + last-item `Paragraph` at the end of
`render_feed_group_picker_content` (added by `1e18e55b`) duplicates the framed
block's own bottom border and re-paints the last item's title — the "stray
trailing border/divider echo" in #634. This is the trailer at the end of the
row loop, not the ` N items` + `▁` header (see Context: that one stays).
`render_home_video_item` already paints the `▁`/`▔` framing for an expanded
row. Remove the trailer rather than conditionally gating it: with a correct
expansion there is no legacy behaviour left to reproduce.

**D5 — Baselines: metadata-bearing as primary, metadata-free as degenerate pin.**
`feed_home_video_group_app()` gains runtime_ticks, genre, and a multi-line
overview on the selected item, and the feed fixtures are exercised at
60x20 through `Model::draw_frame` — capture from the fixed implementation,
and cross-check the *shape* (framed expansion + banner lines present) against
`fbc6888e` narrow output. Keep a metadata-free fixture asserting one row per
item and no border echo, so
the degenerate case that hid the bug stays pinned. Add the tall-selected-row
scroll test from D3 (long list, cursor near the bottom).

## Risks / Trade-offs

- [`feed_selected_height` uses the picker's panel width while
  `NarrowInlineHero::Movie` is built with `layout.main.left_area.width`] →
  Both derive from the same rect; add a test that the published height equals
  `content_rows() + 5` for the fixture so a future width change surfaces as a
  diff rather than a silently clipped banner.
- [Removing the trailer may break `e2da2e0b`-era expectations] → It is pinned
  by the metadata-free baseline pair; regenerate in the same commit, never
  split.
- [The image-cache-dependent `img_height` in `content_rows()` can change
  between frames while a poster loads] → Existing behaviour on the generic
  path (legacy had it too); the row grows once when the image lands. Record
  it, do not design around it.
