## MODIFIED Requirements

### Requirement: Ready Audiobookshelf discovers accessible podcast libraries
After Audiobookshelf becomes Ready, mbv SHALL discover the authenticated user's accessible Audiobookshelf libraries using the Audiobookshelf 2.36 API contract. It SHALL expose podcast libraries for browsing through this capability and book libraries for browsing through the `audiobookshelf-book-browsing` capability.

#### Scenario: User has accessible podcast libraries
- **WHEN** Audiobookshelf becomes Ready for a user with one or more accessible podcast libraries
- **THEN** mbv SHALL load those podcast libraries without waiting for Emby or Feeds
- **THEN** each discovered podcast library SHALL become available as a content tab

#### Scenario: User has only audiobook libraries
- **WHEN** Audiobookshelf becomes Ready for a user whose accessible libraries are all book libraries
- **THEN** Audiobookshelf SHALL remain Ready
- **THEN** mbv SHALL add a content tab for each book library through the `audiobookshelf-book-browsing` capability rather than adding no tab

#### Scenario: Audiobookshelf is the only configured content Service
- **WHEN** mbv starts with configured Audiobookshelf content and no configured Emby Service or feed subscriptions
- **THEN** mbv SHALL enter its ordinary content UI rather than opening Services settings as though no content Service were configured
- **THEN** Audiobookshelf initialization and discovery SHALL occur for both bare-mode and attached Local daemon clients

#### Scenario: Catalog request explicitly rejects the credential
- **WHEN** an authenticated catalog request explicitly rejects the persisted Audiobookshelf credential
- **THEN** Audiobookshelf SHALL enter Needs authentication through the existing Service lifecycle
- **THEN** mbv SHALL remove its Audiobookshelf tabs and catalog content while preserving non-secret setup

#### Scenario: Catalog request is unavailable or incompatible
- **WHEN** library discovery cannot complete because the server is unavailable or does not satisfy the required Audiobookshelf 2.36 contract
- **THEN** mbv SHALL present Audiobookshelf as unavailable with a concise retryable or compatibility result
- **THEN** mbv SHALL preserve the configured setup and credential and SHALL NOT use an older-server fallback
