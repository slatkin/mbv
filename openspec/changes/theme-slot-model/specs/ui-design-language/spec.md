# Spec Delta

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
only as foregrounds: text, indicator, scrollbar, or rule colours.

#### Scenario: A surface row changes

- **WHEN** one surface's resting or focused slot is changed in the theme
- **THEN** every site painting that surface renders the changed appearance
- **AND** surfaces with their own rows do not change
- **AND** no screen requires an individual edit

#### Scenario: A background is painted from a role

- **WHEN** a paint site would fill a background, stripe, bar, or chip from a role
- **THEN** review rejects it
- **AND** the site names a surface row instead, adding one if no row names the concept

#### Scenario: A surface has no painter

- **WHEN** a surface identity exists in the table but no painted rect matches it in a layout under
  test
- **THEN** the conformance test fails
- **AND** the surface is either painted or removed from the table

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
