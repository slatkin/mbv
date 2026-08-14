## ADDED Requirements

### Requirement: Audiobookshelf playback and progress synchronization continue across client exits
When a daemon owner has an Audiobookshelf episode active, playback and periodic progress synchronization SHALL continue uninterrupted after every attached client exits. Session finalization, bounded retry, and queue advancement SHALL proceed from the daemon owner without requiring a client to be present, consistent with how non-Audiobookshelf media remains active under the existing stay-alive lifecycle.

#### Scenario: Last client exits while Audiobookshelf episode is active
- **WHEN** the last attached client exits while the daemon owner is playing an Audiobookshelf episode
- **THEN** playback SHALL continue and periodic synchronization SHALL proceed on its normal interval from the daemon owner
- **THEN** no Audiobookshelf finalization or queue mutation SHALL be triggered by client exit alone

#### Scenario: A later client attaches while daemon is playing an Audiobookshelf episode
- **WHEN** a later capable client attaches to a daemon owner that is playing an Audiobookshelf episode
- **THEN** the client SHALL receive the live canonical queue, active slot, status, and last-broadcast acknowledged Audiobookshelf progress via the existing attach snapshot
- **THEN** the daemon owner's playback and synchronization authority SHALL not be transferred to the attaching client
