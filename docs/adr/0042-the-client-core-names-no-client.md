---
status: accepted
---

# The Client Core Names No Client

The Client core is the layer the TUI Client and the GUI Client share for the
surfaces both present ([ADR 0035](0035-client-core-owns-focus-tuirealm-is-the-tui-adapter.md)).
It decides behaviour by neutral facts, neutral signatures, and the
capabilities of the attached Client. It never names a Client, never calls a
GUI function, and never holds a terminal-cell or toolkit value. Each Client
wraps the core state by composition and answers, through its own code, the
questions the core cannot decide.

## Decision

- **Each Client wraps `App` by composition.** `App` stays the shared core
  state and holds no GUI state. The TUI's shell `Model` owns `App` plus the
  TUI-only state (terminal size, the pushed single-panel fact, the tab-strip
  scroll, the composed frame layout). The GUI owns
  `GuiCore { app, gui }`: `GuiClientState` holds every GUI-only field,
  including `panel_collapsed` and `queue_folded`, while the shown-entry state
  stays in the core. `GuiCore` is defined beside the `app` module but outside
  it, so the crate split moves no definition. The GUI's `CoreThread` holds
  the `GuiCore` and keeps its `ClientTickHost` implementation. Preferences
  split the same way: the core writes only the keys it owns, and each Client
  reads its own keys at start and writes them through `save_client_pref`.
  The file format and every key name stay as they are.
- **The core calls no GUI function.** The former GUI-module methods that core
  dispatch and playback handling reached go three ways. The GUI-half arms of
  `App::dispatch` and `App::handle_playback_request` become documented
  no-ops: every variant stays listed, no wildcard arm hides one, and the GUI
  entry calls the core first and then its own half. The methods that are
  Owner, queue, or shared behaviour — the named-queue request and answer
  path, the queue filter, the chooser target, and the artist-name lookup —
  move into core modules beside the code they serve; `request_new_gui_queue`
  is renamed `request_new_queue`. The library effects and the feed-target
  query cross through one seam: the event handlers take
  `Option<&mut dyn ClientLibraryStores>`, the core collects the
  `ClientLibraryEffect`s of one event and applies them through the seam in
  the same order as before, and the TUI passes `None`.
- **Client-only events leave the shared event enum.** The GUI fetch results
  are no longer variants the core carries. The workers report a
  `GuiClientEvent` on a channel, and the shared tick drains it through a
  `drain_client_events` hook on `ClientTickHost`, after the library-event
  drain. The GUI implementation drains the channel; the TUI implementation
  is a documented no-op that returns `false`. A landed fetch wakes the tick
  through the same `ClientWaker` the other channels use.
- **Shared signatures carry no terminal cell and no router field.** The core
  builds a context menu's entries and target; each Client places it. No core
  function takes a pointer cell and no core state stores one:
  `ContextMenuAnchor::Pointer` is a unit variant, and the TUI shell keeps the
  cell it records in a `menu_pointer` field of its own. Any other
  terminal-cell or toolkit type in a core signature gets the same treatment:
  a neutral value in the core, the conversion in the Client. The GUI routing
  entry resolves its keys through the router's neutral entry,
  `resolve_router_outcome`: a `RouterSnapshot` of facts in, a `RouterOutcome`
  out, and the live routing state read through the `RouterNow` trait. The
  GUI entry reads no router fields of its own. Precedence, order, and chords
  still resolve only in the router's three files.
