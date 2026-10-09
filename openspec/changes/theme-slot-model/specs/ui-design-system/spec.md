# Spec Delta

## MODIFIED Requirements

### Requirement: The palette is one closed meaning-free colour vocabulary

The theme SHALL own one closed set of colour slots: one slot per distinct colour the UI paints, each
slot named by its tier or hue (a background ladder, a foreground ladder, accent hues), never by a
role. A theme SHALL be a value that assigns exactly one `Rgb` colour to every slot; the built-in
default theme SHALL be the only place a palette `Color::Rgb` literal appears. All visual meaning
SHALL live in semantic roles and surface rows, which are compiler-visible assignments over slots,
not over colour values. Production code SHALL consume a slot only through a role or surface row
resolved against the active theme, never by naming a slot or a hex value. Ratatui's own raw
`Color::` specials (Black/White/Reset) SHALL stay outside the slot set as mechanics. Adding a slot,
re-valuing a slot in a theme, or re-assigning a role or surface row SHALL happen in the theme alone.
`docs/palette.json` is a live maintainer reference, not a source or a guard: every `mbv-theme` test
run regenerates its slot, role, role-set, and surface values from the theme, keeping the
hand-written `uses` prose.

#### Scenario: A variant is added or re-valued

- **WHEN** a distinct colour enters the UI, or an existing slot's value is restored or changed
- **THEN** the change lands as one slot assignment in the theme
- **AND** no screen or component gains a second copy of the value

#### Scenario: A colour is consumed

- **WHEN** a surface or text run needs a palette colour
- **THEN** it names a semantic role or surface row that assigns the slot
- **AND** it never names the slot or a hex value directly outside the theme

#### Scenario: A theme is swapped

- **WHEN** the active theme is replaced by another theme value
- **THEN** every role and surface resolves to the new theme's colour for its slot
- **AND** no role, surface row, screen, or component changes

## ADDED Requirements

### Requirement: Theme colours resolve at paint time

Roles and surfaces SHALL resolve to a colour from the active theme when a frame is painted, not from
values fixed at compile time. Code that branches on a colour SHALL compare role or surface
identities, never resolved colour values, so that two identities a theme assigns the same slot stay
distinguishable.

#### Scenario: The default theme is active

- **WHEN** the app paints any frame with the default theme active
- **THEN** every cell renders the same colour it rendered before the slot model existed

#### Scenario: Two identities share a slot

- **WHEN** a theme assigns the same slot to the selection bar and to another fill
- **THEN** a row painted on the selection bar still takes the selection bar's foreground policy
- **AND** a row painted on the other fill does not
