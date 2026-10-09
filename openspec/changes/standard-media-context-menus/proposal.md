# Proposal

## Why

Audiobookshelf book and podcast rows have no context menu. The context-menu
target resolver returns no menu for them, so Add to Queue cannot be reached.
The `AudiobookshelfBookIntent::Enqueue` and `PodcastEpisodeIntent::Enqueue`
handlers exist, but nothing sends to them. Feeds has a menu, but it differs
from Emby without a reason: it always shows both Mark Played and Mark
Unplayed, and its multi-selection menu has no Shuffle. Every library media row
should offer the same standard actions.

## What Changes

- Every library leaf row offers Play, Add to Queue, and one played-state entry
  chosen by state. A finished item shows "Mark Unplayed"; any other item shows
  "Mark Played". This covers Audiobookshelf podcast episodes, Audiobookshelf
  books, Feeds entries, and the existing non-audio Emby leaves.
- Shuffle appears only on rows that have a collection under them (Emby
  folders, as today) and on multi-selections. Episodes, books, and feed
  entries get no single-row Shuffle. A book's chapters are positions in one
  queue item, not a collection.
- Audiobookshelf podcast and book lists open a context menu from the menu key
  (`.`) and from right-click. With a multi-selection active, they show the
  shared multi-selection menu: Play, Shuffle, Add to Queue, Mark Played, Mark
  Unplayed.
- The Feeds single-row menu replaces its two mark entries with the single
  entry chosen by state. The Feeds multi-selection menu gains Shuffle.
- Audiobookshelf played state becomes writable. mbv sends `isFinished` through
  `PATCH /api/me/progress/:libraryItemId/:episodeId?`, or through
  `PATCH /api/me/progress/batch/update` for a multi-selection. After the
  server accepts, mbv applies the new state to browse and queue progress
  itself, because the server sends no `user_item_progress_updated` for a
  manual change.
- The `context-menu` scenario that says Audiobookshelf and Feeds open no menu
  is removed.

## Capabilities

### New Capabilities
- `audiobookshelf-played-state`: marks Audiobookshelf episodes and books as
  finished or unfinished on the server, and applies the result to local
  browse and queue progress.

### Modified Capabilities
- `context-menu`: the keyboard-anchoring requirement no longer excludes
  Audiobookshelf and Feeds. A new requirement fixes the standard entry set for
  library rows: which rows get Shuffle, and the single state-chosen
  played-state entry.

## Impact

- `crates/mbv-audiobookshelf`: new bounded client calls for single and batch
  progress PATCH.
- `crates/mbv-ui-model/src/context_menu.rs`: a new Audiobookshelf target type,
  a new `ContextMenuTargets` variant, and new `ContextAction` variants for
  Audiobookshelf play, shuffle, enqueue, and mark, plus a Feeds shuffle.
- `crates/mbv-components`: `PodcastContent` and `BookContent` handle `.` and
  right-click and emit `ShellRequest::RowContextMenu`, as Feeds does.
- `src/app/dispatch/context_menu/`: build the Audiobookshelf menu, change the
  Feeds menu, and add dispatch arms. Every new `ContextAction` variant gets an
  exhaustive arm.
- `src/app/shell/messages.rs`: route the new `ContextMenuTargets` variant.
- No ctrl protocol, persistence, or daemon change. Queue progress for queued
  Audiobookshelf slots uses the existing `QueueOp::ApplyProgress` relay.
