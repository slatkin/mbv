# audiobookshelf-service-setup Specification

## Purpose
Defines secure Audiobookshelf API-key setup, identity validation, runtime availability, and connection testing without introducing catalog or playback behavior.
## Requirements
### Requirement: Audiobookshelf setup validates an API-key identity before commit
mbv SHALL accept an Audiobookshelf server URL and API key through Services settings, send the key as `Authorization: Bearer <api-key>` to `GET /api/me`, and commit the setup only when the response confirms the associated active user. Audiobookshelf setup SHALL NOT offer a username/password flow.

#### Scenario: New Audiobookshelf setup succeeds
- **WHEN** the user submits a server URL and API key that `/api/me` accepts for an active user
- **THEN** mbv SHALL persist the validated Audiobookshelf setup and Service credential
- **THEN** Audiobookshelf SHALL become Ready

#### Scenario: New Audiobookshelf credential is rejected
- **WHEN** the user submits an API key that the configured server rejects
- **THEN** mbv SHALL report the authentication failure without persisting the attempted setup or API key
- **THEN** the setup input SHALL remain available for correction

#### Scenario: New Audiobookshelf server is unreachable
- **WHEN** `/api/me` cannot validate a submitted setup because the server is unreachable
- **THEN** mbv SHALL report the connectivity failure without persisting the attempted setup or API key

#### Scenario: Failed candidate setup does not replace working setup
- **WHEN** validation of a candidate server URL or API key fails while Audiobookshelf already has a working setup
- **THEN** mbv SHALL preserve the working setup, credential, runtime identity, and Service-owned state

### Requirement: Audiobookshelf credentials remain isolated local secrets
mbv SHALL persist the Audiobookshelf API key only in Audiobookshelf's mode-`0600` Service secret file. The API key SHALL NOT be written to general configuration, ctrl messages, logs, or shared-state storage.

#### Scenario: Audiobookshelf setup is persisted
- **WHEN** Audiobookshelf setup validates successfully
- **THEN** mbv SHALL write the API key to Audiobookshelf's Service secret file with mode `0600`
- **THEN** non-secret setup SHALL identify the server without containing the API key

#### Scenario: Diagnostic output describes an Audiobookshelf failure
- **WHEN** mbv reports or logs an Audiobookshelf request failure
- **THEN** the diagnostic SHALL NOT contain the API key or the request's Authorization value

### Requirement: Configured Audiobookshelf initializes independently
After TUI entry, mbv SHALL validate a configured Audiobookshelf Service through `/api/me` independently of every other Service. It SHALL expose Connecting while validation is pending, Ready with the authenticated user after success, Needs authentication after explicit credential rejection, and Unavailable after connectivity or server failure.

#### Scenario: Persisted Audiobookshelf setup connects
- **WHEN** the TUI starts with a configured Audiobookshelf Service whose `/api/me` request succeeds
- **THEN** Audiobookshelf SHALL transition through Connecting to Ready
- **THEN** its runtime identity SHALL identify the authenticated server and user

#### Scenario: Persisted Audiobookshelf server is unavailable
- **WHEN** background validation cannot reach the configured server or the server cannot complete `/api/me`
- **THEN** Audiobookshelf SHALL become Unavailable
- **THEN** mbv SHALL preserve its server setup and API key

#### Scenario: Persisted Audiobookshelf key is rejected
- **WHEN** the configured server explicitly rejects the persisted API key
- **THEN** Audiobookshelf SHALL become Needs authentication
- **THEN** mbv SHALL preserve its server setup and Service-owned state but delete the rejected API key

#### Scenario: Stale connection result arrives
- **WHEN** a connection result belongs to an Audiobookshelf setup that has since been repaired, replaced, or removed
- **THEN** mbv SHALL ignore that result without changing the current setup, credential, identity, or Service state

### Requirement: Audiobookshelf connection can be tested from Services settings
Services settings SHALL provide a Test connection action for configured Audiobookshelf. The action SHALL call `/api/me` and present a concise result containing the configured server and authenticated user on success, while applying the same failure classification and credential-retention rules as background validation.

#### Scenario: Audiobookshelf connection test succeeds
- **WHEN** the user tests a configured Audiobookshelf Service and `/api/me` succeeds
- **THEN** mbv SHALL report the configured server and authenticated user
- **THEN** it SHALL leave the working setup and API key unchanged

#### Scenario: Audiobookshelf connection test cannot reach the server
- **WHEN** the test request fails because the configured server is unavailable
- **THEN** mbv SHALL report a connectivity failure and preserve the setup and API key

#### Scenario: Audiobookshelf connection test rejects the key
- **WHEN** the test request explicitly rejects the persisted API key
- **THEN** mbv SHALL report an authentication failure, preserve the server setup, delete the rejected key, and show Needs authentication

### Requirement: Audiobookshelf follows the singleton Service lifecycle
mbv SHALL repair, replace, and remove Audiobookshelf through the Service lifecycle established for Remote Services. Repair of the same configured server SHALL preserve Service-owned state; replacement with a different server and removal SHALL require confirmation and clear Audiobookshelf-owned setup, credentials, runtime identity, and local state as applicable.

#### Scenario: Audiobookshelf authentication is repaired
- **WHEN** a replacement API key validates against the existing configured server
- **THEN** mbv SHALL retain the Service identity and Service-owned state
- **THEN** Audiobookshelf SHALL become Ready with the newly validated credential

#### Scenario: User replaces the Audiobookshelf server
- **WHEN** the user validates and confirms a setup for a different Audiobookshelf server
- **THEN** mbv SHALL clear state belonging to the previous server before committing the replacement
- **THEN** identifiers from the previous server SHALL NOT be resolved against the replacement server

#### Scenario: User removes Audiobookshelf
- **WHEN** the user confirms Audiobookshelf Service removal
- **THEN** mbv SHALL delete its setup, API key, runtime identity, and Service-owned local state
- **THEN** Audiobookshelf SHALL become Not configured without affecting Emby or Feeds

### Requirement: Setup remains identity-only
This capability SHALL use Audiobookshelf only to validate the authenticated user. It SHALL NOT request libraries or catalog contents, create queue items, open playback sessions, connect to Socket.IO, or add Audiobookshelf media support to a Player owner.

#### Scenario: Audiobookshelf reaches Ready
- **WHEN** `/api/me` validates a configured Audiobookshelf Service
- **THEN** mbv SHALL expose the validated Service identity and connection actions
- **THEN** it SHALL NOT load or display Audiobookshelf libraries or media

