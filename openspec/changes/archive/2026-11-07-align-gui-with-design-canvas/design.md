# Design: align-gui-with-design-canvas

## Context

The GUI Client was built to TUI parity and then restyled, but the gap audits against the canvas artboards (`audit/{main,collapsed,menus}.md`, copied into this change) still list divergences. The canvas files (`canvas/*.dc.html`, `canvas/canvas.json`) are the spec: their inline CSS values, strings, and ordering are normative.

The user's binding rulings (2026-11-07) narrow this change: **only the QUEUE is brought in line with the canvas, and no new functionality lands** — no new player commands, data derivations, resize, or keys; the canvas sample text is mock content. The rows already merged (U1 shared formatters, U2.1 row ordinal, U3.1–U3.5 player top half) stay landed. The two data units the narrowing cuts (U2.2 footer library context, U2.3 rail compact minutes) are reverted by R0. Collapsed states, the Offset stepper, the chip suffixes, and the window resize are out of scope.

What the code holds today:

- The front end paints from the frame's role map and chrome fills (`frame/theme.rs`, `theme.ts`); no literal hex reaches a component, and several canvas hexes have no `mbv-theme` slot.
- The merged rows already paint the top half and carry the queue ordinal on the frame.
- The queue header, rows, and footer still paint divergences from the canvas (main audit 16–25, 27–29).
- Row-menu entries carry `label` + `key_hint` (`gui_client_state.rs`, the GUI-only builder); no destructive flag exists; `direct_select` (`gui_client_state.rs:181`) matches the raw hint string.
- The popover opens `bottom-full`, its chrome diverges from the Menus artboard, the open chip carries no expanded state, and the drag ghost, insertion line, and hover chrome diverge (menus G3, G4, G6, G8, G9, G11).

## Goals / Non-Goals

**Goals:**

- The GUI queue paints the canvas values exactly: geometry, glyphs, type sizes, colours, strings, and aria labels.
- The row menu, popover, and drag visuals paint the Menus artboard, and the destructive entry is tone-distinct.
- No new functionality: the change paints only what the projections already carry.

**Non-Goals:**

- The TUI does not change — not its surfaces, bindings, key output, palette, or paint (user ruling). Every row states the check; no row touches a file the TUI paints from.
- No collapsed states, no player-top-half work beyond the merged rows, no window resize, no Offset stepper, no chip suffixes, no footer library context, no rail minutes.
- No redesign of the interaction authority: the core keeps every toggle, cursor, menu, drag, and filter; the front end stays a painter.

## Decisions

### DR1. The canvas files are the committed reference

`canvas/` holds the four source files verbatim and `audit/` the three gap lists verbatim. Task rows cite gap IDs (for example, main 17) and canvas element names, so one row plus those two directories is the whole brief an implementer needs. The archive of this change keeps both directories, so the reference survives.

### DR2. The shared-type changes are the merged rows, and no TUI path reads them

The canvas forms that needed shared types already landed. The caller graph (checked against the code) shows the TUI reads none of these sites:

| Change | Typed at | TUI path, verified from the caller graph |
| --- | --- | --- |
| Zero-padded 24-hour `HH:MM` wall clock | `fmt_wall_clock` (`mbv-ui-model`) | Its only callers are the two queue projections (`src/app/queue_summary_projection.rs`, `src/app/queue_playback_projection.rs`) |
| Remaining count = every row without the playhead | `QueueSummaryProjection.remaining` | `queue_summary_projection()` is called only by the GUI frame (`crates/mbv-gui/src/bridge/frame.rs`) |
| Row ordinal, 1-based queue position | `GuiQueueRow::Item.ordinal` (`crates/mbv-gui/src/bridge/frame/queue.rs`) | The bridge lives in the GUI-only crate |
| Episode badge separator `S4 · E1` | `episode_badge` (`queue_playback_projection.rs`) | `queue_playback_projection()` is called only by the GUI core thread (`crates/mbv-gui/src/core_thread.rs`) |

If a later change gives the TUI a reader for one of these types, that change isolates the GUI's form then. No new shared-type change is ahead: the destructive flag stays GUI-only (DR6) and the cut fields are reverted (DR4).

### DR3. The `+1d` overflow suffix is computed at the projection and carried on the one value the canvas suffixes

