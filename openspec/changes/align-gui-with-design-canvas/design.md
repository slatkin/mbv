# Design: align-gui-with-design-canvas

## Context

The GUI Client was built to TUI parity (`.handoffs/add-tauri-gui-client-artifacts/tasks.md`, groups 3 and 6) and then restyled, but the gap audits against the canvas artboards (`audit/{main,collapsed,menus}.md`, copied into this change) still list about 60 divergences. The canvas files (`canvas/*.dc.html`, `canvas/canvas.json`) are the spec: their inline CSS values, strings, and ordering are normative.

What the code holds today:

- The front end paints from the frame's role map and chrome fills (`frame/theme.rs`, `theme.ts`); no literal hex reaches a component. The theme is `mbv-theme`'s `Theme::DEFAULT`, a closed slot table.
- Clocks are preformatted strings from `fmt_wall_clock` (`mbv-ui-model/src/ui_util.rs:216`), 12-hour with an AM/PM marker. The queue finish clock is a projection value (`queue_summary_projection.rs`), as are per-row ends and the scrubber's finish value.
- The panel state is picked in `bridge/panel.ts` from the measured container size plus the core's two toggles; the collapse toggle ranks first, then the short-container check, then the queue fold.
- The frame's queue rows carry no ordinal, the footer carries the playlist pill rather than a library-context line, and the rail's remaining time paints the scrubber-format clock.
- Row-menu entries carry `label` + `key_hint` (`gui_client_state.rs`, the GUI-only builder); no destructive flag exists anywhere.
- The Tauri capability file (`capabilities/default.json`) allows `key`, `pointer`, `report_layout`, and the event listener. No window-size grant exists.

## Goals / Non-Goals

**Goals:**

- The GUI panel paints the canvas values exactly: geometry, glyphs, type sizes, colours (through roles the canvas values resolve to), strings, and aria labels.
- The shared types carry the data the canvas paints (ordinals, footer context, compact minutes, overflow suffix, destructive tone).
- The window follows the user's resize ruling for `z` and `q`.

**Non-Goals:**

- No redesign of the interaction authority: the core keeps every toggle, cursor, menu, drag, and filter; the front end stays a painter.
- No change to ctrl, the Owner, mpv, or the TUI's surfaces.
- No subtitle Offset stepper and no track-label suffixes while their decisions (below) stand.

## Decisions

### DR1. The canvas files are the committed reference

`canvas/` holds the four source files verbatim and `audit/` the three gap lists verbatim. Task rows cite gap IDs (for example, main G19) and canvas element names, so one row plus those two directories is the whole brief an implementer needs. The archive of this change keeps both directories, so the reference survives.

### DR2. Shared types change first, and the TUI-visible consequences are recorded

Three canvas values arrive through types the TUI also reads:

| Change | Typed at | TUI-visible consequence |
| --- | --- | --- |
| Zero-padded 24-hour `HH:MM` wall clock | `fmt_wall_clock` (`mbv-ui-model`) | The TUI's finish clocks lose the AM/PM marker |
| Remaining count = every non-active row | `QueueSummaryProjection.remaining` | The TUI's remaining count rows become the same count |
| Row-menu hint glyph `Enter` → `⏎` | `gui_client_state.rs` (GUI-only builder) | None: the builder is GUI-only |

*Alternative:* a GUI-only formatting or counting layer. Rejected: it puts a second format for one concept in one codebase, and the frame's contract is "each field mirrors one fact of the library's projection". One formatter and one count stay, the canvas's form wins, and the TUI prints shorter clocks. If the user rejects a TUI-visible clock change, the fallback is the alternative and the delta scenario wording holds for both.

### DR3. The `+1d` overflow suffix is computed at the projection and carried on the one value the canvas suffixes

The canvas script appends `' +1d'` to the queue finish clock only (`Main.dc.html` `renderVals`, `queueEnds`); row ends, the scrubber's end value, and the current-row end carry no suffix. The projections compute the suffix from the finish epoch against local midnight (the finish epoch belongs to the next day), and `QueueSummaryProjection` carries the suffixed string. The front end paints no arithmetic.

### DR4. The panel-state precedence puts the toggles above the container measurement

Today the short-container check outranks the queue fold (`panel.ts:panelState`). The resize ruling of DR5 breaks that order: a `q` fold can shrink the window below the 520 px strip threshold, and a measured strip must not then replace the fold the user asked for. New precedence, one line each: the collapse toggle, then the queue-fold toggle, then the container measurement. The canvas is silent on overlap (`audit/collapsed.md` G0), so the ruling's mechanics pick the order. This kills both feedback loops: neither toggle's own window shrink can override the state the toggle set, and no measurement ever drives a state change.

### DR5. The window resizes on a toggle transition; the core stays the intent authority; the front end measures and applies

User ruling (quoted): "z collapse -> yes, shrink the window to the collapsed state's natural size; expanding restores the remembered pre-collapse size (mini-player behavior)." and "q queue fold -> yes, shrink by the queue section's height; unfold restores."

