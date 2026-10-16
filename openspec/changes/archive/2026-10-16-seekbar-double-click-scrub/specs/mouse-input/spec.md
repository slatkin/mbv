# Spec Delta

## MODIFIED Requirements

### Requirement: Every visible interactive surface receives mouse events

An interactive component that is mounted **and painted in the most recent frame**
SHALL receive every terminal mouse event, not only while it holds keyboard focus.
A component that is mounted but was not painted in the most recent frame SHALL
NOT receive mouse events at all — not merely have its resulting message
discarded — because the component framework mutates a component before it
returns a message, so a discarded message does not undo a mutation.

Mouse-event delivery SHALL use the component framework's subscription mechanism;
the shell SHALL NOT introduce a separate mouse event loop, a global
completed-frame hit map, or a global coordinate router.

The mounted destination parent owns gesture recognition for its surface. The
parent SHALL decide whether an event is its own by testing the event coordinates
against the non-list chrome geometry it painted on its most recent render —
pills, scope buttons, the seek bar and transport, overlay and popup regions. For a canonical media-list row whose embedded control has a completed current-frame
retained result, the parent SHALL delegate point resolution by passing only the
point to that result, and SHALL NOT re-derive row coordinates itself. An
unmigrated destination MAY continue to pass its painted list rectangle through its
existing compatibility path until its own retained-result migration. The parent SHALL emit a `Msg` for a
mouse event only when it resolves to a region it painted or to a row the embedded
control claims; otherwise it SHALL ignore the event.

A component that resolves a mouse event SHALL resolve it against geometry it
painted itself. Filtering by mouse event kind SHALL happen inside the component;
no behaviour SHALL depend on the subscription clause filtering by event kind.

#### Scenario: A click lands on a panel that does not hold focus

- **WHEN** keyboard focus is on one panel and the user clicks inside a different
  visible panel
- **THEN** the clicked panel's mounted parent receives the event, resolves the
  target from the non-list chrome geometry it painted or by delegating a list
  point to its embedded control, and acts on it
- **AND** the focused panel's mounted parent, receiving the same event with
  coordinates outside its geometry, produces no message

#### Scenario: Mouse events reach a subscribed component through a live tick

- **WHEN** a mouse event is injected into a mounted `Application` through its
  event listener and `tick()` is called
- **THEN** every eligible mounted parent is given the event, and each resolves
  it against the geometry it painted or its embedded control's rows
- **AND** no parent's message for that event is produced twice

#### Scenario: A destination is mounted but not painted

- **WHEN** several destinations are mounted at once and only one is painted,
  and a mouse event arrives over the painted one
- **THEN** the unpainted destinations' mouse handlers are not invoked at all
- **AND** their cursor, scroll offset, and selection are unchanged

#### Scenario: Chrome that never holds focus is still clickable

- **WHEN** the user clicks a transport control in the playback chrome, or
  double-clicks its seek bar, none of which ever receives keyboard focus
- **THEN** the playback component resolves the gesture against its painted
  control geometry and emits the corresponding transport intent, or a seek
  intent carrying a resolved position fraction
- **AND** the shell handler for that intent does not read the seek bar's
  rectangle
