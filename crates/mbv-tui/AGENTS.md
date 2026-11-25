# mbv-tui Agent Guide

`crates/mbv-tui/` is the TUI Client: the `mbv-tui` program. It links
`mbv-client-core` for the shared state and rules, and it never links
`mbv-gui`. Read the root `AGENTS.md` first for the workspace, the shared
core, the coding standards, and the test rules; this guide holds what applies
to the TUI edge.

## Crate map

- `src/main.rs` — the thin process entry: the mimalloc global allocator, then
  the library's `run`.
- `src/lib.rs` and the modules beside it — the program's own policy:
  - `startup` — argument handling, configuration, `--pin`, `-q`, `--toggle`,
    the Client-side Owner link, and the TUI Client run. The Client starts the
    Owner program (`mbv-owner`) beside this program, else on `PATH` (ADR 0044).
  - `pin` — the `--pin` launch, its failure reporting, and the running-panel
    toggle. The panel identity and every running-panel request live in the
    core's `pin` module; this module builds the launch argv from the core's
    `pub` parts.
  - `owner_restart` — the terminal prompt for an Owner build mismatch, over
    the core's pure `follow_up` decision.
- `src/tui.rs` + `src/tui/` — the shell `Model` and the TuiRealm
  `Application`: mount and focus, runtime lifecycle, projections, dispatch,
  effects, the single tick and draw path (`run.rs`, `draw.rs`), the terminal
  input edge (`neutral_edge`, `input_port`), the image-protocol encoding
  (`image_protocol`), and the TUI hero projection
  (`hero_image_projection`).

The TUI-only crates are `mbv-components` (TuiRealm Interactive Components),
`mbv-render` (the visual substrate), and `mbv-tui-support` (theme colour
conversion and the terminal image protocol). The compiler keeps them out of
the core and the GUI Client.

## The Model and the core

- The shell `Model` owns `App` and the TUI-only state: the terminal size, the
  composed `AppLayout`, the tab-strip scroll, the context menu's pointer
  cell, and the `ImageProtocols` store. Every shared interaction rule lives in
  `mbv-client-core`.
- Terminal geometry and pointer cells never reach the core. The shell pushes
  neutral facts: the single-panel fact at every point where it records the
  terminal size, the left panel area from the composed frame, the tab-reveal
  request, and the context menu's neutral anchor.
- A component or painter never names `App`, a Service client, credentials,
  `Config`, `PlayerProxy`, a protocol object, a lock, or a channel. Components
  receive typed projections and send typed `Msg`s (`mbv-ui-msg`). Their crates
  sit below this one, so a break of that boundary is a compile error.

## Interactive architecture

1. Use one owner and one painter per surface per breakpoint. A surface that
   both Clients present is an adapter over its Surface state in
   `mbv-surfaces`; it forwards neutral events, paints from that state, and
   reports geometry. It holds no second copy of a value the Surface state
   owns.
2. Components never receive `App`, Service clients, credentials, `Config`,
   `PlayerProxy`, protocol objects, integration locks, or channels. Messages
   carry semantic intent and stable opaque identities; they never carry raw
   events or coordinates for the shell to re-resolve.
3. Local movement that drives persistence or effects sends the resolved value
   from the component. The shell never recomputes it. Re-anchor only on
   discrete navigation or a responsive transition.
4. Destination components stay mounted while their Service library is in the
   catalog, so local state survives tab and breakpoint changes. Mounted,
   focused, active, and painted stay distinct. Overlays mount and unmount
   through the focus stack.
5. Give every boundary-crossing request variant an exhaustive dispatch arm or a
   documented no-op. Never hide a variant with a wildcard.

## Keyboard routing

The one Keyboard Router lives in the core, in `mbv_client_core::input`
(`router` precedence, `key_policy` order, `resolver` chords), and serves the
TUI and the surfaces both Clients present. The TUI Client mounts it at one
seam: `src/tui/neutral_edge.rs` translates the terminal key into the neutral
chord and hands it to the router, and the TuiRealm listener reaches the main
loop through `src/tui/input_port.rs`. Per-surface key modules stay beside the
router in the core; this crate resolves no precedence, no order, and no chord.
Never add a second routing site. Local components interpret only local
semantic chords.

The GUI Library panel takes its own keys in the GUI window before they reach
this router (ADR 0036); that is the GUI Client's site, not a second TUI site.

## TuiRealm

`mbv_surfaces::FocusStack` is the one focus authority (ADR 0035). TuiRealm is
this Client's adapter: it delivers events, holds subscriptions, and renders,
and it never decides focus. One sync step, the shell `Model`'s
`apply_focus_stack`, is the only TuiRealm focus caller; the one exception is
the `UiRoot` activation in the `Model` constructor, before the first sync
pass. A call site requests an overlay push, an overlay dismiss, or a plain
mount record on the stack; it never calls TuiRealm `active`, `mount`, or
`umount` to move focus.

## Tests

Presentation tests are owned by the narrowest layer that can regress the fact
(`docs/invariants/14-test-ownership.md`). A new test owns a contract no
existing test owns, or reproduces a real bug.

| Layer | Owned proof | Claims removed from this layer |
|---|---|---|
| Arrangement | Relational placement and breakpoint decisions | Painted glyphs, shell state, absolute coordinates |
| Render Component | Focused buffer content and paint-local geometry | Whole application frames and parent placement |
| Interactive Component | Local state transitions, semantic requests, viewport, retained hit resolution | Root placement and shell effects |
| Shell tick integration | Mount/focus/subscription/routing, latest-frame delivery, projection, cross-boundary effects | Glyph choice, spacing, repeated row arithmetic |

- Mounting, focus, subscription, and routing changes need real
  `Application::tick()` tests under `src/tui/tests/tick_integration/`, through
  the shell sync pass. A direct `Component::on` test does not verify
  composition. These assert composition and delivery, never painter
  appearance.
- Assert pane order and size once, relationally, in the arrangement test
  (`hero.x == browser.right() + gap`), never as absolute coordinates.
- Semantic glyph assertions belong only in the owning painter's test; no
  whole-frame equality and no snapshots.
- The behaviour matrix and the completion checklist for a TUI change are in
  `docs/architecture/tui-frontend.md`. Read it before the change.
