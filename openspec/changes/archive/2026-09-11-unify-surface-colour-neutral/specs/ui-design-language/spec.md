## ADDED Requirements

### Requirement: One surface table maps a surface to its colour

The theme SHALL expose one closed set of rendered-surface identities and one function resolving a
surface plus a focus boolean to its fill and border. Every production paint site SHALL obtain a
surface's colour from that function by naming the surface. A screen SHALL NOT name a colour role or
obtain a surface colour by any other path. Each surface entry SHALL state its nesting level, and a
role SHALL NOT be reachable from a screen except as a text or indicator colour.

The focus boolean SHALL be the call site's own focus input, exactly the condition the site used
before the table existed. The table SHALL NOT change whether a surface reacts to focus, nor which
signal drives the reaction: a surface that rested at one colour in main SHALL rest at that colour
here, and a surface that lit with its site's focus SHALL light on the same condition and no other.

#### Scenario: A level's appearance changes

- **WHEN** the definition of one nesting level's appearance is changed in the theme
- **THEN** every surface of that level renders the changed appearance
- **AND** surfaces of other levels do not change
- **AND** no screen requires an individual edit

#### Scenario: A screen tries to choose a colour

- **WHEN** a screen would name a colour role or call any colour resolver other than the table's
- **THEN** the build fails
- **AND** the screen instead names its surface identity and passes the focus input it already holds

#### Scenario: A surface has no painter

- **WHEN** a surface identity exists in the table but no painted rect matches it in a layout under
  test
- **THEN** the conformance test fails
- **AND** the surface is either painted or removed from the table

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
