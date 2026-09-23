# service-independent-startup Specification

## Purpose
Defines application entry and Remote Service initialization without making any Service credential or network dependency a prerequisite for using mbv.
## Requirements
### Requirement: TUI entry is independent of Remote Services
mbv SHALL enter its TUI without requiring an Emby or Audiobookshelf Service to be configured, authenticated, or reachable. Remote Service failure SHALL NOT terminate startup or redirect the user to a pre-application login form.

#### Scenario: No Remote Service is configured
- **WHEN** mbv starts without a configured Emby or Audiobookshelf Service
- **THEN** the TUI SHALL open normally
- **THEN** feed setup, browsing, and playback SHALL remain available

#### Scenario: Emby is unreachable
- **WHEN** mbv starts with a configured Emby Service whose server cannot be reached
- **THEN** the TUI SHALL remain open
- **THEN** Emby SHALL enter Unavailable without clearing its Service credential

#### Scenario: Emby rejects its credential
- **WHEN** mbv starts with a configured Emby Service whose server rejects its credential
- **THEN** the TUI SHALL remain open
- **THEN** Emby SHALL enter Needs authentication

#### Scenario: Destination Latest is available with only Audiobookshelf or Feeds configured
- **WHEN** mbv starts with an Audiobookshelf Service, feed subscriptions, or both, and no Emby Service configured
- **THEN** the Audiobookshelf podcast tab's `Latest` pill and the Feeds tab's `Latest` pill SHALL show their available data without an Emby-related error and without requiring an Emby Service to become available
- **THEN** the Home tab SHALL show Continue Watching, which MAY remain empty since it stays Emby-only

### Requirement: Remote Services initialize after TUI entry
mbv SHALL begin each configured Remote Service's connection independently after the TUI has started. One Service's connection attempt or failure SHALL NOT delay another Service or the Feeds Service.

#### Scenario: Emby connects successfully
- **WHEN** the TUI has started with a configured and valid Emby Service
- **THEN** Emby SHALL transition through Connecting to Ready
- **THEN** its existing library and playback features SHALL become available

#### Scenario: One Remote Service is unavailable
- **WHEN** one configured Remote Service cannot connect
- **THEN** other Services SHALL continue initializing and operating independently

### Requirement: Empty setup opens Services settings
When no Remote Service is configured and the Feeds Service has no subscriptions, mbv SHALL initially focus the Services view within Settings. This routing SHALL NOT create a separate setup wizard or prevent navigation elsewhere in the TUI.

#### Scenario: First launch has no content setup
- **WHEN** mbv starts with no configured Remote Service and no feed subscriptions
- **THEN** the TUI SHALL open directly to the Services settings view
- **THEN** the user SHALL be able to leave that view without configuring a Service

#### Scenario: Existing content setup is present
- **WHEN** mbv starts with at least one configured Remote Service or feed subscription
- **THEN** mbv SHALL use its ordinary content-oriented initial navigation

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

