# Proposal

## Why

The player panel's header row — the 56 px band above the artwork — adds a strip of chrome
the panel does not need. It holds only three facts, each of which reads better where the
user's eye already rests: the playback-target chooser and the collapse control belong with
the footer's other queue-wide controls, and the `PLAYING`/`PAUSED`/`IDLE` word states
nothing the window does not already show (the transport's play/pause glyph, the scrubber's
motion). Removing the band lets the artwork start at the column's top edge.

The Library top bar sits inside the Library column only, so the two columns start at
different heights. It becomes the window's global header, one band across both columns.
The Library Status bar's left side paints fixed key hints that read as gibberish; they go,
and the Search field moves out of the header into that place.

## What Changes

- The player panel's header row is removed entirely. The artwork becomes the panel's
  topmost content, full-bleed at the top of the queue column, directly under the global
  header. No band replaces the player header inside the column.
- The Library top bar (Home icon, destination tabs, Settings control) leaves `LibraryPanel`
  and becomes the window's global header: one band across the full window width, above
  the Library column, the divider, and the Queue column or its collapsed rail. It paints
  in every arrangement, Mini included, and is absent while the Settings screen shows. In
  Mini with the Queue showing, a tab press makes the destination active and shows the
  Library in place of the Queue.
- The Library Status bar's key hints are removed. The Search field moves from the top bar
  to the left side of the Library Status bar, and its results open upward. The armed
  prefix and the toast paint after the field; the location stays at the right.
- The playback-target chooser (the session chooser: the local player, Cast receivers, and
  Emby Sessions) moves to the left side of the Queue footer band, ahead of the autosave,
  source, and dirty content that already paints there. Its popover opens upward from the
  footer, anchored to the button, because the band sits at the window's bottom.
- The collapse control moves to the left side of the Queue footer band, after the
  playback-target control. It keeps the `z` chord and the core's collapse toggle; only the
  painted control moves.
- The now-playing status word (`PLAYING`, `PAUSED`, `IDLE`) is removed, not moved: no
  surface paints it. The GUI bridge frame stops carrying the now-playing status fact, and
  its type goes away with it; the library's projection fact stays (the TUI Status bar keeps
  its own use of it).
- No core-side change: the region targets (`playbackTarget`, `collapse`, `targetChoice`)
  and the Client core's chooser handling stay as they are. The footer simply reports the
  two moved controls' regions instead of the player panel's header.

## Capabilities

### New Capabilities

- none

### Modified Capabilities

- `gui-client`: the requirement "The first GUI slice presents the player panel of the
  design canvas" changes — the expanded panel shows no header row, the footer line's left
  side carries the playback-target chooser and the collapse control, and no surface paints
  a now-playing status word; the top bar becomes the window-wide global header.
- `gui-client`: the requirement "The top bar shows the destination tabs and Search" changes
  — the window shows the top bar across both columns, and the Search field sits in the
  Library Status bar.
- `gui-client`: the requirement "The Library Status bar shows hints and the location"
  changes — no key hints; the Search field leads its left side.

## Impact

- `crates/mbv-gui/src/window/components/queue_playback_panel.rs`: the header row and its
  popover go; the artwork becomes the topmost content; its region report drops the
  `playbackTarget`, `collapse`, and chooser-row regions.
- `crates/mbv-gui/src/window/components/queue_footer.rs`: gains the playback-target
  control and the collapse control at the left of its band, the upward-opening chooser
  popover, and the regions its contribution reports.
- `crates/mbv-gui/src/bridge/frame/playback.rs` and the bridge sections builder: the
  now-playing status fact and `GuiNowPlayingStatus` are deleted; test fixtures that build
  a playback section lose the field.
- Tests in `queue_playback_panel`'s and `queue_footer`'s suites move with the surfaces.
- `crates/mbv-gui/src/window/app.rs`: mounts the global header above the panel row,
  absent on the Settings screen; Mini's tab press shows the Library.
- `crates/mbv-gui/src/window/components/library_panel.rs`: no longer mounts `LibraryTopBar`.
- `crates/mbv-gui/src/window/components/library_top_bar.rs`: loses the Search field and its
  dropdown; keeps the Library pointer zone.
- `crates/mbv-gui/src/window/components/library_status_bar.rs`: loses `HINTS`; gains the
  Search field and the upward results dropdown.
- `docs/architecture/gui-client.md` (the header, the Search field's place, the `Ctrl+K`
  rule) and the touched components' module comments.
