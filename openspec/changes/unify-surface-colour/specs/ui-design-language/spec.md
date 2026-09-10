## ADDED Requirements

### Requirement: One surface table maps a surface to its colour

The theme SHALL expose one closed set of rendered-surface identities and one function resolving a
surface plus the focus state to its fill and border. Every production paint site SHALL obtain a
surface's colour from that function by naming the surface. A screen SHALL NOT name a colour role, call
a colour resolver directly, or branch on a focus bit to choose between colours. Each surface entry
SHALL state its nesting level, and a role SHALL NOT be reachable from a screen by any other path.

#### Scenario: A level's appearance changes

- **WHEN** the definition of one nesting level's appearance is changed in the theme
- **THEN** every surface of that level renders the changed appearance
- **AND** surfaces of other levels do not change
- **AND** no screen requires an individual edit

#### Scenario: A screen tries to choose a colour

- **WHEN** a screen would name a colour role, call a colour resolver, or pick a colour from a focus
  bit
- **THEN** the build fails
- **AND** the screen instead names its surface identity and passes the focus state it was given

#### Scenario: A surface has no painter

- **WHEN** a surface identity exists in the table but no painted rect matches it in a layout under
  test
- **THEN** the conformance test fails
- **AND** the surface is either painted or removed from the table

#### Scenario: The selection moves inside a focused pane

- **WHEN** a pane holds focus and the selection moves between sub-panels inside it, for example from
  a series list to its episode list or from a music browser to its track list
- **THEN** no surface's fill changes
- **AND** the sub-panel that gained the cursor shows its selected row and the other does not
- **AND** the focus state supplied to the screen is the only input that changed the appearance

#### Scenario: One focus state drives every surface

- **WHEN** a frame renders with a given focus state
- **THEN** every surface's appearance is derived from that one value
- **AND** no screen derives its own focus state for colour
