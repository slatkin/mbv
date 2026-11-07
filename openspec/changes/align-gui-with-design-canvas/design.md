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
- No change to the TUI — not its surfaces, bindings, key output, palette, or paint (the user's ruling: "THE TUI WILL NOT CHANGE FOR THIS"). Every row that adds a shared or additive type states its TUI check; the Offset row's `PlayerCommand` addition is the one owner-side addition, and the TUI has no caller or painter for it.

## Decisions

### DR1. The canvas files are the committed reference

`canvas/` holds the four source files verbatim and `audit/` the three gap lists verbatim. Task rows cite gap IDs (for example, main G19) and canvas element names, so one row plus those two directories is the whole brief an implementer needs. The archive of this change keeps both directories, so the reference survives.

### DR2. Shared types change first, and no TUI path reads them

The canvas forms carry through the types the frame reads. The caller graph (checked against the code for this change) shows that the TUI reads none of them:

| Change | Typed at | TUI path, verified from the caller graph |
| --- | --- | --- |
| Zero-padded 24-hour `HH:MM` wall clock | `fmt_wall_clock` (`mbv-ui-model`) | Its only callers are the two queue projections (`src/app/queue_summary_projection.rs`, `src/app/queue_playback_projection.rs`) |
| Remaining count = every non-active row | `QueueSummaryProjection.remaining` | `queue_summary_projection()` is called only by the GUI frame (`crates/mbv-gui/src/bridge/frame.rs`) |
| Episode badge separator `S4 · E1` | `episode_badge` (`queue_playback_projection.rs`) | `queue_playback_projection()` is called only by the GUI core thread (`crates/mbv-gui/src/core_thread.rs`) |
| Row-menu hint glyph `Enter` → `⏎` | `gui_client_state.rs` (GUI-only builder) | None: the builder is GUI-only |

The TUI paints no wall clock, no remaining count, no Ends-at value, and no episode badge from these sites today, so the changes reach no TUI output: the campaign constraint "The TUI does not change" holds. If a later change gives the TUI a reader for one of these types, that change isolates the GUI's form then.

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

The canvas paints `Remove from queue` in `#e67e80` under a divider (`menus` audit G2). `ContextMenuEntry` (`crates/mbv-ui-model/src/context_menu.rs`) gains a `destructive: bool`, the row-menu builder sets it for the remove action, `RowMenuEntryProjection` and `GuiRowMenuEntry` thread it, and the front end paints the tone and the divider. The flag is additive: the TUI's construction sites set it `false` and its menu painter reads nothing new, so no TUI output changes (the campaign constraint: the TUI does not change).

### DR9. Tests name a contract or do not exist

Each unit's rows name the contract and its layer. Visual claims are asserted as values in vitest (the front end's tests already assert painted structure); projection claims are asserted in the owning crate's tests. No test inflation: a paint row reuses the component's existing render test unless the row adds a new state.

## The canvas decisions (user ruling, 2026-11-07)

The user's ruling (quoted): "there ARE [...] DESIGN SPECS, EVERYTHING SHOULD BE IN THERE." The canvas is the design artifact; every design question reads its answer in `canvas/`, so no decision of this change stands open. Three former open decisions are resolved by it:

### (a) The subtitle Offset stepper: built end to end, as the canvas shows it

The canvas paints the full Offset row and the popover's footer hint (`menus` audit G1, G5): the `−` and `+` steppers, the offset value, and the hint `c · cycle   [ ] · offset`, painted inside the open popover only. The unit builds the whole path: the core state, the player command and status fact, the wire fact, the paint, and the hint's `[` and `]` segments, which act on the offset while the popover is open and leave the seek chords unchanged everywhere else. Nothing of it paints or routes outside the open popover.

### (b) The chip and track-list suffixes: derived and painted

The canvas labels the chips and the popover rows with their stream suffixes (`menus` audit G10). The unit derives the facts the canvas labels assert and paints the canvas label forms.

### (d) Two palettes: the GUI's theme section resolves the canvas hexes; the TUI's palette stays

The GUI's theme section (`crates/mbv-gui/src/bridge/frame/theme.rs`, `GuiTheme::current`) resolves the roles to the canvas hexes; `mbv-theme`'s slot table and `Theme::DEFAULT` stay untouched as the TUI's palette. This amends the parent change's D7 ("mbv-theme's surface table is the one fill authority"; "shadcn-svelte components therefore paint in the mbv palette with no second palette") for the GUI Client only: the GUI's fill and role authority is the canvas palette, carried in the GUI's theme section.

## Risks / Trade-offs

- [The front end's resize measurement can thrash if a browser reports subpixel sizes] → The remembered size is snapshotted only on a transition, and resizes run only from transitions.
- [A reload while collapsed restores the collapsed state's own natural size instead of the remembered one] → Accepted for this change; persisting the window size is later work.
- [Some canvas colours name no theme slot today (chapter ticks, the segment oranges)] → The resolved palette decision (d) carries the colour layer in the GUI's theme section; no paint row hardcodes a hex, and `mbv-theme`'s slot table stays the TUI's palette.

## Migration Plan

Units land in order: shared types (U1, U2), the expanded artboard (U3, U4), the collapsed states (U5), menus and drag (U6, U7), the window resize (U8), docs (U9), then the canvas decisions' units: the Offset stepper (U10, U11), the chip and track-list suffixes (U12), and the GUI-local palette (U13). Rollback is to revert the units; U10 is the one owner-side addition (one `PlayerCommand` variant and one status fact), and no stored state changes.
