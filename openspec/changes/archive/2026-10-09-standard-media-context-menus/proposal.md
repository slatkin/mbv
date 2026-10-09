# Proposal

## Why

Library right-click menus differ from screen to screen for no reason.

- **Audiobookshelf:** podcast episodes and books have no menu at all.
- **YouTube:** rows in the Emby homevideos feed view, for example a YouTube
  tab, look up their targets in a different item set than the one they
  paint. A right-click or `.` there resolves no item.
- **Labels:** single Emby rows say "Watched", while multi-selections and Feeds
  say "Played".
- **Feeds:** a single row shows both mark entries, its multi-selection menu
  has no Shuffle, and a Feeds bulk action never clears its selection.
- **Music:** albums and artists offer "Mark Watched", but tracks offer no mark
  entry.

This change sets one standard for library menus. Each exception is written
down with its reason.

## What Changes

- **One standard action set for every library list.** Entries appear in this
  fixed order:
  - Leaf row: Play, Add to Queue, then one played-state entry.
  - Collection row: Play All, Shuffle, Add to Queue, then one played-state
    entry.
  - Multi-selection: Play, Shuffle, Add to Queue, Mark Played, Mark Unplayed.

  A list-specific removal goes after the mark entries. Examples are Remove
  from Continue Watching and the bulk Remove.
- **One label set.** "Mark Played" and "Mark Unplayed" are used everywhere.
  "Mark Watched" and "Mark Unwatched" are retired. A single row shows only the
  entry that changes its current state.
- **Documented exceptions:**
  - Audiobookshelf books are leaves. Their chapters are positions inside one
    queue item, so a book gets no Shuffle.
  - Music tracks, albums, and artists offer no played-state entry. mbv never
    resumes music, and it ignores music played state everywhere. This covers
    multi-selections too.
  - Continue Watching adds Remove from Continue Watching.
  - Selector pills open no menu. These include podcast shows, feed
    subscriptions, YouTube channel groups, and letters.
  - Inline search results keep their single-row-only menu.
- **YouTube and Latest rows:** the Emby library list sends the items it
  paints, so menus open on homevideos feed view rows and on Latest rows. The
  separate shell lookup, `ContextMenuTargets::Browser`, is removed.
- **Feeds:** the single-row menu matches the standard, the multi-selection
  menu gets Shuffle, and a bulk action clears the selection.
- **Audiobookshelf:** podcast episode and book lists open the standard menu
  from `.` and from right-click. Played state becomes writable through
  `PATCH /api/me/progress/...` and `/batch/update`. After the server accepts,
  mbv applies the change to local browse and queue progress itself, because
  the server sends no `user_item_progress_updated` for a manual change.
- **Out of scope:** the Queue panel, the Playlists sidebar, and the Search
  sidebar. They are not library lists and keep their current menus.

## Capabilities

### New Capabilities
- `audiobookshelf-played-state`: marks Audiobookshelf episodes and books
  finished or unfinished on the server, and applies the result to local
  browse and queue progress.

### Modified Capabilities
- `context-menu`: the keyboard-anchoring requirement covers every library
  list. New requirements define the standard action set, the entry order, the
  played-state label and its choice by state, where Shuffle appears, and each
  documented exception.

## Impact

- `crates/mbv-audiobookshelf`: bounded single and batch progress PATCH calls.
- `crates/mbv-ui-model/src/context_menu.rs`:
  - Add an Audiobookshelf target type and a `ContextMenuTargets::Audiobookshelf` variant.
  - Remove `ContextMenuTargets::Browser`.
  - Add Audiobookshelf and Feeds-shuffle `ContextAction` variants.
  - Add a length rule to `is_bulk_action`.
- `crates/mbv-components`:
  - `EmbyLibraryContent` sends `ContextMenuTargets::Emby` with its painted items.
  - `PodcastContent` and `BookContent` open menus.
- `src/app/dispatch/context_menu/`: Emby label and music changes, new entry
  order, the Feeds menu split, the Audiobookshelf builder and actions, and
  played-state writes.
- `src/app/state/context_menu_capabilities.rs`: music items are not
  played-state capable.
- `src/app/shell/messages.rs`: route the Audiobookshelf targets, drop the
  Browser arm, and give each target variant an explicit arm.
- No ctrl protocol, persistence, or daemon change.
