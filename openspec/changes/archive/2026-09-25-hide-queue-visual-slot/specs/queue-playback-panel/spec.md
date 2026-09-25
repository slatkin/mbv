## ADDED Requirements

### Requirement: The user can hide the visual slot

The user SHALL be able to hide and show the QueueColumn's visual slot (artwork, placeholder, loading
reservation, or visualizer) with the `hide_visual_slot` keyboard action, whose default chord is `h`.
While hidden, the visual slot SHALL NOT be rendered and SHALL reserve zero rows and zero columns; the
header row and the playback panel's transport SHALL remain rendered whenever they would be without
this action. Below 100 columns the transport SHALL begin on the row directly below the header row.
At 100 columns or more the transport SHALL span the full width of the slot region, and the band
above the Queue panel SHALL be the transport's own height. The Queue panel SHALL take every row the
slot released. The action SHALL toggle the hidden state in every playback state (idle included) and
every panel mode; the state SHALL take effect wherever the QueueColumn is rendered. The hidden state
SHALL persist across launches; a first launch with no saved state SHALL show the slot. Idle
collapse is unchanged: while idle the slot reserves zero rows whether or not it is hidden.

#### Scenario: Hiding the slot below 100 columns

- **WHEN** playback is active, the QueueColumn is narrower than 100 columns, and the user presses `h`
- **THEN** the visual slot SHALL NOT render, the transport SHALL begin directly below the header row,
  and the Queue panel SHALL begin below the transport and its separator row

#### Scenario: Hiding the slot at 100 columns or more

- **WHEN** playback is active, the QueueColumn is 100 columns or wider, and the user presses `h`
- **THEN** the visual slot SHALL NOT render, the transport SHALL span the slot region's full width,
  and the Queue panel SHALL begin below the transport's own rows and the separator row

#### Scenario: Showing the slot again

- **WHEN** the visual slot is hidden and the user presses `h`
- **THEN** the visual slot SHALL render again with the currently selected content (artwork or
  visualizer) and the Queue panel SHALL move below it

#### Scenario: Hidden state survives a restart

- **WHEN** the user hides the visual slot and later relaunches mbv
- **THEN** the visual slot SHALL be hidden at launch

#### Scenario: Toggling while idle

- **WHEN** no transport is active and the user presses `h`
- **THEN** the layout SHALL NOT change, and the toggled hidden state SHALL apply when playback
  next starts

#### Scenario: Text entry keeps `h`

- **WHEN** a text-entry surface owns focus and the user types `h`
- **THEN** the character SHALL reach the text field and the hidden state SHALL NOT change

#### Scenario: A focused library screen does not intercept `h`

- **WHEN** a library screen that binds `h` locally has focus and the user presses `h`
- **THEN** the visual slot's hidden state SHALL toggle and the screen's local `h` action SHALL NOT run
