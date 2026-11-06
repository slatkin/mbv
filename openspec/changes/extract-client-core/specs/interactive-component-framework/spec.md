# Spec Delta

## MODIFIED Requirements

### Requirement: Interactive surfaces are TuiRealm components

In the TUI Client, every independently interactive surface — one that owns a
cursor, scroll, selection, focus, form draft, or its own key/mouse
interpretation — SHALL be a TuiRealm `AppComponent` mounted in the
application's `Application<ComponentId, Msg, UserEvent>`. TuiRealm SHALL own
subscriptions, terminal-event delivery, and the entry point that renders each
component. Mounting, unmounting, focus, and the focus stack SHALL be decided by
the Client core focus stack (`client-interaction-core`); TuiRealm SHALL follow
that stack and SHALL NOT decide them by itself. mbv SHALL NOT define a parallel
component trait, component registry, event dispatcher, generic effect
scheduler, or Flux store alongside TuiRealm. The Client core focus stack is the
one focus framework, and mbv SHALL NOT define another.

Where a surface is shared with the GUI Client, its interaction state SHALL live
in front-end-neutral Surface state, and its TuiRealm `AppComponent` SHALL be an
adapter that delivers neutral events to that state and paints it. The adapter
SHALL NOT hold a second copy of a value that the Surface state owns.

Render Components (rows, cards, heroes, pills, modal frames, scrollbars, and
other painters in the `mbv-render` crate's `components` module) are NOT
interactive surfaces and remain plain painting functions invoked from a
component's rendering.

Interactive Components SHALL live in the `mbv-components` crate, Surface state
in the `mbv-surfaces` crate, and Render Components in the `mbv-render` crate.
None of these crates SHALL depend on the application crate that owns `App`,
and a painter SHALL NOT take an Interactive Component type as input. A
component or painter that names `App` therefore fails to compile, rather than
relying on review.

The UI crates (`mbv-ui-model`, `mbv-ui-msg`, `mbv-surfaces`, `mbv-render`,
`mbv-components`) SHALL NOT depend, directly or transitively through normal
dependencies, on the Player runtime, the Player-owner protocol, or the Emby,
websocket, remote-player, cast or daemon crates. The shell-only state that
holds a Player handle, a protocol stream, a worker channel or a provider client
SHALL live in the application crate. Presentation models that the UI crates
receive SHALL carry only the display fields they paint and opaque keys the
shell resolves, not provider runtime values such as session records,
discovered receivers or connectable endpoints.

#### Scenario: An overlay is opened and dismissed

- **WHEN** the user opens an overlay (Search, Settings, Sessions, Playlists,
  Help, context menu, or a modal)
- **THEN** the core focus stack mounts it and gives it focus, and its component
  is mounted and receives focus
- **AND** when it is dismissed the component is unmounted and focus returns
  deterministically to the surface that had it before, independent of mount order

#### Scenario: A destination retains its state while inactive

- **WHEN** the user navigates away from a Library destination and later returns
- **THEN** its component MAY remain mounted so its private cursor and scroll are
  preserved
- **AND** a component for a Service library that has been removed is unmounted

#### Scenario: A component cannot reach shell state

- **WHEN** code in `mbv-components`, `mbv-surfaces`, or `mbv-render` names
  `App`, a shell module, or a dispatch module
- **THEN** the workspace does not compile

#### Scenario: A component cannot reach the Player runtime

- **WHEN** code in any UI crate names a Player handle, a Player-owner protocol
  type, a websocket sender, or an Emby or cast client type, or reaches one
  through a field of a type it can name
- **THEN** the workspace does not compile

#### Scenario: A runtime edit does not recompile the UI crates

- **WHEN** a source file in the Player, protocol, Emby, websocket,
  remote-player, cast or daemon crate changes
- **THEN** none of `mbv-ui-model`, `mbv-ui-msg`, `mbv-surfaces`, `mbv-render`
  or `mbv-components` is rebuilt

#### Scenario: A shared surface has one state owner

- **WHEN** the Queue panel is mounted in the TUI Client
- **THEN** its cursor, marks, and viewport live in its Surface state
- **AND** its TuiRealm adapter holds no copy of them

### Requirement: Mounted-component focus has one authority

Whether a mounted Interactive Component is focused SHALL be determined by the Client core focus stack (`client-interaction-core`). The TuiRealm component-focus lifecycle SHALL follow that stack. A component whose input handling or presentation depends on focus SHALL observe the focus that the stack decides. The shell SHALL NOT also carry the same focused state in content projections, refresh content solely to change focused presentation, or maintain a second component-focus mirror.

Component-private pane focus, cursor, scroll, and selection SHALL remain owned by the component, or by its Surface state where the surface is shared, while mounted. Losing component focus SHALL suppress focused presentation without erasing those local values; regaining component focus SHALL immediately restore keyboard delivery and derive the focused pane from the retained local state.

A plain embedded Component SHALL share the mounted parent's focus boundary rather than becoming an independently focused application surface.

#### Scenario: Panel focus moves from Music to Queue and back

- **WHEN** the Music destination is mounted and Panel focus moves from Library to Queue and then back to Library
- **THEN** Music loses focused presentation while Queue holds focus
- **AND** Music receives keyboard navigation immediately when Library regains focus, without requiring a click or content refresh
- **AND** its component-private cursor, scroll, and pane focus remain as they were before the round trip

#### Scenario: Wide TV loses focused row treatment

- **WHEN** Wide TV is mounted with a selected series row and Panel focus moves from Library to Queue
- **THEN** the TV library rail loses its focused background, marker, and selected-row treatment on the next frame
- **AND** its selected series identity remains available for when Library focus returns

#### Scenario: Content refresh cannot overwrite focus

- **WHEN** shell-owned content is pushed to a mounted component while another component holds focus
- **THEN** the content refresh does not make the receiving component appear or behave focused
- **AND** a later focus transition does not require that content to be pushed again

#### Scenario: Overlay focus restores the underlying component

- **WHEN** a blocking overlay takes focus from a mounted destination and is then dismissed
- **THEN** the destination loses focused presentation while the overlay is active
- **AND** the core focus stack makes the destination focused again without a focus-only content projection

#### Scenario: Embedded controls share parent focus

- **WHEN** a mounted destination composes a plain embedded list, browser, or text-entry control
- **THEN** the embedded control is treated as focused only when its mounted parent and the applicable component-private pane are focused
- **AND** the embedded control is not mounted or focused independently

#### Scenario: TuiRealm focus follows the core stack

- **WHEN** the core focus stack changes the focused surface
- **THEN** the TuiRealm focused component is the matching component after the shell's synchronisation pass
- **AND** no TuiRealm focus change happens that the core stack did not decide
