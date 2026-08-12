# service-management Specification

## Purpose
Defines singleton Service setup, state, credential handling, migration, repair, replacement, and removal through mbv's Settings interface.
## Requirements
### Requirement: Services have singleton lifecycle
mbv SHALL model one Emby Service, one Audiobookshelf Service, and one Feeds Service. Each kind SHALL exist at most once; Feeds SHALL always be present even when it contains no subscriptions.

#### Scenario: Services settings is opened
- **WHEN** the user opens Services in Settings
- **THEN** mbv SHALL show the lifecycle and availability of supported Services
- **THEN** feed subscriptions SHALL be managed as contents of the always-present Feeds Service

#### Scenario: Remote Service is not configured
- **WHEN** a supported Remote Service has no validated setup
- **THEN** its Service state SHALL be Not configured

### Requirement: Remote Service setup is validated before commit
mbv SHALL commit a Remote Service setup only after successfully authenticating it and confirming the remote identity. A failed setup attempt SHALL NOT replace a previously working setup or leave an unverified persisted Service.

#### Scenario: New setup validates successfully
- **WHEN** the user supplies a valid server and credential through Services settings
- **THEN** mbv SHALL persist the setup and Service credential
- **THEN** the Service SHALL become Ready

#### Scenario: New setup cannot be validated
- **WHEN** the supplied server is unreachable or rejects the credential
- **THEN** mbv SHALL report the validation failure in Services settings
- **THEN** it SHALL NOT persist the attempted setup

### Requirement: Emby setup generates a Service credential
Emby Service setup SHALL collect server URL, username, and password, authenticate those credentials with Emby, and persist only the returned token and required Emby identity metadata. The username and password SHALL remain transient and SHALL NOT be written to configuration or credential storage.

#### Scenario: Emby credentials are accepted
- **WHEN** the user submits a valid Emby server, username, and password
- **THEN** mbv SHALL generate and persist the returned Emby token
- **THEN** the password SHALL not be retained after setup completes

#### Scenario: Emby credentials are rejected
- **WHEN** Emby rejects the submitted username or password
- **THEN** no new Emby Service credential SHALL be persisted
- **THEN** the setup form SHALL remain available for correction

### Requirement: Service credentials are isolated local secrets
Each Remote Service credential SHALL be stored separately from general configuration and separately from other Service credentials in a file restricted to the current user. Service credentials SHALL NOT require or be stored in optional shared-state infrastructure.

#### Scenario: Emby token is persisted
- **WHEN** Emby setup completes successfully
- **THEN** its token SHALL be written to the Emby Service's secret file with mode `0600`
- **THEN** it SHALL NOT be written to `config.toml`

#### Scenario: One Service credential is removed
- **WHEN** one Remote Service credential is cleared or removed
- **THEN** credentials belonging to every other Service SHALL remain unchanged

### Requirement: Existing Emby setup migrates automatically
On first use of the new Service model, mbv SHALL migrate the existing Emby server URL, token, and user ID into the Emby Service setup without prompting for credentials. The legacy credential file SHALL be removed only after the new secret file is durably written.

#### Scenario: Valid legacy setup exists
- **WHEN** mbv finds an existing Emby server configuration and `token.json`
- **THEN** it SHALL create the equivalent configured Emby Service
- **THEN** the user SHALL not be asked to authenticate again solely because of migration

#### Scenario: Migration write fails
- **WHEN** mbv cannot durably write the new Emby secret file
- **THEN** it SHALL retain the legacy credential file intact
- **THEN** it SHALL report the migration failure without losing the credential

### Requirement: Runtime failures preserve the correct setup state
Connectivity failures SHALL place a configured Remote Service in Unavailable while preserving its setup and Service credential. Explicit credential rejection SHALL place it in Needs authentication, preserve its server setup and Service-owned state, and clear the rejected secret.

#### Scenario: Configured server is temporarily unreachable
- **WHEN** a Ready or Connecting Remote Service encounters a connectivity failure
- **THEN** it SHALL become Unavailable
- **THEN** its server setup and Service credential SHALL remain persisted

#### Scenario: Configured server rejects its credential
- **WHEN** a Remote Service receives an authentication rejection for its persisted credential
- **THEN** it SHALL become Needs authentication
- **THEN** mbv SHALL delete the rejected secret but preserve the server setup and Service-owned state

#### Scenario: Authentication is repaired
- **WHEN** the user successfully supplies a replacement credential for a Service in Needs authentication
- **THEN** the Service SHALL retain its existing identity and Service-owned state
- **THEN** it SHALL become Ready

### Requirement: Service replacement and removal clear owned state
Changing a Remote Service to a different server SHALL be a confirmed Service replacement. Replacing or removing a Service SHALL delete its credential and clear its queued and persisted item state, library positions, routes, and caches; removal SHALL return it to Not configured.

#### Scenario: User replaces the Emby server
- **WHEN** the user confirms setup of a different Emby server
- **THEN** mbv SHALL clear local state belonging to the previous Emby server
- **THEN** native IDs from the previous server SHALL NOT be resolved against the replacement server

#### Scenario: User removes Emby
- **WHEN** the user confirms Emby Service removal
- **THEN** mbv SHALL delete the Emby setup and credential
- **THEN** it SHALL clear Emby-owned local state and show Emby as Not configured
- **THEN** the TUI and Feeds Service SHALL remain usable

