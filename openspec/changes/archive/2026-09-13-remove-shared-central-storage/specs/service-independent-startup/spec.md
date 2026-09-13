## REMOVED Requirements

### Requirement: Optional shared state cannot gate local operation

**Reason**: The requirement was written to defend Service-independent startup against an optional shared-state endpoint. No shared-state endpoint exists any more, so the condition it guards against cannot occur; keeping the requirement would assert behavior about a facility that has been removed.

**Migration**: Replaced by "Local feed-entry state cannot gate startup or playback" below, which states the same guarantee directly against the local feed-state file that now holds that state.

## ADDED Requirements

### Requirement: Local feed-entry state cannot gate startup or playback

Feed-entry playback state SHALL be stored locally and SHALL NOT be required for startup, browsing, or playback. Absence, unreadability, or unparseability of the local feed-state file SHALL NOT produce a startup failure, block feed browsing, or block playback; the client SHALL continue with unplayed, zero-position entries. mbv SHALL NOT require an account or a database service for feed-entry state.

#### Scenario: Feed-only client starts with no state file

- **WHEN** mbv starts with feed subscriptions and no Emby setup and no feed-entry-state file exists
- **THEN** it SHALL treat every entry as unplayed with zero position
- **THEN** it SHALL remain fully usable for browsing and playback

#### Scenario: Local feed-entry state cannot be read

- **WHEN** the feed-entry-state file is unreadable or invalid
- **THEN** startup and playback SHALL continue
- **THEN** the failure SHALL be recorded without presenting the Feeds tab as unavailable
