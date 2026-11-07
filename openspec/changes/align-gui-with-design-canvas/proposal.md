# Proposal: align-gui-with-design-canvas

## Why

The GUI Client landed close to the design canvas but not on it. Gap audits of the three artboards against the built Svelte UI record about 60 divergences (missing elements, wrong painted values, wrong behaviour, and data the wire does not carry). The parent change's ruling stands: where the canvas disagrees with the current GUI UI, the canvas wins (`.handoffs/add-tauri-gui-client-artifacts/proposal.md`, "What Changes"). The user's campaign ruling adds the constraint this change works under: the TUI does not change — the canvas governs the GUI panel only. This makes the canvas the governing spec for the GUI panel and plans the alignment as small units.

## What Changes

- **The TUI does not change.** No row touches TUI code or output; every row that adds a shared or additive type states its TUI check, verified from the caller graph (design DR2).
- Make the design canvas the committed, normative reference for the GUI Svelte UI (`crates/mbv-gui/ui`). The four canvas source files and the three gap audits ship inside this change and every task row cites their element names and gap IDs.
- Align every painted value with the canvas: header, artwork slot, title, scrubber (position line and trickplay preview), transport, track chips, queue header, queue rows, footer, the three collapsed states, and the popover, row-menu, and drag visuals.
- Carry the canvas forms through the projections and formatters the frame reads: zero-padded 24-hour clocks, the `+1d` overflow suffix on the queue finish clock, the remaining count as the canvas counts it, a 1-based row ordinal, the footer's library-context line, and a compact-minutes remaining fact for the rail. The caller graph shows the TUI reads none of these sites; the TUI's paint is untouched.
- Add the `destructive` flag to the context-menu entry type and paint the row menu's Remove-from-queue entry in the canvas's destructive tone.
- Resize the window with the toggles, per the user's ruling: `z` collapse shrinks the window to the collapsed state's natural size and expanding restores the remembered pre-collapse size (mini-player behavior); `q` fold shrinks the window by the queue section's height and unfolding restores.
- Build the canvas's subtitle Offset stepper end to end: a core state, a player command and status fact, the wire fact, the popover paint, and the hint's `[` and `]` segments acting on the offset while the popover is open (the seek chords are unchanged outside it).
- Derive and paint the canvas's chip and track-list label suffixes, and resolve the GUI's colours in its theme section to the canvas palette's hexes; `mbv-theme` and the TUI's palette stay untouched.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the panel states get a precedence rule with the window-resize ruling; the player panel, the queue, the track chips, the scrubber, and the typography gain canvas conformance clauses (clock format, minus sign, row ordinals, header wording, destructive menu tone, footer library-context line, offset row, suffix labels, GUI-local palette).

## Impact

- `crates/mbv-ui-model` (clock formatter), `src/app/queue_summary_projection.rs` and `src/app/queue_playback_projection.rs` (clock, remaining count, overflow suffix, compact minutes), `crates/mbv-ui-model/src/context_menu.rs` and the row-menu builder (destructive flag, hint glyphs), `crates/mbv-gui/src/bridge/` (new frame fields), most `crates/mbv-gui/ui/src/lib/components/` files, and, for the Offset stepper, the owner-side player path (one `PlayerCommand` variant and one status fact).
- `crates/mbv-gui/capabilities/default.json`: a new window-size permission for the resize command path.
- The TUI's code and paint are unchanged. The clock, count, suffix, and badge values land in sites the caller graph shows have no TUI reader (design DR2), the context-menu flag is additive with no TUI painter (design DR8), and the Offset command and status fact have no TUI caller, binding, or paint.
- No Tauri, ctrl (beyond the Offset command's one variant), Owner lifecycle, or crash-affecting change: the front end stays a painter and the core keeps every interaction state.