The canvas script appends `' +1d'` to the queue finish clock only (`Main.dc.html` `renderVals`, `queueEnds`); row ends, the scrubber's end value, and the current-row end carry no suffix. The projections compute the suffix from the finish epoch against local midnight, and `QueueSummaryProjection` carries the suffixed string. The front end paints no arithmetic. (Landed with U1.4.)

### DR4. Unit A paints with no new data and the cut fields are reverted

The queue header, rows, and footer paint from facts the frame already carries: the merged ordinal, the remaining-count projection, the playlist/source text, and the finish clock. No new projection, wire field, or derivation. The U2.2 `library_context` and U2.3 `remaining_minutes` fields painted nothing in Svelte (nothing reads them), so R0 reverts them to the pre-`5f3f36546` shape instead of carrying dead wire data. `docs/architecture/gui-client.md` keeps listing them nowhere: the doc line for this change records the narrowed scope.

### DR5. The footer paints the queue's existing source text

The canvas footer's left line is `Peep Show · Series 1–6` (main audit 27) — a library-context line the narrowing does not build. Unit A paints the footer's canvas chrome (40 px height, `0 16px` padding, `1px solid #3d484d` border-top, mono 12 px) around the footer's existing content: the left line is the playlist/source text the frame already carries, without its leading icon, and the owner marker paints label then dot, also without its icon (main audit 28). The prefix, toast, autosave, and dirty states keep painting when they are active — the base `gui-client` spec requires them — in the canvas footer's type and spacing.

### DR6. The row menu's destructive flag is GUI-only and derives from the action

The canvas paints `Remove from queue` in `#e67e80` under a divider (menus G2). The flag lands on the GUI's own projection types: `RowMenuEntryProjection` (`src/app/gui_client_state.rs:42`) and `GuiRowMenuEntry` (`crates/mbv-gui/src/bridge/frame/overlay.rs:79`) each gain one `destructive: bool`, derived in the GUI overlay projection from `matches!(action, Some(ContextAction::RemoveFromQueue(_)))` — from the entry's action, not from a label or a position. `ContextMenuEntry` (`crates/mbv-ui-model/src/context_menu.rs`) is untouched, the TUI's construction sites and menu painter are untouched, and `crates/mbv-render` reads nothing new: no TUI output changes (the campaign constraint). No keyed painter exists; the tone and the divider always follow the projected flag.

### DR7. The row menu's `⏎` hint maps at paint time only

The canvas hints `⏎` where the core sends `Enter` (menus G5). The map lives in `QueueRowMenu.svelte` at paint time. The GUI row-menu builder and `direct_select` (`gui_client_state.rs:181`) keep matching the raw `Enter` hint string, so menu key routing follows the configured bindings unchanged; no key route changes, and the TUI's keybinds are untouched.

### DR8. The popover opens below the chip row and carries no divider

The popover opens below the chip row, right-aligned (Menus artboard), replacing today's `bottom-full` open-up. The open chip paints the expanded state of menus G3 and nothing else — no restyle, no dividers: the canvas's only popover divider separates the Offset row, which is out of scope.

### DR9. Canvas hexes with no `mbv-theme` slot resolve through GUI-local `app.css` tokens

The queue, row menu, popover, and drag visuals paint the canvas hexes through queue-local `--canvas-*` custom properties in `crates/mbv-gui/ui/src/app.css`: `#3d484d #475258 #7f897d #56635f #414b50 #7d9460` (main audit 17, 19–25) plus the menu, popover, and drag-ghost chrome they consume (menus G2, G4, G6, G8). No `theme.rs` global remap — that former decision (d) narrows to the token file — and `mbv-theme`'s slot table and the TUI's palette stay untouched, still amending the parent change's D7 for the GUI Client only. No paint row hardcodes a hex; every paint row references a token.

## Risks / Trade-offs

- [The panel background stays on the theme role, `#272e33`, while the canvas paints `#2d353b`] → Accepted as the open risk of the tokens decision: the panel background keeps its `mbv-theme` slot. The user live-checks after Unit A.
- [Some canvas colours name no theme slot today (the segment fills, muted ordinals, menu and ghost chrome)] → Carried by the GUI-local tokens (DR9); no paint row hardcodes a hex, and `mbv-theme` stays the TUI's palette.

## Migration Plan

Units land in order: R0 (revert the cut fields), Unit A (queue header, rows, footer paint plus the `app.css` tokens), Unit B (destructive flag, row menu, hover, popover, drag, and the one doc line). Rollback is per-unit revert; no core, Owner-side, or stored-state change exists in any unit.
