# playback-resume Specification

## Purpose
Defines when saved playback progress is substantial enough for mbv to resume
an Emby item or feed entry instead of restarting it from the beginning.
## Requirements
### Requirement: Resume requires six percent progress

For any media with a known runtime, mbv SHALL treat a positive saved playback
position as resumable only when it is at least 6% of runtime. Exactly 6% SHALL
qualify. A positive saved position with unknown runtime SHALL remain resumable.
Zero and negative saved positions SHALL NOT qualify.

#### Scenario: Position is below six percent

- **WHEN** an item with known runtime has positive saved progress below 6%
- **THEN** mbv SHALL start the item from its beginning rather than resuming

#### Scenario: Position is exactly six percent

- **WHEN** an item with known runtime has saved progress equal to 6%
- **THEN** mbv SHALL resume from the saved position

#### Scenario: Position is above six percent

- **WHEN** an item with known runtime has saved progress above 6%
- **THEN** mbv SHALL resume from the saved position

#### Scenario: Runtime is unknown

- **WHEN** an item has positive saved progress and unknown runtime
- **THEN** mbv SHALL treat the position as resumable

#### Scenario: Position is not positive

- **WHEN** an item's saved position is zero or negative
- **THEN** mbv SHALL start the item from its beginning

### Requirement: Emby and feed playback share one resume rule

mbv SHALL apply the same resume eligibility rule to Emby video items and feed
entries. The item kind SHALL NOT change the 6% boundary or unknown-runtime
behavior.

#### Scenario: Equivalent Emby and feed progress

- **WHEN** an Emby video item and a feed entry have equal positive positions
  and equal runtimes
- **THEN** mbv SHALL make the same resume-or-restart decision for both

