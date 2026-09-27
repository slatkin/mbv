## MODIFIED Requirements

### Requirement: Interactive surfaces are TuiRealm components

Every independently interactive surface — one that owns a cursor, scroll,
selection, focus, form draft, or its own key/mouse interpretation — SHALL be a
TuiRealm `AppComponent` mounted in the application's
`Application<ComponentId, Msg, UserEvent>`. TuiRealm SHALL own mounting,
unmounting, focus, the focus stack, subscriptions, terminal-event delivery, and
the entry point that renders each component. mbv SHALL NOT define a parallel
component trait, component registry, event dispatcher, focus framework, generic
effect scheduler, or Flux store alongside TuiRealm.

Render Components (rows, cards, heroes, pills, modal frames, scrollbars, and
other painters in the `mbv-render` crate's `components` module) are NOT
interactive surfaces and remain plain painting functions invoked from a
component's rendering.

Interactive Components SHALL live in the `mbv-components` crate and Render
Components in the `mbv-render` crate. Neither crate SHALL depend on the TUI
application crate that owns `App`, and a painter SHALL NOT take an Interactive
Component type as input. A component or painter that names `App` therefore
fails to compile, rather than relying on review.

The UI crates (`mbv-ui-model`, `mbv-ui-msg`, `mbv-render`, `mbv-components`)
SHALL NOT depend, directly or transitively through normal dependencies, on the
Player runtime, the Player-owner protocol, or the Emby, websocket,
remote-player, cast or daemon crates. The shell-only state that holds a Player
handle, a protocol stream, a worker channel or a provider client SHALL live in
the TUI application crate. Presentation models that the UI crates receive SHALL
carry only the display fields they paint and opaque keys the shell resolves,
not provider runtime values such as session records, discovered receivers or
connectable endpoints.

#### Scenario: An overlay is opened and dismissed

- **WHEN** the user opens an overlay (Search, Settings, Sessions, Playlists,
  Help, context menu, or a modal)
- **THEN** its component is mounted and receives focus
- **AND** when it is dismissed the component is unmounted and focus returns
  deterministically to the surface that had it before, independent of mount order

#### Scenario: A destination retains its state while inactive

- **WHEN** the user navigates away from a Library destination and later returns
- **THEN** its component MAY remain mounted so its private cursor and scroll are
  preserved
- **AND** a component for a Service library that has been removed is unmounted

#### Scenario: A component cannot reach shell state

- **WHEN** code in `mbv-components` or `mbv-render` names `App`, a shell
  module, or a dispatch module
- **THEN** the workspace does not compile

#### Scenario: A component cannot reach the Player runtime

- **WHEN** code in any UI crate names a Player handle, a Player-owner protocol
  type, a websocket sender, or an Emby or cast client type, or reaches one
  through a field of a type it can name
- **THEN** the workspace does not compile

#### Scenario: A runtime edit does not recompile the UI crates

- **WHEN** a source file in the Player, protocol, Emby, websocket,
  remote-player, cast or daemon crate changes
- **THEN** none of `mbv-ui-model`, `mbv-ui-msg`, `mbv-render` or
  `mbv-components` is rebuilt
