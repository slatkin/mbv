# Proposal

## Why

The Grouped Music tree shipped in `add-grouped-music-tree-browser` gets its
core browsing right, but five interaction gaps make it unusable next to the
other libraries: Enter on an artist root only toggles expansion instead of
opening the artist Hero, track rows have no track numbers, the artist Workspace
plays only one album when the user expects the rest of the discography, pointer
clicks on the tree never pull panel focus, and double-click does the wrong
thing (it opens a Hero instead of expanding or playing). Separately, the
queue's "Go to Library" on a music item silently does nothing.

## What Changes

- With no tree filter active, Enter on a Grouped Music artist root opens/focuses
  its Hero (Wide artist Workspace, otherwise the Library Hero overlay), matching
  Enter on an album leaf; while filtering, Enter retains the filter's local
  expansion behavior and does not open the overlay. Left/Right keep owning
  unfiltered expansion and the artist-Workspace entry.
- The tree's third level (track items) prefixes each track title with its
  track number, matching the Workspace track rows.
- Enter (and pointer activation) on a track inside the **artist** Workspace
  plays the artist's whole in-scope discography from that track onward instead
  of only that track's album. Album-Workspace activation is unchanged.
- A pointer gesture on the Grouped Music tree that resolves a row now requests
  Library panel focus, so clicking the tree focuses the panel like every other
  library list.
- Double-click on a Grouped Music tree node toggles expansion when the node is
  expandable, and otherwise plays the resolved item now — asking for
  confirmation when the queue is populated. This replaces double-click opening
  the Library Hero overlay for Grouped Music.
- Fix the queue "Go to Library" no-op on music items so the navigation either
  lands on the album with the track selected or reports a visible failure;
  diagnose the silent path first with a failing tick-level repro.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `grouped-music-tree-browser`: artist-root Enter/Hero entry, track-number
  titles in the track level, pointer-driven focus request, and double-click
  expand-or-play semantics.
- `music-library-hero`: Wide artist-root Enter enters the artist Workspace,
  and focused artist-Workspace track playback starts the whole discography
  from that track.
- `library-hero-overlay`: non-Wide artist-root Enter opens the artist overlay;
  Grouped Music double-click no longer opens the overlay (it expands or plays).
- `item-library-navigation`: Grouped Music landings SHALL never silently
  no-op — the landed album/track selection is guaranteed and a failure is
  surfaced.

## Impact

- `src/app/components/music_content*.rs`, `music_interaction.rs`,
  `music_tree*.rs` — tree key/mouse translation, track label projection,
  artist-Workspace activation intent.
- `src/app/components/library_panel/panel.rs` — Grouped Music double-click no
  longer taking the overlay-first path.
- `src/app/actions_navigation.rs`, `src/app/shell_messages.rs` — discography
  playback request and handler.
- `crates/mbv-core/src/config_types_queue_state.rs` and queue-source presentation
  matches — persisted `QueueSource::Artist` support; older variants remain
  wire- and persistence-compatible, while older binaries are not expected to
  read the new variant.
- `src/app/library_search_actions.rs`, `library_browse_actions.rs`,
  `lib_event_actions.rs`, `shell_inline_search.rs` — music navigation landing
  and the Go-to-Library diagnosis fix.
- Tests: tree key/mouse tick integration, artist Workspace playback, track
  label projection, and the item-navigation regression family.
