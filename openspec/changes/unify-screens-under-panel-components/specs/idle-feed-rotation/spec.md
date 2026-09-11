## MODIFIED Requirements

### Requirement: Idle feed display in playback panel

The system SHALL display the current feed item's title in the playback panel title row ONLY when playback is idle (nothing playing and no remote session connected), the playback panel is rendered, and the title row is drawn, replacing the otherwise blank title area. The panel's presence is defined by `queue-playback-panel`: in a queue-visible layout an idle player collapses the panel, so the idle feed title SHALL NOT be displayed there; where the panel does render — the right-column player strip, which renders exactly when the queue column is hidden — the idle feed title SHALL display as today.

#### Scenario: Idle state shows feed title

- **WHEN** nothing is playing and no remote session is connected, at least one feed item has been fetched, and the playback panel is rendered
- **THEN** the playback panel title row SHALL display the title of the current feed item

#### Scenario: Idle queue-only shows no feed title

- **WHEN** the queue column is visible (queue-only or two-panel) and nothing is playing
- **THEN** the playback panel SHALL NOT be rendered and no feed title SHALL be displayed; the queue panel SHALL occupy those rows

#### Scenario: Active playback hides feed

- **WHEN** playback becomes active (local or remote)
- **THEN** the feed title SHALL be hidden and the normal now-playing title SHALL be displayed instead

#### Scenario: No feed items yet shows blank space

- **WHEN** nothing is playing but no feed items have been fetched yet (startup, or fetch pending) and the playback panel is rendered
- **THEN** the playback panel SHALL render the existing blank bar, unchanged from current behavior

### Requirement: Idle feed open-link command follows the display

The command that opens the current idle feed item's link SHALL be offered only where the idle feed title can be displayed, which is only where the playback panel renders. In a queue-visible layout with nothing playing the panel is collapsed, so the command SHALL NOT fire there; where the panel renders (the right-column player strip), it SHALL fire as today.

#### Scenario: Open-link suppressed in queue-only idle

- **WHEN** the queue column is visible (queue-only or two-panel) with no playback active, an idle feed item with a link is current, and the user presses the open-feed-link key
- **THEN** no browser SHALL be opened

#### Scenario: Open-link unchanged in two-panel idle

- **WHEN** the layout is the two-panel layout with nothing playing, no remote session connected, and an idle feed item with a link is current
- **THEN** the open-feed-link key SHALL NOT open that item's link, because the layout's playback panel is collapsed while idle

#### Scenario: Open-link fires where the panel renders

- **WHEN** the layout is library-only with nothing playing, no remote session connected, and an idle feed item with a link is current
- **THEN** the open-feed-link key SHALL open that item's link, as today

#### Scenario: Rotation continues while hidden

- **WHEN** the idle feed title is not displayed because the queue-visible layout collapsed the panel
- **THEN** feed fetching and rotation SHALL continue unchanged; only the display and its command are gated
