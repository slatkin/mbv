## REMOVED Requirements

### Requirement: Ctrl+PageDown jumps to next artist
**Reason**: Artist headers are no longer selectable navigation targets.
**Migration**: Navigate between album rows with the existing arrow and paging controls; Ctrl+PageDown no longer performs an artist jump.

### Requirement: Ctrl+PageUp jumps to previous artist
**Reason**: Artist headers are no longer selectable navigation targets.
**Migration**: Navigate between album rows with the existing arrow and paging controls; Ctrl+PageUp no longer performs an artist jump.

### Requirement: Scroll follows cursor
**Reason**: Artist-jump cursor movement is removed, so it no longer has a scroll-follow behavior.
**Migration**: Existing album cursor movement continues to keep the selected album visible.

## MODIFIED Requirements

### Requirement: Only active in grouped album view
Ctrl+PageUp and Ctrl+PageDown SHALL NOT select artist headers or page album rows. The modified keys SHALL remain consumed as unmapped no-ops in grouped and non-grouped library views.

#### Scenario: Grouped album view
- **WHEN** Ctrl+PageUp or Ctrl+PageDown is pressed in a grouped music album view
- **THEN** the selected album and scroll position remain unchanged

#### Scenario: Non-grouped view
- **WHEN** Ctrl+PageUp or Ctrl+PageDown is pressed in a non-grouped library view
- **THEN** the key press is handled as a no-op and is not passed through
