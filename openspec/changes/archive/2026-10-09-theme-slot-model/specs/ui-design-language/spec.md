# Spec Delta

## MODIFIED Requirements

### Requirement: Colour roles have one definition

The TUI SHALL define its colours as named roles and surface rows, and every surface, text run,
rule, and indicator SHALL derive its colour from a role or a surface row rather than from a literal
colour value. Changing a role's or surface row's definition SHALL change every place it is used,
with no per-screen implementation of the same role or row.

#### Scenario: A role definition changes

- **WHEN** the definition of a colour role or surface row is changed
- **THEN** every screen using that role or row renders the changed colour
- **AND** no screen continues to render the previous colour

#### Scenario: Two screens present the same concept

- **WHEN** two screens display the same concept, such as a selected row or a resting panel
- **THEN** both derive that colour from the same role or surface row and render identically

## REMOVED Requirements

### Requirement: One surface table maps a surface to its colour

**Reason**: Its nesting levels never carried a distinct colour (every level resolved the same focused
fill), so the "A level's appearance changes" scenario could not hold. Replaced by "One surface table
maps a surface to its fill", which states each surface's resting and focused slots directly and
makes the surface table the only source of background fills.
**Migration**: Paint sites keep naming a surface and passing their own focus bit; nesting levels are
deleted from the theme. Background fills that came from roles move to surface rows.

## ADDED Requirements

### Requirement: One surface table maps a surface to its fill

The theme SHALL expose one closed set of rendered-surface identities and one function resolving a
surface plus a focus boolean to its fill. Every production paint site SHALL obtain a background fill
from that function by naming the surface. Each surface entry SHALL state exactly two slots, resting
and focused; a surface that does not react to focus states the same slot twice. Roles SHALL serve
only as foregrounds, apart from the permitted inversions.

#### Scenario: A surface row changes

- **WHEN** one surface's resting or focused slot is changed in the theme
- **THEN** every site painting that surface renders the changed appearance
- **AND** surfaces with their own rows do not change
- **AND** no screen requires an individual edit

#### Scenario: A background is painted from a role

- **WHEN** a paint site would fill a background, stripe, bar, or chip from a role
- **AND** the fill is not an inversion permitted by "Inverted spans swap a role and a fill"
- **THEN** review rejects it
- **AND** the site names a surface row instead, adding one if no row names the concept

### Requirement: Inverted spans swap a role and a fill

A reverse-video span MAY paint a foreground role as its fill when its text is the on-accent role;
the indicator Chips and Powerline treatments paint the same status roles that the other treatments
paint as text. An edge glyph that draws a surface's outline MAY paint that surface's fill as its
foreground. No other site SHALL fill from a role or take a foreground from a surface.

#### Scenario: An indicator chip is painted in reverse video

- **WHEN** the Chips or Powerline indicator treatment paints a status indicator
- **THEN** the chip fill is the same role the text treatments paint that indicator with
- **AND** the chip text is the on-accent role
- **AND** no surface row duplicates the indicator's role

#### Scenario: A pill edge glyph is painted

- **WHEN** a pill shell paints its slanted edge glyphs
- **THEN** each glyph's foreground is the pill's surface fill

### Requirement: A surface follows its paint site's own focus condition

The focus boolean passed to the surface table SHALL be the call site's own focus input, exactly the
condition the site used before the table existed. The table SHALL NOT change whether a surface
reacts to focus, nor which signal drives the reaction.

#### Scenario: A site migrates onto the table

- **WHEN** a production paint site is moved onto the table
- **THEN** the site passes the same focus condition it used before, unchanged
- **AND** with that condition true the surface renders the focused appearance it rendered before
- **AND** with that condition false the surface renders the resting appearance it rendered before

#### Scenario: A cursor-driven site keeps its predicate

- **WHEN** a site's colour followed a cursor predicate rather than Panel focus, for example a
  detail pane's episode box that lights only while an episode cursor is active
- **THEN** the site keeps that predicate as its focus input
- **AND** the table does not substitute Panel focus or any other signal for it

### Requirement: One concept takes one colour identity

A concept painted on more than one screen SHALL use one role or one surface row everywhere it
appears. A second identity SHALL NOT be created for an existing concept only so that its colour can
be edited separately; a deliberate visual difference SHALL be a named variant that states the
difference.

#### Scenario: Two lists paint the same stripe

- **WHEN** two lists paint the same secondary zebra fill
- **THEN** both name the same surface row

#### Scenario: Two paint sites paint the same selection bar

- **WHEN** a list's selected row and a popup's selected row paint the same selection bar
- **THEN** both name the same surface row
