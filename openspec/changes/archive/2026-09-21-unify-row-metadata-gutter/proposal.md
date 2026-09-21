## Why

A media-list row's release year and its publish date are the same kind of thing — a date fact about the item — but today they paint in two different places with two different colours: a year sits inline right after the title in green (`STATUS_AVAILABLE`), while a publish date sits in the fixed right-aligned gutter in a distinct yellow (`ROW_DATE_FG`). This split is unintentional (no requirement calls for it) and reads as inconsistent across screens: Movies/TV/Music/Generic rows carry their year one way, podcast episode rows carry their date another way. Standardising on the gutter placement and on green closes that gap.

## What Changes

- **BREAKING**: Collapse `MediaListTrailing::Year(String)` and `MediaListTrailing::Published(String)` into one `MediaListTrailing::Gutter(String)` variant. Every call site that builds a `Year`/`Published` row now builds `Gutter`.
- The merged variant always paints right-aligned in the existing fixed six-column gutter (unchanged width/truncation/reservation behaviour), never inline after the title.
- The gutter's colour becomes `STATUS_AVAILABLE` (green) for every row that carries it, replacing `ROW_DATE_FG`.
- Delete the `ROW_DATE_FG` palette role (`src/app/render/theme/mod.rs`); nothing else references it.
- Group headings (`MediaListRow::Heading`) and spacer rows continue to carry no gutter — no behaviour change there, since a heading has no `trailing` field today.

- Hero workspace lists merge their per-item durations into the green gutter: music tracklists, TV episode lists, and ABS book chapter lists project each item's duration right-aligned in the unified green gutter (`MediaListTrailing::Gutter`) instead of leaving it off — NOT the gold `DURATION` slot, which stays Queue-only.
- Audiobooks additionally show their total runtime in the gutter of the book's browser list row (where movies/TV show the production year).
- Gutter durations use a minutes-precision format — `M:SS` under an hour, `H:MM` at or over an hour — always ≤6 columns so `DATE_GUTTER_W=6` never truncates.

## Capabilities

### Modified Capabilities
- `canonical-media-lists`: the "Shared rows are provider-neutral and bounded" requirement (whose scenarios include "Trailing metadata carries its own role" and "Publish date paints in the fixed right-aligned gutter") currently describes two placements and two roles (inline green year, right-gutter `ROW_DATE_FG` date). It changes to describe one placement (right gutter) and one role (`STATUS_AVAILABLE` green) for both years and publish dates.

## Impact

- `src/app/components/media_list/mod.rs` — `MediaListTrailing` enum (type change).
- `src/app/render/components/media_list/row.rs` — the shared row painter: removes the inline-after-title trailing-pieces path, extends the gutter path to serve both years and dates.
- `src/app/render/theme/mod.rs` — delete `ROW_DATE_FG`.
- Call sites constructing `MediaListTrailing::Year`: `src/app/components/emby_library_content.rs`, `src/app/components/tv_content/mod.rs`, `src/app/render/components/music_wide.rs`, `src/app/components/inline_search.rs`.
- Call site constructing `MediaListTrailing::Published`: `src/app/components/podcast_content.rs`.
- Buffer/paint tests in `src/app/render/components/media_list.rs` that assert the old inline placement/colour for years and the old `ROW_DATE_FG` colour for dates.
- Duration projection sites: `src/app/components/music_content_workspace.rs` (track rows), `src/app/components/tv_content/mod.rs` (episode rows), `src/app/components/book_content.rs` (chapter rows + the book browser row's total runtime), switching their duration handling to the gutter.
- A new small gutter-duration formatter (minutes-precision, e.g. in `src/app/ui_util.rs`) with unit tests.
- Buffer tests for duration-in-gutter painting, plus updates to any tests asserting workspace rows carry no trailing metadata.
- Visual effect: every Movies/TV/Music/Generic list row's year moves from immediately after the title to the row's right edge, and its colour is unchanged (already green). Every podcast episode row's publish date keeps its position but changes colour from yellow to green.
