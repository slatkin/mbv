## ADDED Requirements

### Requirement: Shared focus-aware primary text color utility
The render module SHALL provide a `focused_or_subtle(focused: bool) -> Color` function that returns `palette::WHITE` when focused and `palette::SUBTLE` when not focused. All list renderers SHALL use this function (or its equivalent logic) for primary item text color.

#### Scenario: Focused panel primary text
- **WHEN** a list panel has focus (`focused == true`)
- **THEN** primary item text SHALL render in `palette::WHITE` (RGB 248, 245, 228)

#### Scenario: Unfocused panel primary text
- **WHEN** a list panel does not have focus (`focused == false`)
- **THEN** primary item text SHALL render in `palette::SUBTLE` (RGB 158, 158, 158)

### Requirement: Shared focus-aware accent text color utility
The render module SHALL provide a `focused_or_muted(focused: bool) -> Color` function that returns `palette::YELLOW` when focused and `palette::MUTED` when not focused. This SHALL be used for accent elements such as separator characters.

#### Scenario: Focused panel accent text
- **WHEN** a list panel has focus
- **THEN** accent text (e.g., " • " separators) SHALL render in `palette::YELLOW` (RGB 219, 188, 127)

#### Scenario: Unfocused panel accent text
- **WHEN** a list panel does not have focus
- **THEN** accent text SHALL render in `palette::MUTED` (RGB 108, 108, 108)

### Requirement: Shared focus-aware secondary accent color utility
The render module SHALL provide a `focused_aqua_or_muted(focused: bool) -> Color` function that returns `palette::AQUA` when focused and `palette::MUTED` when not focused. This SHALL be used for secondary accent elements such as year labels.

#### Scenario: Focused panel secondary accent
- **WHEN** a list panel has focus
- **THEN** secondary accent text (e.g., year labels) SHALL render in `palette::AQUA` (RGB 53, 167, 124)

#### Scenario: Unfocused panel secondary accent
- **WHEN** a list panel does not have focus
- **THEN** secondary accent text SHALL render in `palette::MUTED` (RGB 108, 108, 108)

### Requirement: Music album list focus dimming
The `render_power_grouped_album_rows` function SHALL apply focus-aware colors to all non-selected text elements:
- Album titles: `WHITE` when focused, `SUBTLE` when unfocused
- Year labels: `AQUA` when focused, `MUTED` when unfocused
- Separator characters (" • "): `YELLOW` when focused, `MUTED` when unfocused
- Artist header labels (non-selected): `YELLOW` when focused, `SUBTLE` when unfocused

This applies to both the grouped-block display path and the non-grouped (legacy) display path.

#### Scenario: Music album list with focused panel
- **WHEN** the music library panel has focus and an album is not selected
- **THEN** the album title SHALL be `WHITE`, the year SHALL be `AQUA`, and the separator SHALL be `YELLOW`

#### Scenario: Music album list with unfocused panel
- **WHEN** the music library panel does not have focus and an album is not selected
- **THEN** the album title SHALL be `SUBTLE`, the year SHALL be `MUTED`, and the separator SHALL be `MUTED`

#### Scenario: Music album list selected item preserves highlight
- **WHEN** an album is selected (regardless of focus state)
- **THEN** the selected item SHALL retain its `FOAM` + BOLD styling and SHALL NOT be dimmed

#### Scenario: Non-grouped album list selected-but-unfocused item preserves highlight
- **WHEN** an album is selected in the non-grouped display path and the panel is not focused
- **THEN** the selected item SHALL retain its `FOAM` + BOLD styling and SHALL NOT be dimmed

#### Scenario: Non-grouped album list focus dimming
- **WHEN** the non-grouped album list panel has focus and an album is not selected
- **THEN** the album title SHALL be `WHITE`, the year SHALL be `AQUA`, and the separator SHALL be `YELLOW`

#### Scenario: Non-grouped album list unfocused dimming
- **WHEN** the non-grouped album list panel does not have focus and an album is not selected
- **THEN** the album title SHALL be `SUBTLE`, the year SHALL be `MUTED`, and the separator SHALL be `MUTED`

### Requirement: Home video list focus dimming consistency
The `render_home_video_item` function SHALL use `SUBTLE` for unfocused non-selected item titles, consistent with all other list renderers. The current `TEXT` fallback SHALL be replaced with focus-aware logic.

#### Scenario: Home video item when panel is unfocused
- **WHEN** the home video panel does not have focus and the item is not selected or expanded
- **THEN** the item title SHALL render in `palette::SUBTLE`

#### Scenario: Home video item when panel is focused
- **WHEN** the home video panel has focus and the item is not selected or expanded
- **THEN** the item title SHALL render in `palette::TEXT` or `palette::WHITE`

#### Scenario: Home video selected item preserves highlight
- **WHEN** a home video item is selected and focused
- **THEN** the item title SHALL retain its `IRIS` + BOLD styling

### Requirement: Album art and track detail rendering unaffected
The focus-dimming changes SHALL NOT alter the rendering of album art or track detail rows. These features are structurally separate from the list row rendering paths and must remain unaffected.

#### Scenario: Inline album art rendering unaffected
- **WHEN** album art is displayed inline and the panel focus state changes
- **THEN** the album art SHALL render identically to before the focus-dimming changes

#### Scenario: Track detail expanded rendering unaffected
- **WHEN** a track detail row is expanded and the panel focus state changes
- **THEN** the track detail row SHALL render identically to before the focus-dimming changes