- **Capabilities, not a Client kind.** Where the Clients differ, the core
  reads a neutral fact from the attached Client's `ClientCapabilities` — a
  flag set of `ClientCapability` values (`TerminalPanelRules`,
  `VisualSlotControls`, `ShownQueueEntries`, `TransportChipKeys`), not a
  struct of `bool` fields. The attach step takes the Client's capabilities,
  builds the first Hello from them, and stores them on the new `App` before
  its first step, so the core reads them from the first frame; a reconnect
  builds its Hello from the stored value. The Client-kind flag, the
  enable call, and the derived helpers are gone. Every former reader maps to
  one fact, with no behaviour change: the Mini view reads the pushed
  single-panel fact together with `TerminalPanelRules` (the TUI pushes it at
  every point where it records the terminal size, so the first frame of a
  narrow terminal is in Mini view as before); the shown-entry gates read
  `ShownQueueEntries`; the router gates become `TransportChipKeys` and
  `TransportChipQueueTarget` and read the carried capabilities. The
  launch-restore skip and the pin-swap non-participation are not
  capabilities: the GUI calls `skip_launch_restore` and claims its
  pin-swap non-participation itself, before it attaches.

### Transitional arrangements

While the Clients are still one root package, the GUI's one-call wrapper
steps move a placeholder — the parked `App`, a stub construction that ships
in every build — out of position while the step holds the live one in its
`GuiCore`, and the test seams beside it serve the same shape. These
scaffolds are known and transitional: the change `2-split-tui-gui-clients`
removes them when the Clients become crates. This decision records them;
it does not relitigate them.

## Context

The GUI modules were `impl App` blocks beside the core. They read dozens of
non-GUI `App` fields directly, owned GUI fields on `App`, and added GUI
variants to the shared library event enum, so the shared event type carried
GUI-only traffic. Core dispatch and playback handling called about twenty
GUI-module methods directly, as plain `App` methods, and the one Client-kind
flag on `App` decided GUI-only behaviour deep inside the core — the Mini
rule, the empty queue column, the visualizer controls, the shown queue, and
the router gates. Any new Client would have to either grow a third branch of
that flag or fake one of the two existing kinds.

A first attempt at the split threaded GUI state through core functions and
failed: hundreds of files changed in one step. Answering each question at
one seam instead — the no-op arm, the moved core method, the
`ClientLibraryStores` seam, the event channel, the neutral anchor, and the
capability flag — keeps every step small.

## Considered options

- **`App<X: ClientExt>` with a Client-specific slot (rejected).** It would
  make the core generic over the Clients, change every shared signature, and
  spread the boundary over seventy thousand lines.
- **A `Box<dyn Any>` Client slot with downcasts (rejected).** It hides the
  boundary from the compiler instead of proving it.
- **An extension trait on `App` per Client (rejected).** It needs `pub`
  fields once the Clients are separate crates.
- **Threading `Option<&mut GuiClientState>` through core functions
  (rejected).** The first attempt did this; the seam removes it again.
- **A hook that runs GUI code inside the core handler, or an async channel
  for the library effects (rejected).** The hook reaches most destinations
  from one call, and the channel changes the event order and cannot answer
  the feed-wanted query.
- **Keep a `ClientKind { Tui, Gui }` enum in the core (rejected).** Every
  branch on the kind is GUI code in the shared core, which the
  client-interaction-core delta forbids.

## Consequences

- The core compiles with no GUI module, no Client name, and no terminal
  toolkit in its signatures or state. The compiler, not review, holds the
  boundary: a GUI call from the core is a missing arm or a missing seam, and
  a Client kind is a missing capability fact.
- A third Client attaches by building its own `ClientCapabilities`, passing
  `None` where it has no library stores, and implementing
  `drain_client_events` as a no-op. It changes no core code.
- The GUI reads the shown entry through a core read method, so the
  [ADR 0041](0041-gui-shows-one-entry-per-queue.md) chooser rule and the
  Owner-projection fallback keep their authority in the core.
- The parked-`App` scaffolds and the in-package seams are debt with a named
  owner: the crate-split change removes them, and nothing else builds on
  them.
- Related decisions:
  [ADR 0035](0035-client-core-owns-focus-tuirealm-is-the-tui-adapter.md)
  keeps the core's focus authority,
  [ADR 0039](0039-gui-client-renders-with-dioxus-native-on-blitz.md) keeps
  the GUI's renderer beside the core in one process, and
  [ADR 0040](0040-gui-key-target-follows-the-pointer.md) keeps the pointer
  as the GUI's key target, which the neutral router entry serves.
