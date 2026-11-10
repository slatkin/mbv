# Proposal: add-gui-library-panel

## Why

The GUI Client presents only the player panel. A user cannot browse a library or start playback from the window (issue #986). The design canvas (`canvas/Library.dc.html`, *Library — Music › Hip-Hop*) specifies the Library panel of a mouse-driven GUI: destination tabs and a Search field in a top bar, multi-select collection pills, an artist tree with letter headings and a letter rail, a Hero with the album and its tracklist, and a Library Status bar. The canvas is the specification. The GUI does not need parity with the TUI, and the TUI does not change.

## What Changes

- **The GUI presents the Library panel beside the Queue panel**, as the canvas Library and Queue artboards show. The front end arranges the two panels from its own measured size: Wide shows the Hero beside the list, Narrow opens the Hero as an overlay, and Mini shows only the panel with Panel focus.
- **The GUI Library panel owns its interaction state** (ADR 0036). The front end holds the cursor, hover, scroll, open tree nodes, focus inside the panel, filter text, and the open Hero overlay and item menu, and it handles the panel's keys while the panel has Panel focus. The Client core holds the data selection (destination, selected collections, the Hero's item), loads the Service data, encodes the artwork, and runs the actions. No Library interaction rule moves out of the TUI.
- **Collection pills are single-select** (the TUI's Grouped Music pill, which replaces the canvas SPEC note's multi-select rules): a press selects a pill, an Alt-press does the same, Space selects the focused pill, one pill is always selected, and the list shows the selected group's albums in sort-name order.
- **The list** is the canvas tree: artists, their albums, and the album tracks, with letter headings, the `/` filter, and a letter rail that jumps to a letter.
- **The Hero** shows the picked album with its artist header and its tracklist, and the actions Play, Play next (`n`), Add to queue (`a`), Shuffle artist, and More actions. Play next is a new core action: it admits the item after the active slot. More actions opens the item's menu.
- **The top bar** shows the destination tabs and, with global search, the Search field (`Ctrl+K`).
- **The Library Status bar** under the Library column shows the canvas key hints and the location (destination › selected collections › picked item). The prefix state and toasts replace the hints while active.
- **The GUI keeps its own launch location**: the destination, the selected collection of each Grouped Music destination, and Panel focus, saved at orderly exit and restored at start. The Podcasts and Feeds pill rows save no pill and open on Latest. The GUI no longer reads the TUI launch state.
- **The service bootstrap becomes a shared Client step**, so the GUI runs the same Service startup as the TUI and its Library has content. The TUI's startup order and behaviour do not change.
- **Delivery has a gate.** Music lands first from end to end. A `/code-review` round and a user live-test round accept it before the other destinations and Search start.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the GUI presents the Library panel, owns its interaction state and keys, and keeps its own launch location. The keyboard, pointer, and window-content requirements of the player panel are scoped to the surfaces that the GUI shares with the TUI.
- `client-interaction-core`: the shared-rule requirements apply to the surfaces that both Clients present. A surface that one Client presents alone keeps its rules in that Client. The GUI derives no breakpoint class for the Library.

## Impact

- `crates/mbv-gui`: the Library frame section and bridge command, the core-thread wiring, the artwork encode for Library art, and the Svelte Library components.
- `src/app` and `src/lib.rs`: GUI-only Library entry points and projections (beside `src/app/gui_client_state.rs`), the Music collection store, the new play-next action, the shared service bootstrap step, and the GUI's Panel-focus and launch-restore settings.
- `crates/mbv-config`: an additive GUI launch-location file.
- `docs/adr/0036-gui-library-panel-owns-its-interaction-state.md`, `docs/architecture/gui-client.md`, `CONTEXT.md`, `AGENTS.md`.
- No TUI behaviour change. No new dependency.
- One Owner-side change rides with it: the Player's queue-rebuild transition gate now outlives its arming command and disarms on the rebuild's settle, and a real `QueueDesynced` resubmits the canonical queue instead of only relaying to Clients (`docs/invariants/29-mpv-playlist-drift-resubmits-the-canonical-queue.md`). The ctrl wire gains one additive `EmbyItem` field (`parent_id`, `#[serde(default)]`), re-blessed into the v10 conformance corpus; an older Owner ignores it on decode.
