# Proposal: align-gui-with-design-canvas

## Why

The GUI Client landed close to the design canvas but not on it. Gap audits of the artboards against the built Svelte UI record the divergences (missing elements, wrong painted values, wrong behaviour). Two rulings now govern this change, both from the user (2026-11-07, binding): **ONLY the QUEUE in the GUI is brought in line with the Claude design canvas**, and **no new functionality lands** — no new player commands, data derivations, resize, or keys; the canvas sample text is mock content. The earlier ruling stands too: where the canvas disagrees with the current GUI UI, the canvas wins, and **the TUI does not change**. The canvas stays the governing spec for the queue paint.

## What Changes

- **The TUI does not change.** No row touches TUI code or output; the TUI's keybinds, palette, and paint are untouched.
- **Scope: the queue paint only.** The narrowed change paints the expanded queue panel's queue section — header, rows, footer — and the row-menu, popover, and drag visuals, in `crates/mbv-gui/ui`, plus the queue-local canvas tokens they need. No core, Owner-side, wire, or keybind change; the one Rust edit is the GUI-only destructive flag (design DR6).
- **Rows already merged stay landed:** the shared formatters (U1: 24-hour clocks, remaining count, `+1d` suffix, U+2212 minus), the row ordinal (U2.1), and the player top half (U3.1–U3.5: header, still slot, title, scrubber, transport) remain in the branch and out of this change's new rows.
- **R0 reverts the two cut data units** the Svelte paint never reads: the footer `library_context` field (U2.2) and the rail `remaining_minutes` field (U2.3). Nothing in Svelte paints them.
- **Out of scope:** collapsed states, the player top half beyond the merged rows, the subtitle Offset stepper, chip and track-list suffixes, the window resize on `z` and `q`, the library-context footer line, and the rail minutes.
- The four canvas source files and the three gap audits stay committed inside this change; every task row cites canvas element names and gap IDs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the queue, row menu, popover, and drag visuals gain canvas paint clauses (header wording, one-line rows with ordinals, destructive tone, popover chrome and placement, hover chrome, GUI-local canvas tokens). The window-resize and Offset-row requirements, the panel-state-size requirement, the footer library-context scenario, the chip chevron and suffix clauses, and the theme-section palette clause are deleted from this delta; the base `gui-client` spec already requires the prefix, toast, and status projection they carried.

## Impact

- `crates/mbv-gui/ui`: `src/app.css` (queue-local `--canvas-*` tokens), `QueueHeader.svelte`, `QueuePanel.svelte`, `QueueFooter.svelte`, `QueueRowMenu.svelte`, `TrackChips.svelte`, and their `*.test.ts` files; `src/lib/bridge/types.ts` for the destructive flag.
- `src/app/gui_client_state.rs` (`RowMenuEntryProjection`), `crates/mbv-gui/src/bridge/frame/overlay.rs` (`GuiRowMenuEntry`), and `src/app/queue_footer_projection.rs` / `crates/mbv-gui/src/bridge/frame/playback.rs` only for the R0 revert — no new projection or wire field anywhere.
- `docs/architecture/gui-client.md`: the narrowed scope, the tokens, the GUI-only flag, and the paint-time hint glyph.
- The TUI's code and paint are unchanged: the shared type surfaces this change touches are the GUI-only menu projection, the reverted fields the TUI never painted, and the front end's own CSS. No Tauri, ctrl, Owner lifecycle, stored-state, or crash-affecting change: the front end stays a painter and the core keeps every interaction state.
