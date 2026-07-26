## ADDED Requirements

### Requirement: Queue panel has no top border row
The queue panel SHALL render without a top border row. Only the bottom border row (`▁`, LOWER ONE EIGHTH BLOCK) is drawn; the row previously used for the top border (`▔`, UPPER ONE EIGHTH BLOCK) is returned to the queue list's content area instead of being reserved as a border.

#### Scenario: Queue panel top row is content, not a border
- **WHEN** the queue panel is rendered at any focus state
- **THEN** the top row of the panel's area is not painted with the `▔` border glyph, and the queue list's visible content area includes that row (one row taller than before)

#### Scenario: Bottom border is unaffected
- **WHEN** the queue panel is rendered with height greater than 1
- **THEN** the bottom row of the panel's area is still painted with the `▁` border glyph, unchanged from before

### Requirement: Queue group headers require a minimum run of 3 items
The queue list SHALL only display a group header (and its associated spacer) above a run of consecutive items sharing the same grouping key (album for audio tracks, series for episodes) when that run contains at least 3 items. Runs of 1 or 2 items render as plain track rows, identical in presentation to ungrouped items.

#### Scenario: Run of 3 or more same-key items shows a header
- **WHEN** the queue contains 3 or more consecutive audio tracks from the same album (or 3 or more consecutive episodes from the same series)
- **THEN** a group header naming that album/series is rendered above the run, followed by the run's track rows

#### Scenario: Run of 1 or 2 same-key items shows no header
- **WHEN** the queue contains only 1 or 2 consecutive items sharing the same album or series
- **THEN** no group header or extra spacer is rendered for that run; the item(s) render as plain track rows

#### Scenario: Grouping key and header label are unchanged
- **WHEN** a header is shown for a qualifying run (3+ items)
- **THEN** its grouping key and label text are computed exactly as before this change (album id / "Artist: Album" for audio, series name for episodes)
