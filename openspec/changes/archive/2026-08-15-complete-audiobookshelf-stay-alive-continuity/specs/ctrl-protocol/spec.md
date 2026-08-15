## ADDED Requirements

### Requirement: Active daemon playback emits acknowledged Audiobookshelf progress
An active daemon Player owner SHALL emit the provider-qualified Audiobookshelf progress event whenever it acknowledges owned playback progress, carrying episode identity, acknowledged position and completion state, and setup generation. The emission SHALL remain gated per connection so only peers that negotiated the capability receive it, and SHALL contain no API key, Authorization header, resolved URL, or playback `sessionId`.

#### Scenario: Owner acknowledges progress with a capable client attached
- **WHEN** an active daemon owner acknowledges Audiobookshelf progress for the active episode and a capable client is attached
- **THEN** the daemon SHALL emit the provider-qualified progress event to that client with current setup generation

#### Scenario: Owner acknowledges completion
- **WHEN** the owner acknowledges final progress at natural episode completion
- **THEN** the emitted event SHALL carry the finished state and acknowledged position

#### Scenario: No capable client is attached
- **WHEN** the owner acknowledges progress while no attached peer negotiated the Audiobookshelf progress capability
- **THEN** the owner SHALL continue synchronizing and finalizing playback and SHALL emit no substitute unknown event

## REMOVED Requirements

### Requirement: Progress transport remains dormant before playback activation
**Reason**: This final milestone-4 child (#528) activates the daemon-owner emission and client reconciliation that the transport child (#525) intentionally left dormant. Dormancy no longer holds once daemon-owned Audiobookshelf playback and its client-facing progress loop are active.
**Migration**: The dormancy constraint is superseded by "Active daemon playback emits acknowledged Audiobookshelf progress" (this delta) and by the client reconciliation requirements in `audiobookshelf-podcast-playback`. The event shape, capability strings, and per-connection gating from #525 are unchanged; only the "no emission" constraint is lifted.
