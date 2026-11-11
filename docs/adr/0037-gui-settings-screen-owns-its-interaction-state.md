---
status: accepted
---

# The GUI Settings Screen Owns Its Interaction State

The GUI Client's Settings screen is a GUI-owned surface, as the Library panel
is ([ADR 0036](0036-gui-library-panel-owns-its-interaction-state.md)). The
window content holds the screen's sidebar cursor, the keyboard position
(sidebar or content), each list's cursor and scroll, the form drafts, the
choice marks before an apply, and the key-capture state. The Client core
holds whether the screen is open, the section it shows, the section's data,
the form progress and errors, and every action the screen runs.

The open section is the core's for two reasons. First, the open actions are
rebindable `[keys]` actions, and only the core's keyboard router resolves
them — a window that opened the screen itself would need to read the user's
configured chords. Second, some sections start loads when they open
(Playlists loads the playlist list, Library loads the library list), and the
core owns those loads. A window-content reload keeps the screen open at its
section and drops the rest of the front end's state.

## Context

The Settings screen replaces the TUI's Settings sidebar, whose interaction
state lives in the TUI shell's components. No shared Surface state exists to
share with: the TUI keeps its sidebar, and the two Clients' settings
presentations differ in content — the GUI hides the settings that have no
effect on it and adds the Sessions and Playlists sections. Applying the
shared-Surface-state rule of `client-interaction-core` would therefore mean
new surface state with no second Client to share it.

The core stores the open state as `App::gui_settings`, and the GUI core
thread's drain of the App's pending sidebar requests drives it: the
`settings_open`, `sessions_open`, and `playlists_open` actions open, close,
and switch the screen through one toggle rule. Opening pushes
`ComponentId::Overlay(OverlayId::Settings)` onto the core focus stack and
closing pops it, so a key routes with no leaf while the screen shows — the
global and transport actions fire, and the queue-scoped keys stay behind the
screen. The screen is not a blocking overlay of the router's.

## Considered options

- **Surface state in `mbv-surfaces` (rejected).** The same reasons ADR 0036
  rejects it for the Library panel: it binds the GUI's screen to the TUI's
  rules, and there is no shared TUI surface to share with.
- **Front-end-only open flag (rejected).** The open actions are
  `[keys]`-configured; the window content would need the user's configured
  chords to decide what a press opens, which duplicates the router's
  resolution in the front end.
- **Interaction state in the front end, open section in the core (chosen).**
  The webview owns cursors, drafts, and capture state; the core owns what the
  router and the section loads need.

## Consequences

- A window-content reload keeps the screen open at its section and resets the
  rest of the interaction state: half-typed forms and unapplied marks go.
- A frame carries only the open section's data, and never a secret; the
  `gui-settings-screen` spec owns the frame's shape.
- The screen's keys are GUI keys of the front end's key router, installed
  ahead of the Library panel's steps while the frame carries the screen. Keys
  the screen does not take still reach the core's keyboard router.
- The TUI Client's Settings sidebar and its shell state do not change.
