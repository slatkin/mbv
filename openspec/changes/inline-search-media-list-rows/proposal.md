## Why

Inline search results are the last live consumer of the legacy row painters
(`render_generic_movies_home_video_rows_with_ctx` in
`src/app/render/components/list.rs`, down through `list_letter_groups.rs` and
`render/components/media_list/plain_rows.rs`). Every other list has migrated
to the shared `MediaList` carrier, so search result rows still paint with the
old full-bleed selected-row style and will read as the odd one out next to
the canonical selected-row bar and zebra striping used everywhere else
(GH #720). The letter-grouped branch reachable only through this path is
dead code that cannot be deleted while the shared function keeps this caller.

## What Changes

- Inline Search result rows render through the shared canonical
  `MediaListCarrier<String>` / fixed-row presentation, like every other
  library list: one-column fixed rows, canonical selected-row bar, zebra
  striping, shared theme roles, scrollbar policy.
- The search control keeps owning what is search-specific (query, candidate
  pool, scoring/debounce, open/dismiss, typed activation intents) and embeds
  the carrier as its results owner; the carrier owns the row flow (cursor,
  scroll, stable-target selection, viewport, retained geometry).
- Result rows become provider-neutral `MediaListRow::Item` rows with
  stable opaque targets (item ids), primary display labels (with the album
  index display-label substitution and the existing folder suffixes), and a
  trailing year for playable leaves — content parity with today's plain
  rows, styling from the media_list default.
- Selection semantics are preserved: a query re-score resets selection to
  the first result; a corpus/pool refresh with an unchanged query preserves
  the selected stable target (now the carrier's native behavior).
- Row-local pointer behavior (click, double-click activation, wheel,
  right-click context menu) flows through the panel's normalized
  `MediaListSurfaceInput` → `delegate_operation` intent path, replacing the
  per-owner bespoke `handle_search_pointer` body; the already-dead raw-event
  mouse path (`InlineSearch::handle_mouse`, `InlineSearchMouse`) is
  **removed**.
- The empty/loading result box keeps today's placeholder states
  (" Loading…" / " (empty)") painted by the panel's placeholder painter.
- The legacy painters become deletable and are deleted in this change:
  `render/components/list.rs`, `render/components/list_letter_groups.rs`,
  `render/components/media_list/plain_rows.rs`, and the
  `render_inline_search` render component; the `LibraryListRenderCtx`
  search fields (`search_query`, `search_loading`, `with_search`,
  `is_search_active`) become dead and are trimmed. `list_rows.rs`
  (`LibraryListRenderCtx`) survives for its other consumers (TV/Music wide
  contexts, detail), as do the canonical painters in
  `render/components/media_list/{row,wide}.rs`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `inline-library-search`: "Results render as a flat list on every library
  type" changes its renderer — from the legacy plain column-aware painter to
  the shared canonical fixed-row media-list presentation (selected-row bar,
  zebra, theme roles), keeping the flat no-headings contract and adding the
  placeholder/selection-stability wording the canonical path implies.
- `canonical-media-lists`: adds a requirement covering Inline Search
  results as a composed shared-owner flow (embedded carrier in the search
  control, one fixed-row presentation in the list box, stable-target
  selection, re-score reset vs pool-refresh preservation, search box stays
  selector-row chrome outside the row flow).

## Impact

- **Control**: `src/app/components/inline_search.rs` — embeds
  `MediaListCarrier<String>`, drops its own `order`/`cursor`/`scroll`/
  `layout`/gesture state, routes movement keys through carrier operations;
  `InlineSearchHost` defaults stay.
- **Owners**: `emby_library_content.rs`, `tv_content/{interaction,mod}.rs`,
  `music_content.rs`/`music_interaction.rs` — search pointer paths delegate
  to the carrier and translate standard `RowIntent`s; keyboard translation
  arms unchanged in shape.
- **Panel paint**: `src/app/components/library_panel/wide.rs`
  (`paint_browser_pane`, used by both Wide and Narrow) — the `ListSlot::Search`
  arm paints `render_search_box` plus the carrier through the `PanelList`
  surface, replacing `render_inline_search`.
- **Render tree**: deletions listed above; `render_search_box`
  (`render/components/hero.rs`) survives as the bar painter;
  re-exports in `render/mod.rs` trimmed.
- **Tests**: characterization/pointer/search tests in
  `emby_library_inline_search_tests.rs`,
  `tv_content_component_tests_search.rs`, `library_panel/wide_tests.rs`,
  tick-integration TV/Music/Emby-library search tests updated; new rendered
  evidence for bar/zebra/stable-target behaviors.
- **No Service/queue/persistence surface changes**; playback, activation,
  context-menu, and shortcut requests keep their existing typed `Msg`
  contracts (this change standardizes the row path, not the actions).