- The core keeps its authority: `z` and `q` set the two toggles only, as today. No window knowledge reaches the core, and no core field changes.
- The front end compares the previous frame's toggles with the new frame's and issues a window resize only on a transition, never from a container measurement. A transition to collapsed or folded shrinks; a transition out restores the remembered size.
- The remembered size is the last container measurement the front end observed while the panel painted the fully expanded state. It is a layout measurement, not interaction state (the spec already separates them), and the front end already measures and reports that size in `LayoutReport`.
- The natural sizes are DOM measurements: the rail's band, and the queue-section height for the fold. The rail paints first at the old window size (the toggle outranks the measurement, DR4), its natural size is measured, then the window resizes.
- Mechanics: the capability file gains `core:window:allow-set-size`, and the front end calls `getCurrentWindow().setSize(new LogicalSize(...))`. No new Rust command and no new core field. *Alternative:* a dedicated `resize_window` Tauri command. Rejected: the command's only work would be `window.set_size`, for one more permission plus one more registered command.
- A reload while collapsed loses the remembered size (the measurement lives in the front end). Restoring then falls back to the collapsed state's own natural size. Risk section records it; persisting a window size is a later change's work.

### DR6. The footer gains the canvas's library-context line and keeps its projection extras

The canvas footer's left line is the queue's library context: `Peep Show · Series 1–6` (`main` audit 27). The frame's footer gains a `library_context` field built in `QueueFooterProjection` from the queued items of the active series: the series name and the season range joined with an en dash; `None` when the queue's items carry no seasons (music, feeds, mixed queues). The playlist pill paints in that line's slot as the fallback, and the autosave, dirty, prefix, and toast facts keep painting when they are active, in the canvas footer's type and spacing — the canvas artboard paints a quiet footer, and the spec's footer-line clause (prefix armed, toasts) stays in force. The right marker keeps its status text and dot and loses its leading icon (`main` audit 28).

### DR7. The video chip paints the canvas chevron and still opens nothing

The canvas paints a chevron on all three chips (`main` audit 15). The parent change's ruling D4a (the video chip shows the resolved label only and opens no popover) made it label-only; the canvas wins on the painted glyph, the ruling stands on the press, so the chip paints the chevron and its press is a documented no-op.

### DR8. The row menu's destructive entry is flagged at the type

The canvas paints `Remove from queue` in `#e67e80` under a divider (`menus` audit G2). `ContextMenuEntry` (`crates/mbv-ui-model/src/context_menu.rs`) gains a `destructive: bool`, the row-menu builder sets it for the remove action, `RowMenuEntryProjection` and `GuiRowMenuEntry` thread it, and the front end paints the tone and the divider. The TUI can adopt the flag later; nothing forces it to.

### DR9. Tests name a contract or do not exist

Each unit's rows name the contract and its layer. Visual claims are asserted as values in vitest (the front end's tests already assert painted structure); projection claims are asserted in the owning crate's tests. No test inflation: a paint row reuses the component's existing render test unless the row adds a new state.

## Risks / Trade-offs

- [The TUI recolours and reformats where the shared types change (DR2)] → Recorded per change with a fallback; the user reviews this change before implementation.
- [The front end's resize measurement can thrash if a browser reports subpixel sizes] → The remembered size is snapshotted only on a transition, and resizes run only from transitions.
- [A reload while collapsed restores the collapsed state's own natural size instead of the remembered one] → Accepted for this change; persisting the window size is later work.
- [Some canvas colours name no theme slot today (chapter ticks, the segment oranges)] → The palette decision (below) resolves the whole colour layer before any paint row pins a literal; no paint row hardcodes a hex.

## Migration Plan

Units land in order: shared types (U1, U2), the expanded artboard (U3, U4), the collapsed states (U5), menus and drag (U6, U7), the window resize (U8), docs (U9). The blocked units follow their decisions. Rollback is to revert the units; no stored state or protocol changes.

## Open decisions

Each is a Yes/No question. None is decided here.

### (a) The subtitle Offset stepper — yes or no?

The canvas paints an Offset row with a `−/+0.25s` stepper and the hint `c · cycle   [ ] · offset` (`menus` audit G1, G5). Facts: no subtitle-delay state exists in the core, the player, or the wire (the gap audit searched `src/` and `crates/mbv-player`); the building plan needs a new core state, a new player command and status field, and popover-only key routing; and the canvas's `[` / `]` chords are today's seek chords (-10 s / -30 s), so the open popover would have to shadow or rebind them (`menus` audit G12). Answer yes: the blocked unit builds the whole stepper, including that key decision. Answer no: the popover paints choices only, the hint paints `c · cycle`, and the canvas's offset row is recorded as a declined ruling.

### (b) The track-chip and track-list suffixes — yes or no?

The canvas labels chips `FHD · direct`, `EN 2.0`, `CC EN` and popover rows `English · SRT`, `English SDH · embedded` (`menus` audit G10). Facts: nothing in the wire carries a source-medium, channel-count, or subtitle-format fact; mpv's track labels may or may not yield the suffixes, which only a live player shows, and the live test is the user's (with their approval). Answer yes: the unit adds derived facts (Emby `MediaSourceInfo.media_streams`, mpv `track-list` fields the status does not yet project) and paints the canvas labels. Answer no: labels stay today's names and the gap is recorded as declined.

### (d) The canvas palette values — the theme's default palette or GUI-local values?

The canvas's 19 distinct hexes share 3 with the theme's `docs/palette.json` (`#232a2e`, `#a7c080`, `#dbbc7f`); the rest differ (checked this change's naming: `#e59875` vs the canvas `#e69875`, `#e57e80` vs `#e67e80`, `#2b3238` vs `#2d353b`, and similar). Answer yes (edit `Theme::DEFAULT` and the slot table to the canvas values): one palette, D7's one-fill-authority stands, and the TUI recolours to the canvas palette. Answer no: the GUI's theme section (`GuiTheme`) resolves the roles to the canvas's hexes and the TUI's palette stays, which amends D7 to two palettes. The blocked unit lands the choice wherever it goes.
