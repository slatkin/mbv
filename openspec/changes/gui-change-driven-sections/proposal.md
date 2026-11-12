# Proposal

## Why

The GUI Client copies the TUI repaint loop. The core thread ticks on input, on a wake, or on a timer (`CLIENT_IDLE_CADENCE`, `App::frame_cadence_due`). Then it emits one full `GuiWindowFrame` over `mbv://frame` that carries every section, and the front end keeps only the last frame. The timer exists mainly to move the playhead: the Owner applies progress as a status apply that no drain reports as an event. Thus every projection (Library, Home, Shows, Search, Settings, the Status bar, the queue) rebuilds and re-serializes on each frame (up to 6.7 times each second during playback), also when nothing changed. Artwork rides as base64 `data:` URLs inside these frames. "Carry once" flags (the armed theme, the artwork memo, the trickplay latch, the Library content revision) exist only to work around the full snapshot. Each new section, for example Settings from `add-gui-settings-screen` (PR 998), adds cost to every frame. This is not how a web UI works. A web UI keeps one store for each kind of data, updates a store only when its data changes, and lets the browser animate time-based values and load images by URL.

This change closes issue #997 and includes the PR 998 efficiency findings.

## What Changes

- **BREAKING (bridge-internal):** the `mbv://frame` event and the `GuiWindowFrame` / `GuiFrame` snapshot are removed. They are replaced by one `mbv://sections` event that carries only the sections that changed in a tick. Each section has its own revision. The front end keeps one store for each section and replaces only the stores that a batch names. Nothing outside `crates/mbv-gui/` reads this contract.
- **Change-driven emit.** No repaint timer. The core thread still wakes on input, on a wake, and on the idle wait, because the shared tick has time-based phases (maintenance, toasts). But a tick emits a batch only when at least one section changed. A window with stopped playback gets no events, except when a toast expires or a Library deadline (design D3) passes. A window with paused playback also gets the finish clocks when their minute changes. `App::frame_cadence_due` is deleted.
- **Local playhead.** A new `playhead` section carries the active slot, the position, the runtime, and the paused state. The core emits it only on a discontinuity: track change, pause or resume, runtime change, a seek, or a drift past a fixed tolerance. The front end advances the position itself between anchors, and it derives the scrubber fill, the elapsed and remaining clocks, the rail ring, and the compact strip's progress line. Facts with a minute or percent grain (the finish clocks, the queue row's progress, the current-chapter mark) stay Rust projections and emit when their value changes.
- **Gated projections.** Expensive sections (the Library list, the Library detail, Search, the Library row menu, Settings, the theme) rebuild only when the core marks them dirty. Inputs, drain hooks of the shared tick, and named deadlines set the marks. Cheap sections are built on each tick and compared with the last value. A test-only audit rebuilds every clean section and fails on a missed mark.
- **Section split.** The Library's list content and its detail (the selected row, the Hero, the Workspace, the item menu) become separate sections, so a pick re-sends the detail and not the whole list. Search and the Library row menu become their own sections.
- **Window sync.** The front end calls a new `sync` command when it mounts. The core then sends every present section in one batch. `report_layout` applies layout only. It no longer forces a frame or re-arms the theme, the artwork, or the Library content. The attach phase answers `sync` the same way.
- **Art by URL.** The core serves cached artwork through a Tauri custom URI scheme, `mbv-art`. Sections carry an art reference with the image's URL, not image bytes. The window loads `<img src>` by that URL. The core answers each request from its image cache and the window makes no network request. This removes the base64 work on the frame path, the per-frame JPEG clone, the artwork memo, and the trickplay sheet latch (#997 items 2 and 6).
- **Front end scroll split.** The Library lists keep the scroll offset in its own store. They derive their rows from the content, the open nodes, the filter, and the filter state only, so a scroll event does not rebuild rows (#997 item 1).
- **Projection clean-ups.** Search returns at once for an empty query and stops double-copying rows (#997 item 5). Settings reads the configuration without a full `Config` clone, borrows `GuiSettingsState`, and borrows its choice lists (the PR 998 findings).
- **Measure first.** The work starts with criterion benches of the frame path and a vitest bench of the Library row derivation. Each optimization is judged against that baseline (`docs/architecture/performance.md`).

## Prerequisite

`add-gui-settings-screen` (PR 998) merges to `main` before this change is applied. The Settings tasks edit that merged code.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the window's update contract changes. New requirements cover change-driven section updates, the window sync on mount, and the local playhead. The Library art requirement changes from "through the frame" to "from the core's image cache by URL". The window-reload scenario of "The window content holds no interaction state" names the sync.

## Impact

- `crates/mbv-gui/src/`: `core_thread.rs` (the step's emit rule, the `Sync` and `Art` inputs), a new section ledger, `bridge/frame*` (DTOs regrouped as sections, art references), `library_frames.rs`, `artwork.rs` and `trickplay.rs` (the memos are removed), `main.rs` (the event, the `sync` command, the URI scheme), `attach.rs`.
- `crates/mbv-gui/ui/`: `bridge/frames.ts` is replaced by section stores, a new playhead store and duration formatter, `library/state.ts` (the scroll store), every component that reads `$frames`.
- `crates/mbv-gui/tauri.conf.json`: the CSP `img-src` replaces `data:` with `mbv-art:`.
- `src/app/`: `frame_cadence_due` is deleted. `gui_search_projection` returns early. The Settings projection's clones are removed. Six read-only `App` queries are added, and the TUI calls none of them: `gui_navigation_idle` and `gui_art_retry_due` (the Library deadlines), `gui_playhead_marks` (the playhead marks), `gui_art_source` and `trickplay_sheet_bytes` (the `mbv-art` answers), and `with_config` (the Settings read). One test seam, `backdate_navigation_for_test`, is added. The TUI loop, `ClientCore::tick`, and `ClientTickHost` do not change.
- Dependencies: `crates/mbv-gui` adds the existing workspace dev-dependency `criterion` (0.8.2) for its bench. No new crate in the workspace.
- Docs: `docs/architecture/gui-client.md` (process shape, bridge contract, layout reports, theme, player panel, artwork, Library section), `docs/architecture/interactive-surface-ledger.md` (the GUI Library panel row names the scroll store), `docs/architecture/performance.md` (the new benches), `CONTEXT.md` (*Window section*, *Section batch*, *Playhead anchor*), a new invariant 32 (a section's dirty marks cover its sources), invariant 27 (the GUI art URL and retry mark), and ADR 0038.
