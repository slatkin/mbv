## RENAMED Requirements

- FROM: `### Requirement: Wide Music focus uses the Home visual language`
  TO: `### Requirement: Hero-on-left uses one focus treatment`

## MODIFIED Requirements

### Requirement: Grouped Music uses responsive compositions

The grouped Music album view SHALL use the hero-on-left arrangement. Below the shared breakpoint it
SHALL fall back to hero-on-top with a single list column. At or above the breakpoint it SHALL render
hero-on-left, with album detail and tracks in the hero pane and album browsing in the list pane. The
grouped Music view SHALL NOT evaluate the breakpoint itself. Screens assigned hero-on-top SHALL NOT
change arrangement because of this requirement.

#### Scenario: Grouped Music below the breakpoint

- **WHEN** the grouped Music content area is narrower than the shared breakpoint
- **THEN** group pills span the content width, the album hero renders above the list, and albums
  render one per row

#### Scenario: Grouped Music at the breakpoint

- **WHEN** the grouped Music content area reaches the shared breakpoint
- **THEN** it renders the hero-on-left arrangement

#### Scenario: Non-Music library at wide width

- **WHEN** a library assigned hero-on-top is rendered at or above the breakpoint
- **THEN** it renders hero-on-top with a two-column list and does not adopt hero-on-left

### Requirement: Hero-on-left uses one focus treatment

The hero-on-left arrangement SHALL apply one focused and unfocused surface treatment to every screen
that uses it, including grouped Music and Home. During album browsing the list pane SHALL carry the
focused treatment and the hero pane SHALL carry the resting treatment. During track selection those
treatments SHALL reverse. When the Library panel itself is unfocused, both panes SHALL use the
unfocused treatment. Grouped Music SHALL NOT define these colours itself.

#### Scenario: Album browser has focus

- **WHEN** track selection is inactive and the Library panel is focused
- **THEN** the list pane has the arrangement's focused treatment and the hero pane remains a
  readable preview

#### Scenario: Track selection has focus

- **WHEN** track selection is active and the Library panel is focused
- **THEN** the hero pane has the arrangement's focused treatment and the list pane is visibly dimmed
  while retaining the selected album marker

#### Scenario: Queue has focus

- **WHEN** the Queue panel has focus
- **THEN** both Music panes use the arrangement's unfocused treatment

#### Scenario: The focused treatment is changed

- **WHEN** the hero-on-left focused treatment is changed in its single definition
- **THEN** grouped Music, Home, and audiobooks all render the change
