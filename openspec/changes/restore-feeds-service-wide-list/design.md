## Context

The Feeds Service/tab uses `src/app/render/components/feeds.rs::render_feeds_content` and `feed_row.rs`. Scout evidence identifies: W1 `library_column_count` creates two columns at the 82-width threshold; W2 the right rail lacks the existing semantic surface/backdrop/hero-on-left treatment; W3 selected rows are malformed through title suppression, two-column cell-background drift, duplicated hero title, and markers spanning multi-column rows. The Emby homevideos feed view is out of scope.

## Goals / Non-Goals

**Goals:** one-column Wide Feeds Service rows, established rail framing, stable selected/active/played marker and background geometry, and non-vacuous automated evidence.

**Non-Goals:** changing Narrow layout, feed fetching/model semantics, group expansion, Emby homevideos feed-view rendering, mouse/key routing, or canonical-list implementation itself.

## Decisions

### D1 — Fix the Wide policy at the owning render boundary

`render_feeds_content` selects the Wide arrangement and passes a one-column policy to the existing row painter/geometry. It must not derive columns from `library_column_count`; that helper remains available to unrelated non-hero catalogs. The arrangement owns breakpoint selection, while the Feed render component owns row geometry and painting.

### D2 — Reuse semantic rail treatment

Use the same semantic surface/backdrop roles and border/framing policy used by Music/TV Hero-on-left right rails. No raw colors or screen-owned painter override is introduced. The Feeds parent supplies content; the arrangement supplies the rail rectangle.

### D3 — One row is one selectable target

Wide Feeds rows have one full-width target rectangle. `feed_row.rs` must not suppress the selected title merely because a hero/title is present, emit a second title, or paint selection/played/active markers against a column cell. Selected-row rendering is tested with metadata-bearing entries and repeated titles to distinguish title duplication from ordinary content.

### D4 — Preserve Narrow unless evidence requires a correction

No Narrow behavior is intentionally changed. Tests cover the existing Narrow fixture as a regression guard; any necessary Narrow adjustment requires a failing test and an explicit scope note before implementation.

### D5 — Verification before composition

Add focused render/buffer tests at width 82 (threshold) and a larger Wide width. Fixtures include enough FeedEntries and metadata to exercise selected, played, and active states. Assert one-column x geometry, rail background/border cells, exactly one selected title, and marker alignment. The tests must inspect rendered geometry/output rather than only construct models.

### D6 — Stacked delivery

Implement as an independent PR/change targeting `feat/migrate-tui-to-tuirealm`, before the Home/Feeds canonical-list slice and without folding #634/#637 or changing PR #606's merge rule. Keep source files under 800 lines; split only if implementation makes a file exceed the cap.

## Risks / Trade-offs

- Existing shared row helpers may serve non-hero two-column surfaces; constrain the change at the Feeds Wide call site and add a regression assertion for unrelated policy.
- A framing mismatch could be hidden by blank fixtures; metadata/state-bearing fixtures and threshold captures make it observable.
- Narrow drift is possible if shared helpers are modified; prefer a Feeds Wide-specific policy or prove Narrow equivalence with tests.
