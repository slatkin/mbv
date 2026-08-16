## MODIFIED Requirements

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

#### Scenario: Home browsing is available with only Audiobookshelf or Feeds configured
- **WHEN** mbv starts with an Audiobookshelf Service, feed subscriptions, or both, and no Emby Service configured
- **THEN** the Home tab SHALL show Audiobookshelf and Feeds Latest pills with their available data
- **THEN** Home SHALL NOT show an Emby-related error and SHALL NOT require an Emby Service to become available
- **THEN** the Continue Watching section MAY remain empty, since it stays Emby-only
