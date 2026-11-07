# Proposal: add-gui-library-panel

## Why

The GUI Client presents only the player panel (Queue, Queue playback, Status bar). A user cannot browse a library or start playback from the window (issue #986). The design canvas now has a Library artboard (`canvas/Library.dc.html`, Music › Hip-Hop), so the Library panel can join the GUI with the Queue panel, as the TUI arranges them.

## What Changes

- **The GUI presents the Library panel beside the Queue panel.** Panel focus moves between the two panels as in the TUI. The window arranges the panels by the shared breakpoint classes (`mbv_surfaces::breakpoint`): Mini shows one panel, Narrow shows the browser pane plus the Queue panel and opens the Hero as an overlay, Wide shows the Hero beside the browser pane plus the Queue panel. The Queue panel keeps its four states.
- **All seven destinations are in scope:** Home, Movies and other Emby libraries, Shows (TV), Music, Audiobooks, Podcasts, and Feeds. The Music artboard is the template for every slot. Each other destination paints the same slots with the content its TUI owner supplies (`LibraryPanelContent`): its selector pills, its list, its Hero arm (Landscape, Portrait, Square, or no Hero), and its Workspace (none, a list, or a selector plus a list).
- **Library interaction state moves to the shared Client core.** The cursor, viewport, tree open state, pill choice, Workspace focus, and filter draft of each destination owner move from the TUI components into `mbv-surfaces` Surface state. The TUI components become adapters. **The TUI output does not change.**
- **Collection pills are multi-select in the GUI** (canvas SPEC note). A press toggles a pill, Alt solos it, Space and Alt+Space are the keys, at least one pill stays on, and the list is the de-duplicated union of the selected collections. The collection pills are the Music groups, Podcast shows, Feeds subscriptions, and Emby feed groups; every other pill stays single-select. The TUI stays single-select. Each destination keeps its set for the session; across a restart only Music's selected groups persist, because Music's group is the only live pill the launch state keeps today.
- **The letter rail** jumps the list to a letter on lists sorted by name. It is a pointer shortcut; the keyboard path is the existing `/` filter.
- **The tab bar and the global Search** paint in the top bar. `Ctrl+K` opens Search in a GUI-only key context (beside the existing `Ctrl+/`); the Search results paint as a media list in the Music row look. The Search sidebar state moves to the shared Client core.
- **A Library Status bar paints under the Library column** when the Library panel shows, as in the canvas: the shared prefix and toast, plus GUI-only key hints with their configured chords and the location (destination, selected pills, selected item). Prefix and toast stay in the Queue footer when only the Queue panel shows.
- **Library art comes from the mbv image cache through the frame**, not from a webview fetch.
- **Delivery has a gate.** Music lands first from end to end. A `/code-review` round and a user live-test round accept it before the other destinations start.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the GUI presents the Library panel, its slots, the destinations, the responsive arrangement with the Queue panel, Panel focus, multi-select collection pills, the letter rail, the tab bar, global Search, and the Library Status bar.
- `tui-launch-state`: the snapshot also carries the GUI Client's extra selected Grouped Music groups; the TUI restores only the first.

## Impact

- `crates/mbv-surfaces`: new Library Surface state (selector, list, tree, Workspace, filter, Search) used by both Clients.
- `crates/mbv-components`: the seven destination owners, `library_panel`, `tab_panel`, `inline_search`, and `search_sidebar` become adapters over the Surface state. Paint and key output are unchanged.
- `src/app` and `src/lib.rs`: the shell projection that builds the GUI Library frame; the GUI-only key context gains `Ctrl+K`.
- `crates/mbv-config`: `TuiLaunchState` gains an additive field for the extra selected Music groups; only the GUI applies it.
- `crates/mbv-gui`: frame DTOs, region ids, `ListId`s, leaves, core-thread wiring, and the Svelte Library components.
- `docs/architecture/gui-client.md`, `docs/architecture/interactive-surface-ledger.md`: the new surfaces and owners.
- No Owner-side, ctrl protocol, or Player change. No new dependency.
