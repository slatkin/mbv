## MODIFIED Requirements

### Requirement: Navigation is single-flight
Next and Previous SHALL each remain single-flight until the requested track change reaches Applied, Rejected, or Superseded. An equivalent repeated navigation input during that interval SHALL be coalesced. The Playback run SHALL confirm the transition through the same TrackChanged observation path regardless of whether the Playback run uses a full mpv playlist or owner-driven active-file projection.

#### Scenario: Repeated Next before confirmation
- **WHEN** Next is invoked again before the first Next changes the confirmed current item
- **THEN** the repeated request is coalesced and playback advances by only one item

#### Scenario: Next after confirmation
- **WHEN** the first Next has changed the confirmed current item and the user invokes Next again
- **THEN** a new navigation intent is accepted for the following item

#### Scenario: Repeated Previous before confirmation
- **WHEN** Previous is invoked again before the first Previous changes the confirmed current item
- **THEN** the repeated request is coalesced and playback moves back by only one item

#### Scenario: Active-file JumpTo confirms via TrackChanged
- **WHEN** a JumpTo command completes in active-file mode (owner-driven single-item projection)
- **THEN** the Playback run SHALL emit a TrackChanged observation carrying the transition's request identity and the target slot
- **AND** the Player owner SHALL settle the transition and update the observed active slot from that observation

#### Scenario: Next advances sequentially in active-file mode
- **WHEN** the user presses Next three times in active-file mode, waiting for each to confirm before pressing again
- **THEN** each Next SHALL advance the observed active slot by one position from the previous confirmation
- **AND** the daemon SHALL resolve each subsequent Next from the updated observed position
