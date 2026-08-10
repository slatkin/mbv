## MODIFIED Requirements

### Requirement: Shared-data hosting and use are explicit opt-ins

The system SHALL host shared data only when the canonical daemon has shared-data hosting enabled. A client SHALL use shared data only when configured with an explicit shared-data endpoint. The shared-data endpoint SHALL be independent of the client's playback endpoint and library routes. When hosting creates a shared-data database, it SHALL use file format v3, which is readable by the current redb 2.6 runtime and a future redb 4.x runtime.

#### Scenario: Hosting is disabled

- **WHEN** the daemon starts without shared-data hosting enabled
- **THEN** it SHALL open no shared-data listener or database
- **THEN** ordinary daemon playback behavior SHALL remain unchanged

#### Scenario: Hosting creates a database

- **WHEN** shared-data hosting is enabled and no shared-data database exists
- **THEN** the daemon SHALL create the database in file format v3
- **THEN** subsequent restarts SHALL open it without format migration

#### Scenario: Client use is disabled

- **WHEN** a client has no shared-data endpoint configured
- **THEN** it SHALL use the existing local persistence behavior

#### Scenario: Playback route changes

- **WHEN** a participating client changes its playback route
- **THEN** its shared-data endpoint and authenticated shared-data session SHALL remain unchanged

### Requirement: Storage failure is isolated from playback

The daemon SHALL acknowledge a write only after the database commits it. Before hosting shared data, the redb 2.6 runtime SHALL safely migrate a supported file-format-v2 database to file format v3. Migration SHALL be idempotent and SHALL preserve every record in every known application-owned table, including application-level revisions. Database open, migration, corruption, serialization, disk-full, and commit failures SHALL fail shared-data hosting or the affected operation without stopping daemon playback or damaging the original database or a previously committed value.

#### Scenario: Supported legacy database is present

- **WHEN** shared-data hosting starts with a database in a supported legacy format
- **THEN** the daemon SHALL migrate it to the current format before opening the shared-data listener
- **THEN** every record in every known application-owned table SHALL remain unchanged after migration and restart

#### Scenario: Already migrated database is present

- **WHEN** shared-data hosting restarts with a database already in the current format
- **THEN** the daemon SHALL open it without repeating or changing the migration

#### Scenario: Migration fails

- **WHEN** a supported legacy database cannot be migrated and validated safely
- **THEN** the original database SHALL remain available for a later migration or recovery attempt
- **THEN** shared-data hosting SHALL remain unavailable for that run
- **THEN** daemon playback SHALL continue

#### Scenario: Database cannot open

- **WHEN** shared-data hosting is enabled but its database cannot be opened safely and is not a supported legacy format
- **THEN** shared-data hosting SHALL remain unavailable
- **THEN** daemon playback SHALL continue

#### Scenario: Commit fails

- **WHEN** durable commit of an update fails
- **THEN** the service SHALL not acknowledge or broadcast the proposed revision
- **THEN** the previously committed document SHALL remain authoritative
