## Purpose

Defines application entry and Remote Service initialization without making any Service credential or network dependency a prerequisite for using mbv.

## ADDED Requirements

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

### Requirement: Optional shared state cannot gate local operation
Absence or failure of an optional shared-state endpoint SHALL NOT prevent Service-independent startup, local feed state, browsing, or playback. This change SHALL NOT introduce an mbv account or require a redb-backed service.

#### Scenario: Feed-only client has no shared-state endpoint
- **WHEN** mbv starts with feed subscriptions but without Emby or a shared-state endpoint
- **THEN** it SHALL use local feed state and remain fully usable

#### Scenario: Configured shared state cannot authenticate
- **WHEN** shared state is configured but cannot authenticate because Emby is absent or unavailable
- **THEN** mbv SHALL use its existing local fallback behavior
- **THEN** startup and local playback SHALL continue
