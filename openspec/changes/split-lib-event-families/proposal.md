# Proposal

## Why

`App::handle_lib_event` (`src/app/dispatch/library/event.rs`) is the most complex function in the workspace: cyclomatic 36, 186 SLOC (rust-code-analysis-cli), carrying the user-approved `#[expect(clippy::too_many_lines)]` from design D5 of `2026-09-27-decompose-app-god-type` (issue #827). The complexity is structural: `LibEvent` is one flat enum of 35 variants, so any exhaustive, wildcard-free match over it is a 35-arm function. Splitting the function without changing the type would need wildcards or `unreachable!()` passthroughs, which is exactly what D5 removed.

## What Changes

- `LibEvent` becomes a small top-level enum whose variants wrap typed family enums: `Browse(BrowseEvent)`, `Music(MusicEvent)`, `Series(SeriesEvent)`, `Audiobookshelf(AudiobookshelfEvent)`, `Playlist(PlaylistEvent)`, `ModelContent(ModelContentEvent)`, plus the two cross-cutting variants `QueueEnriched` and `Error` left at the top level.
- `handle_lib_event` becomes one exhaustive match over the top-level variants, each delegating to a family dispatcher (`handle_browse_event`, `handle_music_event`, …) that matches its own family enum exhaustively. No wildcard arms, no `unreachable!()`, no `Option<LibEvent>` passthroughs: exhaustiveness is enforced by the compiler at every level.
- Every `ModelContentEvent` variant is shell-applied; the App side gets one documented no-op arm for the whole family.
- The `#[expect(clippy::too_many_lines)]` on `handle_lib_event` is deleted.
- Every producer and every shell-side pattern (`shell/run/drains.rs`, `shell/inline_search.rs`, `shell/run.rs`, tests) is updated to the nested shape. No behaviour change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Pure refactor; `skip_specs: true`.

## Non-goals

- `handle_player_event` (explicitly out of scope per #827).
- The shell drain's pre-existing `ev => self.handle_inline_search_lib_event(ev)` catch-all and the `unreachable!` in `handle_restored_library_position_event` (`shell/run.rs`): patterns are updated to the nested shape only, their structure is unchanged.
- Renaming or re-shaping any handler method or event payload beyond the family move.

## Impact

- `src/app/state/events.rs` (type change), `src/app/dispatch/library/event.rs` and `event/{audiobookshelf,browse_loads}.rs` (dispatch).
- ~40 files that construct or match `LibEvent` (~170 references), mostly one-line wrapper edits.
- No API, dependency, protocol, or persistence change.
