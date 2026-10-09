# Spec Delta

## MODIFIED Requirements

### Requirement: Podcast tab uses one state-and-show pill selector

The podcast tab's Selector row SHALL present one mutually exclusive pill selection: `Latest`, the state pills `All`, `Unplayed`, and `Played`, followed by one pill per subscribed podcast. `Latest` SHALL show the library's Newest Episodes shelf independently of the selected show or state filter. A state pill SHALL show matching episodes across all shows; a show pill SHALL show that show's episodes regardless of play state. Exactly one
pill SHALL be active, identified by value rather than by position, and state and show selections SHALL NOT
combine in one view. `Unplayed` SHALL include episodes with missing or incomplete (in-progress) progress;
`Played` SHALL include only completed progress. Pills SHALL use the shared `render_pill_bar` widget, follow
its label truncation and overflow contract, and SHALL write `layout.selector_tabs`. `[` and `]` SHALL move
through the pill bar as one uniform gesture — there is one pill kind, with no per-kind key, ordering, or
behaviour. The last active pill SHALL be remembered in session memory across tab switches. On every restart, a podcast library SHALL start on `Latest`, whether or not it was the selected destination at orderly exit; no podcast pill selection SHALL persist across a restart.

#### Scenario: Podcast tab renders state-and-show pills
- **WHEN** the Audiobookshelf podcast tab is displayed with shows available
- **THEN** the Selector row renders `Latest`, `All`, `Unplayed`, and `Played` followed by one pill per subscribed podcast
- **AND** no alphabetical range bucket, `#` bucket, or empty-range pill renders

#### Scenario: Latest is independent of other pills
- **WHEN** the user selects `Latest` while a show or state pill was active
- **THEN** the list shows the library's newest episodes from its Newest Episodes shelf
- **AND** no show or played-state filter excludes those episodes

#### Scenario: State filter semantics

- **WHEN** `Unplayed` is active
- **THEN** only episodes with missing or incomplete progress render across all shows
- **WHEN** `Played` is active
- **THEN** only episodes with completed progress render across all shows

#### Scenario: Show pill scope

- **WHEN** a show pill is active
- **THEN** the list renders that show's episodes under the same age-group headings
- **AND** episodes of other shows do not render

#### Scenario: Mutually exclusive selection

- **WHEN** the user picks a show pill while a state pill is active (or the reverse)
- **THEN** the selector moves to the picked pill as the single active selection
- **AND** no combined state-plus-show view exists

#### Scenario: Keyboard navigation

- **WHEN** the user presses `]` or `[` on the podcast tab
- **THEN** the active pill moves to the next or previous pill in the bar, wrapping at either end
- **AND** the movement is identical whether the pill is Latest, a state pill, or a show pill

#### Scenario: Last pill remembered in session
- **WHEN** the user leaves the podcast tab and returns without restarting mbv
- **THEN** the remembered pill is active again
- **WHEN** mbv restarts, whether it exited on this podcast library or on a different tab
- **THEN** the `Latest` pill is active
- **THEN** a show or state pill selected in the previous session SHALL NOT restore

#### Scenario: Pills use the shared widget

- **WHEN** the selector pills are rendered
- **THEN** they use the same `render_pill_bar` widget and overflow behavior as every other library tab
- **AND** they follow the shared label truncation contract (show-name pills truncated like feed-group labels)
- **AND** they write `layout.selector_tabs` for mouse hit-testing
