> Status: complete (2026-09-01), retained as historical context for the canonical media-list umbrella. User live visual acceptance already occurred (see the Acceptance note in `tasks.md`). The deferred two-space list-row indent ownership passes to the canonical Home/Feeds source-of-truth slice.

## Context

The Feeds Service/tab is orchestrated by `src/app/render/components/feeds.rs::render_feeds_content`, which selects the arrangement, decides column count, computes each row/cell rectangle, paints the hero detail pane and rail frame, and draws edge selection/active markers. `src/app/render/components/feed_row.rs::render_feed_entry_cell` only paints inside a single cell rectangle it is handed. Scout evidence identifies: W1 `library_column_count` creates two columns at the 82-width threshold; W2 the right rail lacks the existing semantic surface/backdrop/hero-on-left treatment; W3 selected rows are malformed through title suppression, two-column cell-background drift, duplicated hero title, and markers spanning multi-column rows. The Emby homevideos feed view is out of scope.

## Goals / Non-Goals

**Goals:** one-column Wide Feeds Service rows, established rail framing, stable selected/active/played marker and background geometry, and non-vacuous automated evidence.

**Non-Goals:** changing Narrow layout, feed fetching/model semantics, group expansion, Emby homevideos feed-view rendering, mouse/key routing, or canonical-list implementation itself.

## Decisions

### D1 — Fix the Wide policy at the owning render boundary

`feeds.rs::render_feeds_content` owns the Wide column policy. It reads the Wide arrangement from `hero_left::shared_hero_presentation` and sets `cols = 1` for the Wide rail instead of `library_column_count(list_area.width)`, which stays in use for non-hero catalog surfaces. `render_feeds_content` also computes each cell's `x`/width and the per-row map. `feed_row.rs::render_feed_entry_cell` never chooses a column count; it paints only within the cell rectangle it is given. The arrangement owns breakpoint selection.

### D2 — Reuse semantic rail treatment

Use the same semantic surface/backdrop roles and border/framing policy used by Music/TV Hero-on-left right rails. No raw colors or screen-owned painter override is introduced. The Feeds parent supplies content; the arrangement supplies the rail rectangle.

### D3 — One row is one selectable target

Wide Feeds rows have one full-width target rectangle. Ownership splits:

- `feeds.rs::render_feeds_content` decides whether the row title shows (it passes `show_title = !selected || wide`, so the Wide rail always keeps its row title), paints the hero/detail-pane title exactly once via `paint_hero_content`, and draws the edge selection/active markers through `list_rows::draw_column_selection_markers_with_background`. It must not emit a second-column cell in the Wide rail (`cols = 1`) or offset markers as if rows span multiple columns.
- `feed_row.rs::render_feed_entry_cell` must honor the caller's `show_title` without extra suppression, fill the full `cell_w` with one contiguous background, and paint only the within-row watched `✓` — never a selection marker.

Selected-row rendering is tested with metadata-bearing entries and repeated titles to distinguish title duplication from ordinary content.

### D4 — Preserve Narrow unless evidence requires a correction

No Narrow behavior is intentionally changed. Tests cover the existing Narrow fixture as a regression guard; any necessary Narrow adjustment requires a failing test and an explicit scope note before implementation.

### D5 — Verification before composition

Add focused render/buffer tests at width 82 (threshold) and a larger Wide width. Fixtures include enough FeedEntries and metadata to exercise selected, played, and active states, with a first visible heading and a final visible FeedEntry/marker at both widths. Assert one-column x geometry, rail background/border cells, exactly one selected title, marker alignment, and that the semantic border/background never overwrites the first visible heading or the final visible FeedEntry/marker at the 82-column threshold or a larger Wide width. The tests must inspect rendered geometry/output rather than only construct models.

This change was delivered test-first (fixtures written alongside the render correction, with live visual confirmation recorded at acceptance). That ordering predates the visual-first rule now in force and MUST NOT be copied into future canonical Feeds work — see the historical ordering note in `tasks.md`.

### D6 — Stacked delivery

Implement as an independent PR/change targeting `feat/migrate-tui-to-tuirealm`, before the Home/Feeds canonical-list slice and without folding #634/#637 or changing PR #606's merge rule. Keep source files under 800 lines; split only if implementation makes a file exceed the cap.

## Risks / Trade-offs

- Existing shared row helpers may serve non-hero two-column surfaces; constrain the change at the Feeds Wide call site and add a regression assertion for unrelated policy.
- A framing mismatch could be hidden by blank fixtures; metadata/state-bearing fixtures and threshold captures make it observable.
- Narrow drift is possible if shared helpers are modified; prefer a Feeds Wide-specific policy or prove Narrow equivalence with tests.
