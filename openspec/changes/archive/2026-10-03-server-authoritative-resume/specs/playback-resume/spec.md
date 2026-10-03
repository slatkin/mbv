# Spec Delta

## ADDED Requirements

### Requirement: Each medium has one source of resume progress

The position mbv resumes from SHALL come from one source per medium. For an Emby video item it SHALL be the position Emby reports for that item when the start position is decided. For an Audiobookshelf item it SHALL be the position returned by the Audiobookshelf playback session opened to play it. For a feed entry it SHALL be the locally recorded position. No locally stored or remembered position SHALL be used as the resume position for Emby or Audiobookshelf media. The one-percent eligibility rule SHALL apply to the position from that source.

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

### Requirement: A failed Emby position fetch retries, then starts from the beginning

When fetching an Emby video item's position fails, mbv SHALL retry the fetch automatically, making at most three attempts in total. A response that does not contain the requested item SHALL count as a failure. If every attempt fails, playback SHALL start from the beginning, and SHALL NOT fail or wait for user action because of the fetch. Emby audio items SHALL NOT be fetched for a position, because they always start from the beginning.

#### Scenario: Fetch succeeds on retry

- **WHEN** the first position fetch for an Emby video fails and the second succeeds
- **THEN** playback SHALL start from the fetched position under the one-percent rule

#### Scenario: Every attempt fails

- **WHEN** all three position fetch attempts for an Emby video fail
- **THEN** playback SHALL start from the beginning
- **AND** playback SHALL NOT report an error for the failed fetch

#### Scenario: Item missing from the response

- **WHEN** Emby answers the fetch successfully but does not include the requested item
- **THEN** mbv SHALL treat that attempt as failed and retry it
