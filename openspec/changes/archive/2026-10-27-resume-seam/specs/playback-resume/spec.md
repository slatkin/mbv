# Spec Delta

## ADDED Requirements

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
