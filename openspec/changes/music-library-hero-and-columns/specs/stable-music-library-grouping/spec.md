## MODIFIED Requirements

### Requirement: Grouped-view continuity

When a settled grouped snapshot is replaced, the system SHALL preserve the current album selection by stable album identity when that album remains present. It SHALL retain the closest practical viewport position around that selection and SHALL ensure artist-header actions use the same group membership that is visible to the user.

The grouped album view SHALL operate within the shared library list renderer's hero and list area split. The hero displays the selected album's detail; the list area displays the grouped album rows. Grouping stability — settled snapshots, atomic replacement, selection preservation — SHALL be maintained identically regardless of whether the grouped view renders through the shared list renderer or formerly through its own dedicated view.

#### Scenario: Selected album survives a replacement

- **WHEN** a replacement snapshot contains the album selected in the prior snapshot
- **THEN** that album remains selected and remains visible after the replacement is committed

#### Scenario: Selected album is absent from a replacement

- **WHEN** the selected album is not present in a replacement snapshot
- **THEN** the system selects a valid album using its normal fallback selection behavior

#### Scenario: Artist header action follows the visible grouping

- **WHEN** the user invokes an artist-header action on a settled grouped view
- **THEN** the action operates on exactly the albums shown under that header in the settled snapshot

#### Scenario: Grouping stability preserved in the shared renderer

- **WHEN** the grouped album view renders through the shared library list renderer
- **THEN** settled snapshot behavior (no reordering from late metadata, atomic replacement) is identical to the former dedicated view
