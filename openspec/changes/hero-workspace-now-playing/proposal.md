# Proposal

## Why

A hero Workspace (TV episodes, Music tracks, Audiobookshelf chapters) derives its row state only
from each item's stored facts, so the one row that is actually playing in the queue looks like
every other row. The Queue already paints its now-playing row with a highlight and live progress;
the Workspace, which shows the same media, stays silent. A user browsing a series, album, or book
cannot see which constituent is playing without opening the Queue.

## What Changes

- A Workspace row whose item is the queue's active item paints the canonical `NowPlaying` semantic
  state: the same treatment the Queue's now-playing row uses, including the live inline progress
  percentage that follows playback.
- Matching is by item identity, not queue origin: a row lights up whenever its item is the active
  queue item, whatever surface started playback. The pending-selection rule mirrors the Queue: the
  slot the user just selected paints as now playing before the owner confirms it.
- All three Workspace kinds participate: TV episode rows, Music track rows (album and artist
  Workspaces), and Audiobookshelf book chapter rows. For chapters, the active queue item is the
  book, so the playing chapter is derived from the playback position within that book.
- The Workspace re-projects as playback ticks advance, exactly as the Queue column does today.
- Surfaces without a Workspace (Home, Movies, home videos, Feeds, Audiobookshelf podcasts) are
  unchanged.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `library-panel`: adds the requirement that hero Workspace rows mark the queue's now-playing item
  with the canonical now-playing row state and live progress, across the Wide Hero pane and the
  Library Hero overlay (one unified Workspace look).

## Impact

- `src/app/shell/` workspace push paths (`push_tv_workspace_content`, `push_music_workspace_content`,
  `push_audiobookshelf_book_content`) gain the shell-owned now-playing projection input; the sync
  pass already re-projects these owners every tick, so live progress flows without a new tick path.
- Workspace content owners in `crates/mbv-components/` (`tv_content`, `music_content`,
  `book_content`) apply the now-playing override to matching rows; row builders stay pure.
- `mbv-render`'s `media_list` row painter already renders `MediaSemanticState::NowPlaying`; no new
  paint vocabulary.
- No queue, player-owner, or provider client changes.
