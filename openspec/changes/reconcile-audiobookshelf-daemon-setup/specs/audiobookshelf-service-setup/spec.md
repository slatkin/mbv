## ADDED Requirements

### Requirement: Audiobookshelf setup carries a persisted revision
Every committed owner-local Audiobookshelf setup SHALL contain a persisted unsigned 64-bit `revision`, initially `1` and incremented exactly once for every successful initial setup, same-server repair, or different-server replacement. The persisted revision SHALL identify the commit to another process. It SHALL be distinct from the in-memory setup generation, which advances for each runtime install or replacement so stale asynchronous Audiobookshelf work cannot affect the current runtime.

#### Scenario: Initial setup commits
- **WHEN** a validated Audiobookshelf candidate commits for the first time
- **THEN** the persisted setup SHALL carry revision `1`

#### Scenario: Same-server repair commits
- **WHEN** a validated candidate for the already installed server commits
- **THEN** the persisted setup SHALL carry the next revision with the replacement credential
- **THEN** Audiobookshelf-owned state SHALL be preserved

#### Scenario: Different-server replacement commits
- **WHEN** a validated candidate for a different server commits
- **THEN** the persisted setup SHALL carry the next revision
- **THEN** state owned by the previous setup SHALL be cleared before the replacement is usable

### Requirement: Daemon owners load Audiobookshelf owner context from their own storage
A Local daemon and packaged `mbvd` SHALL load their owner-scoped Audiobookshelf setup, API key, setup generation, and stable device identity from their own storage without transporting credentials through ctrl. Constructing the owner context SHALL NOT authenticate; a daemon SHALL start and remain Service-independent even when the configured server is unavailable or the setup is absent.

#### Scenario: Owner has a configured Audiobookshelf setup
- **WHEN** a Local daemon or packaged `mbvd` starts with a persisted Audiobookshelf setup and credential
- **THEN** it SHALL construct an Audiobookshelf owner context holding setup, API key, generation, and stable device identity
- **THEN** it SHALL NOT authenticate or enable Audiobookshelf playback

#### Scenario: Owner has no Audiobookshelf setup
- **WHEN** a daemon owner starts without an Audiobookshelf setup
- **THEN** it SHALL hold no Audiobookshelf owner context
- **THEN** every unrelated Service and core daemon behavior SHALL remain available

#### Scenario: Credentials stay out of ctrl
- **WHEN** a daemon owner loads or reconciles Audiobookshelf owner context
- **THEN** the API key and any Authorization value SHALL NOT appear in ctrl messages, queue state, or logs

#### Scenario: Device identity is stable
- **WHEN** an owner constructs Audiobookshelf owner context
- **THEN** it SHALL load the same stable, non-secret device identifier used by every Audiobookshelf playback session request

### Requirement: Committed Audiobookshelf owner state is reconciled by rereading owner storage
Committed Audiobookshelf owner state SHALL be reconciled by signaling what changed and making the owner reread its own storage. The owner SHALL compare the persisted revision to the signaled revision, apply the committed state when they match, and reject a stale signal. Bare mode SHALL invoke the same semantic operation directly, without a ctrl round trip.

#### Scenario: Owner applies a matching revision
- **WHEN** an owner receives a reconciliation signal whose revision equals the persisted Audiobookshelf setup revision
- **THEN** the owner SHALL reread its own setup and secret and install the committed runtime state with an advanced generation

#### Scenario: Owner rejects a mismatched revision
- **WHEN** an owner receives a reconciliation signal whose revision differs from the persisted setup revision
- **THEN** the owner SHALL reject the signal and keep the installed runtime unchanged

#### Scenario: Bare mode applies directly
- **WHEN** bare-mode mbv commits an Audiobookshelf setup, repair, replacement, or removal
- **THEN** the in-process owner SHALL apply the committed state directly without signaling another process

### Requirement: Bare-mode Audiobookshelf changes apply to a running same-user Local daemon
After mbv commits an Audiobookshelf setup, repair, replacement, or removal through Services, a running same-user Local daemon SHALL adopt the committed state when possible by rereading its own storage. The durable commit SHALL be preserved whether or not a running Local daemon acknowledges it.

#### Scenario: Running Local daemon adopts the commit
- **WHEN** mbv commits an Audiobookshelf change while a same-user Local daemon is running
- **THEN** the Local daemon SHALL reread its own Audiobookshelf storage and install the committed state with an advanced generation

#### Scenario: No Local daemon is running
- **WHEN** mbv commits an Audiobookshelf change while no same-user Local daemon is running
- **THEN** the commit SHALL succeed
- **THEN** the next Local daemon startup SHALL load the committed state

#### Scenario: Live reconciliation is unavailable
- **WHEN** mbv commits an Audiobookshelf change but the running Local daemon cannot acknowledge rereading it
- **THEN** mbv SHALL preserve the durable commit
- **THEN** mbv SHALL report clearly that a daemon restart is required and SHALL NOT claim the change is active in the daemon

### Requirement: Audiobookshelf replacement and removal clean owner state without daemon playback
A different-server Audiobookshelf replacement and an Audiobookshelf removal SHALL advance the owner generation and clear Audiobookshelf-owned state for that owner while leaving unrelated persisted media intact. These operations SHALL NOT make a daemon owner eligible to bind or play Audiobookshelf podcast episodes.

#### Scenario: Different-server replacement clears previous Audiobookshelf state
- **WHEN** a validated different-server Audiobookshelf replacement commits
- **THEN** state owned by the previous setup SHALL be cleared before the replacement is active
- **THEN** identifiers from the previous server SHALL NOT be resolved against the replacement server

#### Scenario: Removal clears setup, credential, and owned state
- **WHEN** Audiobookshelf is removed for an owner
- **THEN** the setup, API key, and Audiobookshelf-owned state SHALL be deleted
- **THEN** Emby, Feeds, and unrelated persisted media SHALL remain

#### Scenario: Daemon playback stays disabled
- **WHEN** a daemon owner loads or reconciles Audiobookshelf owner context
- **THEN** the owner SHALL continue treating Audiobookshelf podcast episodes as unplayable
- **THEN** no Audiobookshelf item SHALL enter a daemon Bound queue or start playback
