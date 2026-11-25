# Proposal

## Why

In the GUI Client, a press on a Music artist row does nothing visible. The Hero shows an album, and the artist row picks no album, so the Hero does not change. The TUI shows an artist page for a focused artist root: the artist's artwork, name, album count and year span, and the artist's tracks grouped by album. The GUI lost that page in the migration. The Shows list is the model of what the GUI should do: a press on a series shows the series in the Hero.

## What Changes

- A press on a Music artist row picks the artist, and the Hero shows an **artist page** for that artist. This is the same behavior as the TUI artist root detail, in the GUI's Hero frame:
  - The header band shows the artist name and the plain facts: the selected collections, the album count, the release-year span, and the artist's track count and total time (when the tracks have loaded).
  - The detail block shows the artist's artwork and the actions Play, Add to queue, and Shuffle artist. Play and Add to queue act on the whole artist: every album of the artist in the list, in the list's album order.
  - The list band shows the artist's tracks grouped by album, in album order and then disc and track order. Each album group has a label with the album name and year.
  - A double-click on a track row, its hover Play control, or Enter while the pointer rests on it plays from that track through the rest of the artist's tracks that the page shows. It does not stop at the end of the album.
- The artist page opens the tracks of every album of the artist when the artist is picked, so the page fills in as the loads land.
- A press on an album row or a track row of the list shows that album, as it does now. This also ends the artist page.
- The artist row stays marked in the list while the Hero shows its page.
- The artist page is a view of the session. It is not saved in the launch location, and a restart opens the group's kept album. A change of the collection selection keeps the page while the new selection still holds the artist, and otherwise the Hero shows an album as it does now.
- The Narrow arrangement does not change: a press picks, and only the row menu's View Detail opens the Hero overlay. The overlay then shows the artist page for a picked artist.

### Decisions made while planning

- The change covers the GUI Client only. The TUI keeps its own artist detail.
- The page lists the artist's tracks grouped by album, as the TUI Workspace does. It is not a list of album rows. An album group label is a visual label. A press on it does nothing. The user reaches an album Hero from the album rows in the list.
- The artist page has no More actions control. An artist item offers no entries that the Hero's buttons do not already cover. Shuffle artist stays in the header band, as it is for an album.
- The page takes the tracks of each album from the per-album track cache, through the bounded fan-out the core already has for the TUI's fallback artist. It does not use the TUI's artist-id query, because the GUI Music list is a union over the checked groups and holds no settled catalog revision to key that cache.
- The sequence for a played track is the sequence the page shows. An album whose tracks have not loaded yet is not in it, as in the TUI.
- The artist page is not persisted. The launch location keeps one album per group, and an artist page does not replace it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the requirements "The Music list is the canvas tree", "The Hero shows the picked item with its actions", "Every Hero is one frame with the largest unit as its title", and "Each Music group keeps its last picked album" change, and the new requirement "A picked Music artist shows an artist page" is added.

## Impact

- `crates/mbv-client-core`: one new `pub` intent method on `App` that starts the bounded track loads for a list of albums.
- `crates/mbv-gui/src/gui_core`: the Music projection (the Hero subject), the pick state, the Library command set, and the ensure pass for Music.
- `crates/mbv-gui/src/bridge`: the Hero frame, the command mirror, and the section frames.
- `crates/mbv-gui/src/window`: the Library state's row press, the Music Hero component, the tracklist, and the hovered-track key route.
- `docs/architecture/gui-client.md`, `CONTEXT.md` (the term Artist page).