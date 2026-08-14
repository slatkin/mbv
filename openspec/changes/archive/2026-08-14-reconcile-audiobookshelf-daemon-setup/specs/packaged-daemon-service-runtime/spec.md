## ADDED Requirements

### Requirement: Audiobookshelf behavior is owned by an optional runtime
Packaged `mbvd` SHALL load owner-local Audiobookshelf setup, credential, generation, and stable device identity only while it has usable owner-local Audiobookshelf setup. Absence or loss of that context SHALL NOT disable unrelated Services or core daemon control, and SHALL NOT enable Audiobookshelf playback. This change SHALL NOT start Audiobookshelf lookup, source preparation, or playback lifecycle from the packaged owner.

#### Scenario: Audiobookshelf setup is usable
- **WHEN** packaged `mbvd` loads a valid Audiobookshelf setup and credential
- **THEN** the owner SHALL hold Audiobookshelf owner context
- **THEN** no Audiobookshelf item SHALL become playable or enter the owner's Bound queue

#### Scenario: Audiobookshelf is not configured
- **WHEN** packaged `mbvd` has no Audiobookshelf setup
- **THEN** it SHALL hold no Audiobookshelf owner context
- **THEN** core queue, control, and Feed behavior SHALL remain available

#### Scenario: Audiobookshelf runtime request fails after startup
- **WHEN** a runtime Audiobookshelf validation or reread fails because the server is unavailable
- **THEN** the failure SHALL be reported without stopping unrelated playback or ctrl
- **THEN** the owner SHALL retain its persisted Audiobookshelf setup for a later explicit operation

### Requirement: Packaged daemon applies Audiobookshelf setup reconciliation
The packaged daemon SHALL apply `ApplyServiceSetup` for `ServiceKind::Audiobookshelf` by rereading its own Audiobookshelf setup and secret, comparing the persisted revision exactly to the request revision, and returning `ServiceSetupApplied` or `ServiceSetupRejected` with a reason. Applied or removed setup SHALL advance the in-memory setup generation. Audiobookshelf owner admission and playback SHALL remain disabled regardless of reconciliation.

#### Scenario: Reconciliation applies a matching revision
- **WHEN** the packaged daemon receives `ApplyServiceSetup` for Audiobookshelf with the stored revision
- **THEN** it SHALL reread its own storage and install the committed owner context with an advanced generation
- **THEN** it SHALL return `ServiceSetupApplied`

#### Scenario: Reconciliation sees no stored setup
- **WHEN** the packaged daemon receives `ApplyServiceSetup` for Audiobookshelf and its storage holds no Audiobookshelf setup
- **THEN** it SHALL drop its Audiobookshelf owner context, advance the generation, and return `ServiceSetupApplied`

#### Scenario: Reconciliation revision differs from storage
- **WHEN** the packaged daemon receives `ApplyServiceSetup` with a revision other than the stored Audiobookshelf revision
- **THEN** it SHALL return `ServiceSetupRejected` with `RevisionMismatch`
- **THEN** it SHALL keep the installed runtime unchanged

#### Scenario: Reconciliation cannot read owner storage
- **WHEN** the packaged daemon cannot load the committed Audiobookshelf setup or secret
- **THEN** it SHALL return `ServiceSetupRejected` with `StorageUnavailable`

#### Scenario: Playback stays disabled after reconciliation
- **WHEN** a packaged daemon reconciles a valid Audiobookshelf setup
- **THEN** Audiobookshelf podcast episodes SHALL remain ineligible for its Bound queue and playback
