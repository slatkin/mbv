---
status: accepted
---

# The GUI Library Panel Owns Its Interaction State

The GUI Client's Library panel is a GUI-owned surface. The design canvas
specifies it, not the TUI Client. Its interaction state and its interaction
rules live in the GUI Client's front end. The Client core supplies the
Service data, the artwork, and the actions. The rule that interaction
behaviour lives once in the Client core applies only to the surfaces that
both Clients present: the Queue panel, the Queue playback panel, and the
Status bar.

## Decision

- The front end holds the GUI Library panel's cursor, hover, scroll, open
  tree nodes, focus inside the panel, filter text, the open Hero overlay,
  and the open item menu. It handles the panel's keys while the panel has
  Panel focus.
- The Client core holds the data selection: the active destination, the
  selected collections of each destination, and the item whose detail the
  Hero shows. It loads the Service data that the selection needs, encodes
  artwork from the image cache, and runs the actions (play, add to queue,
  play next, shuffle, the item menu's entries) through Owner admission.
- The core focus stack keeps Panel focus between the Library panel and the
  Queue panel ([ADR 0035](0035-client-core-owns-focus-tuirealm-is-the-tui-adapter.md)).
  Focus inside the GUI Library panel is front-end state.
- The TUI Client's Library code does not change. The two Clients' Library
  panels can differ in content, keys, and behaviour.

## Context

The first GUI slice moved the Queue's interaction state into `mbv-surfaces`,
so both Clients run one set of Queue rules. Applying that rule to the
Library meant two jobs that are not GUI work. The first job moved the state
of seven TUI destination owners into the Client core with the TUI output
unchanged. The second job fitted each canvas behaviour into the TUI's
keyboard model: for example, Space toggles a focused pill in the canvas
but plays and pauses in the TUI. The canvas is a mouse-driven web design
with pressed buttons, hover controls, Alt-click, and native focus. The GUI
does not need parity with the TUI.

## Considered options

- **Port the TUI destination owners to shared Surface state (rejected).**
  It refactors seven TUI owners, and it binds every GUI Library behaviour
  to the TUI's rules and keys.
- **GUI-only interaction state in the Client core (rejected).** It removes
  the TUI refactor, but every hover, scroll, and focus change crosses the
  bridge, and the Library keys still pass the shared keyboard router, where
  they meet the TUI's bindings.
- **Interaction state in the front end (chosen).** The webview gives native
  focus, scroll, and pointer handling. The core keeps what both Clients must
  agree on: the Service data, the queue, and the Owner.

## Consequences

- A window-content reload resets the GUI Library panel's cursor, scroll, and
  open nodes. The data selection stays in the core.
- The GUI Library panel's keys are GUI keys. `[keys]` does not configure
  them. Keys that the panel does not handle still go to the core's keyboard
  router.
- `openspec/specs/client-interaction-core/spec.md` and
  `openspec/specs/gui-client/spec.md` own the scope of the shared rules.
