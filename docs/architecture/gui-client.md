# GUI Client frontend guide

Read before any GUI Client change: a painter, input translation, breakpoint,
theme role, key, or pointer gesture — including one-line tweaks. Term
definitions live in `CONTEXT.md` under Presentation (*TUI Client*, *GUI
Client*, *Client core*, *Surface state*, *Breakpoint class*); they win any
disagreement with this file. Behavioural authority is the `gui-client` spec
(`openspec/specs/gui-client/spec.md`; while `add-gui-front-end` is open, its
delta overlays it). The TUI-side rules are in `tui-frontend.md` and are not
repeated here.

## Where GUI painters live

The GUI Client is the `crates/mbv-gui/` program: an eframe window whose
`update` hook runs the shared `ClientCore::tick` and then paints. It links
the `mbv` library for the shell `App`, the dispatch, the keyboard router,
and the Owner find/start/attach steps; the `mbv` program itself carries no
windowing or GPU stack.

| File | Owns |
| --- | --- |
| `crates/mbv-gui/src/tick_host.rs` | `GuiTickHost`: the Client-owned half of the tick, the per-frame breakpoint classification, and the pointer-gesture state |
| `crates/mbv-gui/src/input.rs` | the one raw-input translation site: egui `RawInput` to neutral `KeyEvent`/`PointerEvent` |
| `crates/mbv-gui/src/queue.rs` | the Queue panel painter over `QueueSurface` |
| `crates/mbv-gui/src/queue_playback.rs` | the Queue playback band painter over `QueuePlaybackSurface` |
| `crates/mbv-gui/src/status_bar.rs` | the Status bar painter over `StatusBarSurface` |
| `crates/mbv-gui/src/headless.rs` | the headless frame harness for tests |

Each panel painter is the GUI counterpart of a TUI adapter in
`crates/mbv-components/`: it paints from the same Surface state in
`mbv-surfaces`, reports each frame's capacity and `HitMap` from its own
painted geometry, and holds no interaction state of its own — no stateful
egui widget exists in a painter, because scroll, cursor, marks, selection,
and transport focus belong to Surface state. Every colour resolves through
the theme's neutral `Rgb`, never a literal.

## The interaction contract

Each rule below is a requirement of the `gui-client` spec. A GUI change that
touches any of them must keep the rule, and the rule lives in the Client
core, not in `mbv-gui`.

1. **Separate program.** The GUI Client is its own program; the `mbv`
   program links no windowing or GPU library.
2. **Client of the Owner.** The GUI finds, starts, and attaches to the Owner
   process through the same library code as a TUI Client. It holds no owner
   lock and never becomes a Player owner.
3. **No playback in the window.** The GUI never starts, embeds, or controls
   mpv. Playback and queue requests go to the Owner over ctrl, and video
   plays in the mpv child process window.
4. **First slice surfaces.** The GUI presents the Queue, the Queue playback
   panel, and the Status bar, with the same content, Panel arrangement, and
   states as the TUI in the queue-only Panel mode. A surface the GUI does
   not yet present stays absent — never a stub or placeholder.
5. **Keyboard parity.** For every presented surface, each chord has the same
   effect as in the TUI Client, through the same `[keys]` configuration and
   the same keyboard router.
6. **One visible focus.** Exactly one focused surface and one focused item
   at all times. Focus moves only by a key or a pointer press; hover never
   moves it.
7. **Keyboard reachability.** Every action has a key chord. Nothing is
   pointer-only, and no affordance appears only on hover.
8. **Scroll follows the cursor.** A list keeps the cursor row fully visible
   after every keyboard movement, with fixed row heights so capacity is a
   whole number of rows.
9. **Motion never delays input.** Animation is feedback only; a key press
   takes effect on the next frame.
10. **Pointer resolves through semantic targets.** A press or wheel step
    resolves to the semantic target painted at that point in the current
    frame, against the last completed frame's `HitMap`, with the same
    effect as the matching TUI gesture. A press on no target changes
    nothing.
11. **Breakpoint from the window.** The layout breakpoint class derives from
    the window width alone, through the shared `classify` (below).

## GUI code holds no interaction rule

Each interaction rule exists in the Client core one time — in Surface state
(`mbv-surfaces`), the `FocusStack`, the gesture policy, and the keyboard
router — and each Client only translates input and paints. Add a cursor,
selection, paging, focus, or gesture rule to `mbv-surfaces`, never to
`mbv-gui`. This is the GUI mirror of the TUI rule that an adapter holds no
interaction rule; a painter holds even less, because it translates nothing.

No compiler gate enforces this. The review checks are: an interaction
decision (cursor move, selection change, paging stride, focus change,
gesture recognition) written in `mbv-gui` instead of the core; a second copy
of a value its Surface state owns; hit-test arithmetic that no longer
matches painted geometry; and shell state (`App` internals, a Service
client, `Config`, a channel) reaching a painter. `GuiTickHost` holds the
`App` and the frame's raw input — it is the Client edge, not a painter.

## Breakpoint class

The GUI classifies the window's logical-point width through
`mbv_surfaces::breakpoint::classify` before each frame, with
`GUI_BREAKPOINTS` in `tick_host.rs`: `mini_below: 640.0`,
`wide_from: 960.0`. The values are a presentation choice (design D10 of
`add-gui-front-end`), scaled from the TUI's 80-column mini view and
120-column tuned width at about 8 points per column; the spec pins the
mechanism, not the numbers. The class drives the same layout behaviour as
the TUI applies in the same class.

The class is computed and held per frame (and readable in tests through
`breakpoint_test_class`), but the first slice does not yet apply it to an
arrangement: the Queue, Queue playback, and Status bar painters compose a
fixed layout. Wiring the class into arrangement choice is later work, and
until then the TUI keeps its own column thresholds and does not call
`classify` either.

## Known arrangement divergences

Recorded in design.md of `add-gui-front-end` so archive-time spec sync does
not read them as misses:

- The Queue playback title paints on the transport title row, not inside
  the header row. The spec carries it on the header row
  (`NowPlayingTitleSite::Header`); the GUI paints the same projected content
  one row down, because the GUI owns its geometry.
- The status word and the footer pills paint in regular weight: the bundled
  fonts expose no bold face, so the TUI's BOLD spans have no GUI
  counterpart.
- The playback-target indicator in the header row is not clickable; the
  Sessions surface is outside this change's first slice.

## Tests

GUI composition tests run the headless harness (`headless.rs`): one egui
`Context::run` over synthetic `RawInput`, with no window and no GPU. They
assert core state and delivery, never appearance. Each input translator has
one `#[case]` table of its mapping.
