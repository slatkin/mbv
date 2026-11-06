---
status: accepted
---

# The Client Core Owns Focus; TuiRealm Is the TUI Adapter

The Client core decides focus, and each Client presents that decision.
`mbv_surfaces::FocusStack` is the one focus authority for every Client.
TuiRealm is the TUI Client's adapter: it follows the stack and never decides
focus itself. This supersedes the focus part of
[ADR 0022](0022-migrate-existing-tui-framework-to-tuirealm.md), which gave
TuiRealm's `Application` ownership of focus. The rest of ADR 0022 stands: the
component model, the shell/`App` authority split, and the composition
ownership.

## Decision

`mbv_surfaces::FocusStack` (crates/mbv-surfaces) holds three records keyed by
`ComponentId`: the mounted surfaces, the Panel focus in effect, and the overlay
stack. It derives the focused surface and never records focus per surface:

- While an overlay is on the stack, the topmost overlay holds focus.
- With an empty overlay stack, the Panel-focus surface holds focus: `Queue` or
  `Library`. While Panel focus is on the Library side and the Library panel is
  not mounted, `UiRoot` holds focus, which matches the queue-only presentation.
- A focus hold takes focus off every surface. Armed prefix capture withholds
  the keyboard from every surface while no overlay is open, and no other stack
  state represents that. The hold is not an overlay, and no dismissal lifts
  it; an explicit release clears it.

Dismissing an overlay restores the surface beneath it, so restore is
deterministic and does not depend on the order the surfaces were mounted in.

Shell call sites request a change on the stack: an overlay push, an overlay
dismiss, or a plain mount record for the panel surfaces. They no longer call
TuiRealm `active`, `mount`, or `umount` to move focus. One sync step, the shell
`Model`'s `apply_focus_stack`, is the only TuiRealm focus caller. It feeds the
stack the shell-held inputs, reconciles the stack against TuiRealm's mount
state in both directions, and applies the derived focus to TuiRealm. The one
exception is the one-time `UiRoot` activation in the `Model` constructor, which
runs before the first sync pass.

`App`'s `PanelFocus` stays the shell-held launch record: TUI launch state
persists it, and the sync step hands it to the stack. It no longer decides
focus by itself.

## Context

[ADR 0022](0022-migrate-existing-tui-framework-to-tuirealm.md) made TuiRealm's
`Application` the focus authority. A second Client cannot present TuiRealm's
focus. Mirroring TuiRealm focus into the GUI Client would keep two focus
authorities, and two authorities drift. Focus is the center of the interaction
contract: a Client shows one focused surface. So focus moves into the Client
core with the other interaction rules, and every Client presents the focus the
same stack derives.

## Considered options

- **Keep TuiRealm as the focus authority and mirror it into the GUI Client
  (rejected).** Two focus authorities drift, and focus is the core of the
  interaction contract.
- **Derive focus in each Client from its own mount state (rejected).** The
  derivation rules would exist once per Client, and the overlay restore rules
  would drift between them.

## Consequences

- A focus rule changes once, in `mbv_surfaces::FocusStack`, and both Clients
  show it.
- TuiRealm keeps event delivery, subscriptions, and the render entry point. Its
  own LIFO focus restore still moves focus at an overlay umount; the next sync
  step re-derives and corrects it.
- The stack reconciles against TuiRealm's mount state in both directions. A
  mount or umount a call site missed cannot leave the stack stale, and a
  component a test mounts by hand is adopted, so the derivation keeps it
  focused as the top overlay.
- `App.panel_focus` keeps its persistence role and feeds the stack; the
  derivation lives in the stack alone.
- `openspec/specs/client-interaction-core/spec.md` owns the current focus
  contract and its scenarios.
