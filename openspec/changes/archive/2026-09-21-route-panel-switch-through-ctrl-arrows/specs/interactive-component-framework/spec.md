# Spec Delta

## ADDED Requirements

### Requirement: The panel-focus switch is bound to Ctrl+arrow, not plain arrow

The router-owned `panel_left`/`panel_right` actions SHALL default to
`Ctrl+Left`/`Ctrl+Right`, not plain `Left`/`Right`. A focused leaf's own plain
`Left`/`Right` chords SHALL be reachable in every panel mode and every
destination without a per-destination carve-out in the router's precedence:
the leaf receives the chord unless it declares its own claim on it, never
because the panel switch stepped aside for one destination.

#### Scenario: Plain Left/Right always reaches the focused leaf

- **WHEN** a focused leaf declares its own plain `Left` or `Right` chord, in
  any panel mode
- **THEN** the router does not claim the chord for panel switching
- **AND** the leaf's own handler receives it

#### Scenario: Panel focus switches only on Ctrl+arrow

- **WHEN** `Ctrl+Left` is pressed while the Queue panel holds focus, or
  `Ctrl+Right` is pressed while the Library panel holds focus
- **THEN** panel focus switches to the other panel
- **AND** the same chord pressed on the panel that already holds focus is
  inert, leaving that panel's own focused leaf to handle it if it claims one
