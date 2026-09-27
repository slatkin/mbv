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
