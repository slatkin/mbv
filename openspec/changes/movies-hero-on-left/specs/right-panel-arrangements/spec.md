## MODIFIED Requirements

### Requirement: Each screen is assigned one wide arrangement

Every right-panel screen SHALL be assigned exactly one wide arrangement. Movies, TV shows, podcasts,
feeds, and home videos SHALL use hero-on-top except that the dedicated Movies library SHALL use
hero-on-left. Home, music, and audiobooks SHALL use hero-on-left. No right-panel screen SHALL be
without an assignment.

#### Scenario: Movies is displayed at a wide width

- **WHEN** the dedicated Movies library is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-left arrangement
- **AND** the selected-media hero is on the left
- **AND** the letter-range pills and one-column Movies list are in the right rail

#### Scenario: Movies falls below the shared breakpoint

- **WHEN** the Movies library's available width falls below the shared breakpoint
- **THEN** it falls back to hero-on-top with a single list column

#### Scenario: TV, podcasts, feeds, and home videos are displayed at a wide width

- **WHEN** any of those hero-on-top screens is displayed at or above the shared breakpoint
- **THEN** its arrangement remains hero-on-top
- **AND** its list may use the hero-on-top two-column presentation

#### Scenario: Feeds is displayed at a wide width

- **WHEN** the feeds screen is displayed at or above the breakpoint
- **THEN** it renders the hero-on-top arrangement with a two-column list

#### Scenario: Home videos is displayed at a wide width

- **WHEN** the home videos screen is displayed at or above the breakpoint
- **THEN** it renders the hero-on-top arrangement with a two-column list

#### Scenario: Audiobooks is displayed at a wide width

- **WHEN** an Audiobookshelf book library is displayed at or above the shared breakpoint
- **THEN** it renders the hero-on-left arrangement, matching music at the same dimensions

### Requirement: Hero-on-left presents up to two focusable panes

The hero-on-left arrangement SHALL present up to two panes, of which at most one is focused, and
only while the right panel itself is focused. A screen with a read-only hero pane, such as Home or
the wide Movies library, SHALL expose only its right-hand list as focusable content. Its hero SHALL
remain a projection of the selected list item and SHALL NOT receive keyboard focus or activation.

#### Scenario: Wide Movies has Library focus

- **WHEN** the wide Movies library is displayed and the Library panel has focus
- **THEN** the right-hand Movies list is the focused pane
- **AND** the left selected-media hero remains read-only and does not become a second focus target

#### Scenario: Focus moves between panes

- **WHEN** the user moves focus within a hero-on-left screen that has focusable hero content
- **THEN** exactly one pane is focused and the other renders its unfocused appearance

#### Scenario: The right panel is unfocused

- **WHEN** the right panel is not focused
- **THEN** neither pane of a hero-on-left screen renders as focused

#### Scenario: Queue has focus on wide Movies

- **WHEN** the Queue panel is focused while wide Movies is displayed
- **THEN** neither the left hero nor the right Movies list renders as focused
