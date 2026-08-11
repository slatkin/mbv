# artist-keyboard-navigation Specification

## Purpose

Defines how modified PageUp and PageDown keys behave in library views, including the grouped music album view.
## Requirements
### Requirement: Ctrl+PageDown jumps to next artist
The system SHALL move the library cursor to the next artist header in the grouped album display target list when Ctrl+PageDown is pressed in the library panel.

#### Scenario: Jump to next artist from album
- **WHEN** the cursor is on an album row and Ctrl+PageDown is pressed
- **THEN** the cursor moves to the next artist header after the current position

#### Scenario: Jump to next artist from artist header
- **WHEN** the cursor is on an artist header and Ctrl+PageDown is pressed
- **THEN** the cursor moves to the next artist header after the current one

#### Scenario: No next artist exists
- **WHEN** Ctrl+PageDown is pressed and there is no subsequent artist header
- **THEN** the cursor moves to the last item in the target list

### Requirement: Ctrl+PageUp jumps to previous artist
The system SHALL move the library cursor to the previous artist header in the grouped album display target list when Ctrl+PageUp is pressed in the library panel.

#### Scenario: Jump to previous artist from album
- **WHEN** the cursor is on an album row and Ctrl+PageUp is pressed
- **THEN** the cursor moves to the nearest artist header before the current position

#### Scenario: Jump to previous artist from artist header
- **WHEN** the cursor is on an artist header and Ctrl+PageUp is pressed
- **THEN** the cursor moves to the artist header immediately before the current one

#### Scenario: No previous artist exists
- **WHEN** Ctrl+PageUp is pressed and there is no preceding artist header
- **THEN** the cursor moves to the first item in the target list

### Requirement: Scroll follows cursor
The system SHALL update the scroll offset so the new cursor position is visible after an artist jump.

#### Scenario: Cursor jumps beyond visible area
- **WHEN** an artist jump places the cursor outside the current viewport
- **THEN** the scroll offset adjusts to show the cursor

### Requirement: Only active in grouped album view
The system SHALL only activate Ctrl+PageUp/PageDown artist navigation when the library is in the grouped album (music group) view.

#### Scenario: Non-grouped view
- **WHEN** Ctrl+PageUp or Ctrl+PageDown is pressed in a non-grouped library view
- **THEN** the key press is handled as a no-op (swallowed, not passed through)
