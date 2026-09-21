## Why

A media-list row's release year and its publish date are the same kind of thing — a date fact about the item — but today they paint in two different places with two different colours: a year sits inline right after the title in green (`STATUS_AVAILABLE`), while a publish date sits in the fixed right-aligned gutter in a distinct yellow (`ROW_DATE_FG`). This split is unintentional (no requirement calls for it) and reads as inconsistent across screens: Movies/TV/Generic rows carry their year one way, podcast episode rows carry their date another way. Standardising on the gutter placement and on green closes that gap.

Separately, the Library panel's Hero Workspace lists (TV season episodes, music album/artist tracks, book chapters) project no duration at all, because the current requirement grants a duration only to the Queue list. A Workspace row is the playable leaf a viewer is about to start — its runtime is the fact they want — so the Workspace lists gain the duration slot too, while ordinary browse lists keep none.

## What Changes

- Keep `MediaListTrailing::Year(String)` and `MediaListTrailing::Published(String)` as two variants. Both paint right-aligned in a fixed gutter at the row's right edge, in `STATUS_AVAILABLE` (green); the year gutter reserves four columns and the date gutter six, because a year is four characters and a date is at most six.
- Remove the inline-after-title paint path for years; the inline trailing slot keeps only the FOAM progress badge.
- Delete the `ROW_DATE_FG` palette role (`src/app/render/theme/mod.rs`); both consumers (the shared row painter and the music tree label) move to `STATUS_AVAILABLE`.
- Narrow the music tree's own year gutter from six columns to four, matching the year's actual width. The music tree browser keeps its own gutter implementation by design — it is a tree-view component, not the canonical list, and its gutter applies only at the album level.
- **BREAKING (requirement)**: Hero Workspace lists project a duration string. `build_episode_rows` (TV), `track_row` (music), and `chapter_rows` (books) supply a `M:SS`/`H:MM:SS` duration instead of `None`. Ordinary browse lists still carry none, and `Collection` rows stay duration-free.
- Group headings (`MediaListRow::Heading`) and spacer rows continue to carry no gutter and no duration.

## Capabilities

### Modified Capabilities
- `canonical-media-lists`: the "Shared rows are provider-neutral and bounded" requirement currently describes two placements and two roles for date facts (inline green year, right-gutter `ROW_DATE_FG` date), and restricts durations to the Queue list. It changes to describe one placement (a right-aligned gutter) and one role (`STATUS_AVAILABLE` green) for both years and dates, with per-kind gutter widths, and to grant the duration slot to Hero Workspace lists alongside the Queue.

## Impact

- `src/app/render/components/media_list/row.rs` — the shared row painter: removes the inline-after-title path for years, extends the right-aligned gutter path to serve years (4 columns) and dates (6 columns), both `STATUS_AVAILABLE`.
- `src/app/render/theme/mod.rs` — delete `ROW_DATE_FG` (its comment already misdescribes its own value: `Palette::Iris` annotated "sage").
- `src/app/render/mod.rs`, `src/app/palette.rs` — re-export lists naming `ROW_DATE_FG`.
- `src/app/components/music_tree.rs` — `YEAR_GUTTER_WIDTH` 6 → 4; `src/app/components/music_tree_label.rs` — recolour the year to `STATUS_AVAILABLE`.
- `src/app/components/media_list/mod.rs` — `MediaListTrailing` doc comments describing the old split placement/role.
- Hero Workspace row builders gaining a duration: `src/app/components/tv_content/mod.rs` (`build_episode_rows`), `src/app/components/music_content_workspace.rs` (`track_row`), `src/app/components/book_content.rs` (`chapter_rows`).
- Buffer/paint tests asserting the old inline year placement or the old `ROW_DATE_FG` colour: `src/app/render/components/media_list.rs`, `src/app/render/tests_music_characterization.rs`, `src/app/render/tests_music_groups.rs`.
- Visual effect: every Movies/TV/Generic list row's year moves from immediately after the title to a four-column gutter at the row's right edge, colour unchanged. Podcast episode rows keep their gutter position and change colour from yellow to green. The music tree's album year shifts two cells right. TV episode, music track and book chapter rows in a Hero Workspace gain a right-aligned duration.

## Non-goals

- The year gutter and the date gutter do not line up column-for-column across screens. That follows from per-kind widths and is intended; no single list mixes both.
- The music tree browser is not migrated onto the canonical row painter.
