## MODIFIED Requirements

### Requirement: Idle feed display in playback panel

The system SHALL display the current feed item's title in the playback panel title row ONLY when playback is idle (nothing playing and no remote session connected), the playback panel is rendered, and the title row is drawn, replacing the otherwise blank title area. In queue-only state with no playback active the playback panel is not rendered (see `queue-only-playback`), so the idle feed title SHALL NOT be displayed there; in the two-panel layout it continues to display as today.

#### Scenario: Idle state shows feed title

- **WHEN** nothing is playing and no remote session is connected, and at least one feed item has been fetched, and the playback panel is rendered
- **THEN** the playback panel title row SHALL display the title of the current feed item.

#### Scenario: Idle queue-only shows no feed title

- **WHEN** the layout is in queue-only state and nothing is playing
- **THEN** the playback panel SHALL NOT be rendered and no feed title SHALL be displayed; the queue panel SHALL occupy those rows.

#### Scenario: Active playback hides feed

- **WHEN** playback becomes active (local or remote)
- **THEN** the feed title SHALL be hidden and the normal now-playing title SHALL be displayed instead.

#### Scenario: No feed items yet shows blank space

- **WHEN** nothing is playing but no feed items have been fetched yet (startup, or fetch pending) and the playback panel is rendered
- **THEN** the playback panel SHALL render the existing blank bar, unchanged from current behavior.

## ADDED Requirements

### Requirement: Idle feed open-link command follows the display

The command that opens the current idle feed item's link SHALL be offered only where the idle feed title can be displayed. In queue-only state with no playback active, where the playback panel is not rendered, the command SHALL NOT fire; in the two-panel layout it continues to fire as today.

#### Scenario: Open-link suppressed in queue-only idle

- **WHEN** the layout is in queue-only state with no playback active, an idle feed item with a link is current, and the user presses the open-feed-link key
- **THEN** no browser SHALL be opened.

#### Scenario: Open-link unchanged in two-panel idle

- **WHEN** the layout is in two-panel state with nothing playing, no remote session connected, and an idle feed item with a link is current
- **THEN** the open-feed-link key SHALL open that item's link, as today.

#### Scenario: Rotation continues while hidden

- **WHEN** the idle feed title is not displayed because the layout is queue-only idle
- **THEN** feed fetching and rotation SHALL continue unchanged; only the display and its command are gated.
