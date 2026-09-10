## MODIFIED Requirements

### Requirement: Focus state colouring is centrally controlled

The focused and unfocused appearance of every panel, sub-panel, list, and component SHALL be
determined in one place from a focus state supplied by the caller. A screen SHALL supply the
panel's focus state — the existing `PanelFocus` plus, for Wide hero screens, a pane bit — and
SHALL NOT name the colour used for any state. This SHALL apply to the left panel's card and queue
as well as to right-panel content.

The surface a panel sits on SHALL resolve from that same focus state, and SHALL change appearance
when the panel it contains gains or loses focus. The highlight on a list's selected row SHALL
resolve to the surface that contains the list's panel rather than to a fixed backdrop, so a focused
list always shows its selection against a colour distinct from its own panel fill.

#### Scenario: The focused appearance is changed

- **WHEN** the definition of the focused appearance is changed in one place
- **THEN** every panel and sub-panel in the application renders the changed appearance, including
  the queue and card
- **AND** no screen requires an individual edit

#### Scenario: A screen reports its focus state

- **WHEN** a screen renders with a given panel and pane focus state
- **THEN** its appearance is chosen by the shared definition from that focus state alone

#### Scenario: A panel gains focus

- **WHEN** a panel gains focus
- **THEN** both the panel fill and the surface the panel sits on render their focused appearance
- **AND** the selected row's highlight resolves to the surface the panel sits on
- **AND** the panel fill remains distinguishable from that surface

#### Scenario: A column surface does not follow focus

- **WHEN** a screen renders a column or pane that contains a panel
- **THEN** the column's colour is chosen by the shared definition from the focus state the screen
  supplies
- **AND** the column does not keep one fixed colour in every focus state
- **AND** the screen does not name the colour

## ADDED Requirements

### Requirement: Nested content surfaces have distinct roles

A colour role SHALL express exactly one level of content nesting, and a role SHALL NOT be reused
for a different level. The levels are: the column or pane surface that a panel sits on; the panel
that holds content; the recess inset inside a pane or panel; and the modal overlay frame. A role
that names a focusable surface or panel SHALL be focus-resolved, and a screen SHALL NOT keep a
private role for a level that the design language already names.

#### Scenario: Two screens render the same nesting level

- **WHEN** two screens render a focused panel, or two screens render the pane containing one
- **THEN** both derive that level's colour from the single role named for that level
- **AND** neither screen keeps a private role for the same level

#### Scenario: One role spans two nesting levels

- **WHEN** the same colour role would paint both a column surface and a recess inset
- **THEN** each level is given its own role
- **AND** changing one level's definition does not change the other

#### Scenario: The focused panel colour is redefined

- **WHEN** the definition of the focused panel colour changes
- **THEN** every focused panel renders the changed colour
- **AND** modal and dialog frames retain their own role's colour
- **AND** no dialog frame is repainted as a side effect
