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

### Requirement: Palette primitives are not a public API

Raw `Color` primitives SHALL be private to the theme module. Semantic roles and surface rows SHALL
be the only public styling API. Components SHALL consume roles, surface rows, or component style
policies; screens SHALL NOT pass arbitrary `Color` or `Style` values into shared components.

#### Scenario: A component renders focused and unfocused states
- **WHEN** the component receives its focus state
- **THEN** it resolves the appropriate surface fill through a surface row and its text styles
  through roles
- **AND** the screen does not select independent foreground and background colours

#### Scenario: No existing role fits a call site
- **WHEN** a call site cannot be expressed with an existing role or surface row
- **THEN** a named role or surface row carrying the visual meaning is added to the theme
  vocabulary
- **AND** the primitive is not re-exported and no screen-local colour alias is created

### Requirement: Screen modules do not paint

Screen modules SHALL NOT paint directly: no direct Ratatui painting, no
layout-rect construction, and no buffer access. A screen supplies typed content;
painting belongs to an owning component or arrangement.

A call site SHALL state the styling role or surface row it sets explicitly. Supplying a bare
colour value where a style is expected silently sets the foreground and leaves
the intended background unpainted, so it is not conforming.

Nothing enforces this set of rules mechanically. It is a review obligation
carried by the module table and the frontend guide's completion checklist; a green
build is not evidence that it holds.

Duplicated arrangement geometry and hit targets that have drifted from their
painting are review's responsibility for the same reason: they are not statically
detectable. Buffer tests verify component behaviour and preserved output; they do
not by themselves establish conformance.

#### Scenario: A screen bypasses a canonical painter
- **WHEN** a change adds direct rendering or rect construction in a screen module
- **THEN** review rejects the change
- **AND** the code moves to the component or arrangement that owns the geometry or
  painting, or out of `screens/` because it was never screen code

#### Scenario: A bare colour is supplied where a style is expected
- **WHEN** a change paints a block or widget by supplying a bare colour value in place of a style
- **THEN** the call site names the foreground role or background surface row it intends to set
- **AND** the change is not conforming until it does

#### Scenario: Painting code that is not screen code
- **WHEN** code that owns geometry or painting sits in a screen module for
  historical reasons
- **THEN** it is rehomed to the arrangement, component, or shell module that its
  signature identifies as its owner
- **AND** the observable painted output is unchanged

## ADDED Requirements

### Requirement: Theme colours resolve at paint time

Roles and surfaces SHALL resolve to a colour from the active theme when a frame is painted, not from
values fixed at compile time. Code that branches on a colour SHALL compare role or surface
identities, never resolved colour values, so that two identities a theme assigns the same slot stay
distinguishable. The one exception is the queue band's indicator row, which recognises a
reverse-video indicator span by its on-accent text over a fill, the mark that defines such a span.

#### Scenario: The default theme is active

- **WHEN** the app paints any frame with the default theme active
- **THEN** every cell renders the same colour it rendered before the slot model existed

#### Scenario: Two identities share a slot

- **WHEN** a theme assigns the same slot to the selection bar and to another fill
- **THEN** a row painted on the selection bar still takes the selection bar's foreground policy
- **AND** a row painted on the other fill does not
