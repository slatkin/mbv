# playback-resume Specification

## Purpose
Defines when saved playback progress is substantial enough for mbv to resume
an Emby item or feed entry instead of restarting it from the beginning.

## Requirements

### Requirement: Resume requires one percent progress

For any media with a known runtime, mbv SHALL treat a positive saved playback
position as resumable only when it is at least 1% of runtime. Exactly 1% SHALL
qualify. A positive saved position with unknown runtime SHALL remain resumable.
Zero and negative saved positions SHALL NOT qualify.

#### Scenario: Position is below one percent

- **WHEN** an item with known runtime has positive saved progress below 1%
- **THEN** mbv SHALL start the item from its beginning rather than resuming

#### Scenario: Position is exactly one percent

- **WHEN** an item with known runtime has saved progress equal to 1%
- **THEN** mbv SHALL resume from the saved position

#### Scenario: Position is above one percent

- **WHEN** an item with known runtime has saved progress above 1%
- **THEN** mbv SHALL resume from the saved position

#### Scenario: Runtime is unknown

- **WHEN** an item has positive saved progress and unknown runtime
- **THEN** mbv SHALL treat the position as resumable

#### Scenario: Position is not positive

- **WHEN** an item's saved position is zero or negative
- **THEN** mbv SHALL start the item from its beginning

### Requirement: Emby and feed playback share one resume rule

mbv SHALL apply the same resume eligibility rule to Emby video items and feed
entries. The item kind SHALL NOT change the 1% boundary or unknown-runtime
behavior.

#### Scenario: Equivalent Emby and feed progress

- **WHEN** an Emby video item and a feed entry have equal positive positions
  and equal runtimes
- **THEN** mbv SHALL make the same resume-or-restart decision for both

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

### Requirement: A stopped Emby video's report precedes the next position fetch

When playback of an Emby video ends, mbv SHALL send its stop report, or wait a bounded time for it, before a later start of playback fetches an Emby position. The wait SHALL be bounded. When the bound passes, mbv SHALL fetch anyway and SHALL NOT fail or wait for user action because of the wait.

#### Scenario: Replay right after a stop

- **WHEN** the user stops an Emby video at 40% of its runtime
- **AND** the user plays the same item again before the stop report has reached Emby
- **THEN** playback SHALL start at 40%, the position the stop report sets

#### Scenario: Replay right after a finish

- **WHEN** an Emby video finishes and the run ends
- **AND** the user plays the same item again before the stop report has reached Emby
- **THEN** playback SHALL start from the position Emby holds after that report, not from a position reported before it

#### Scenario: The stop report is slow

- **WHEN** the stop report has not been sent when the bounded wait ends
- **THEN** mbv SHALL fetch the position anyway
- **AND** playback SHALL start from the fetched position under the one-percent rule

#### Scenario: Nothing is queued

- **WHEN** no stop report is queued for delivery
- **THEN** mbv SHALL fetch the position with no added wait
