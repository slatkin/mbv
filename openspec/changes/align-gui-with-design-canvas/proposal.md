# Proposal: align-gui-with-design-canvas

## Why

The GUI Client landed close to the design canvas but not on it. Gap audits of the three artboards against the built Svelte UI record about 60 divergences (missing elements, wrong painted values, wrong behaviour, and data the wire does not carry). The parent change's ruling stands: where the canvas disagrees with the current UI or the TUI, the canvas wins (`.handoffs/add-tauri-gui-client-artifacts/proposal.md`, "What Changes"). This change makes the canvas the governing spec for the GUI panel and plans the alignment as small units.

## What Changes

- Make the design canvas the committed, normative reference for the GUI Svelte UI (`crates/mbv-gui/ui`). The four canvas source files and the three gap audits ship inside this change and every task row cites their element names and gap IDs.
- Align every painted value with the canvas: header, artwork slot, title, scrubber (position line and trickplay preview), transport, track chips, queue header, queue rows, footer, the three collapsed states, and the popover, row-menu, and drag visuals.
- Amend the shared projections and formatters the frame reads: zero-padded 24-hour clocks, the `+1d` overflow suffix on the queue finish clock, the remaining count as the canvas counts it, a 1-based row ordinal, the footer's library-context line, and a compact-minutes remaining fact for the rail. Some of these show in the TUI; design.md records each consequence.
- Add the `destructive` flag to the context-menu entry type and paint the row menu's Remove-from-queue entry in the canvas's destructive tone.
- Resize the window with the toggles, per the user's ruling: `z` collapse shrinks the window to the collapsed state's natural size and expanding restores the remembered pre-collapse size (mini-player behavior); `q` fold shrinks the window by the queue section's height and unfolding restores.
- Two more alignments are planned but blocked on open decisions in design.md: the subtitle Offset stepper, and the track-chip and track-list label suffixes. A third, the exact canvas palette values against the theme's default palette, is also a decision, because the theme's default palette and the canvas share three colours and differ on the rest.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the panel states get a precedence rule with the window-resize ruling; the player panel, the queue, the track chips, the scrubber, and the typography gain canvas conformance clauses (clock format, minus sign, row ordinals, header wording, destructive menu tone, footer library-context line).

## Impact

- `crates/mbv-ui-model` (clock formatter), `src/app/queue_summary_projection.rs` and `src/app/queue_playback_projection.rs` (clock, remaining count, overflow suffix, compact minutes), `crates/mbv-ui-model/src/context_menu.rs` and the row-menu builder (destructive flag, hint glyphs), `crates/mbv-gui/src/bridge/` (new frame fields), and most `crates/mbv-gui/ui/src/lib/components/` files.
- `crates/mbv-gui/capabilities/default.json`: a new window-size permission for the resize command path.
- The TUI sees three value changes through the shared types: the wall-clock format, the remaining count, and the row-menu hint glyphs where the TUI paints its own menu. design.md records one alternative for each and the reason the shared type wins.
- No Tauri, ctrl, Owner, mpv, or crash-affecting change: the front end stays a painter and the core keeps every interaction state.
