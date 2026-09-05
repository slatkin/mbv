Tracks: https://github.com/slatkin/mbv/issues/653

## Why

Media lists share one painter (`wide_media_row`) but each parent formats its own duration string, so queue shows `4:32` while TV/music/books show `4m`. Folder-ness is also smuggled into `primary` strings per parent (`"Name · N items"` copied twice in `browser/mod.rs`).

## What Changes

- One owner for list duration text: all `MediaListRow::Item.duration` projections use `fmt_duration_short` (`4:32`, `1:02:03`) when a duration is shown.
- Add `MediaKind { Collection, Media }` field on `MediaListRow::Item`; painter suppresses duration for `Collection` and owns count-suffix rendering.
- Migrate TV episode, music track, book chapter/file projections from `fmt_duration_approx` to short. Queue/Home/Feeds already short (Feeds via duplicate — fold it).
- Fold `fmt_duration_mmss` (selection modal only) into `short`; delete the duplicate `feeds_model::format_duration`.
- Duration-free catalogs (browser Movies/generic, albums, shows, series) stay `duration: None` — no new duration columns.
- Hero/detail/modal `approx` strings out of scope (follow-up).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `canonical-media-lists`: `duration` slot semantics (short when present) + `MediaKind` on `Item` rows.

## Impact

- `src/app/ui_util.rs` (one helper, delete `mmss` dup), `src/app/components/media_list/mod.rs` (kind field), `src/app/render/components/media_list.rs` (painter owns kind rules).
- Projections: `tv_workspace`, `music_workspace`, `audiobookshelf_book.rs`, `browser/mod.rs`, `feeds.rs`, `feeds_model.rs`, `selection_modal_actions.rs`.
- Buffer tests for kind + duration painter rules; existing characterization tests unchanged in output except `4m`→`M:SS` rows.
