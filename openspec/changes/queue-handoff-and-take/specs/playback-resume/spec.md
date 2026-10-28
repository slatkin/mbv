# Spec Delta

## MODIFIED Requirements

### Requirement: Each medium has one source of resume progress

The position mbv resumes from SHALL come from one source per medium. For an Emby video item it SHALL be the position Emby reports for that item when the start position is decided. For an Audiobookshelf item it SHALL be the position returned by the Audiobookshelf playback session opened to play it. For a feed entry it SHALL be the locally recorded position. No locally stored or remembered position SHALL be used as the resume position for Emby or Audiobookshelf media. The one-percent eligibility rule SHALL apply to the position from that source. One exception applies: the start slot of a queue replacement that carries a start position (Hand off or Take) SHALL start at that carried position for every medium, without the one-percent rule; every later start in that queue SHALL use the per-medium source.

#### Scenario: Restored queue resumes the server position

- **WHEN** mbv restarts with a restored queue whose Emby video slot Emby reports at 37% of its runtime
- **AND** the user plays that slot
- **THEN** playback SHALL start at Emby's position

#### Scenario: Progress made on another machine

- **WHEN** a long-running Stay-alive process last played an Emby video to 20%
- **AND** the same item was later played to 60% on another machine
- **AND** the user plays it again on the first machine
- **THEN** playback SHALL start at 60%

#### Scenario: Emby position below one percent

- **WHEN** Emby reports a positive position below 1% of the item's runtime
- **THEN** playback SHALL start from the beginning

#### Scenario: Audiobookshelf resumes from its session

- **WHEN** an Audiobookshelf episode or book is played
- **THEN** playback SHALL start at the position returned by the Audiobookshelf playback session

#### Scenario: Feed entry resumes locally

- **WHEN** a feed entry with a locally recorded position is played
- **THEN** playback SHALL start from that local position under the one-percent rule

#### Scenario: Hand off continues a song mid-way

- **WHEN** an Emby audio track plays at 1:30 and the user hands it off to a peer
- **THEN** the peer SHALL start that track at 1:30, not from the beginning

#### Scenario: Later slots after a Hand off use their own source

- **WHEN** the handed-off start slot ends and the peer advances to the next feed entry
- **THEN** that entry SHALL start from the peer's locally recorded position under the one-percent rule
