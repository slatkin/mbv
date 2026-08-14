# packaged-daemon-service-runtime Specification

## Purpose
Defines packaged `mbvd` as a Service-independent Player owner whose core queue, control, and Feed behavior does not depend on any Remote Service.

## Requirements

### Requirement: Packaged daemon starts without a Remote Service
Packaged `mbvd` SHALL start with zero configured Remote Services and SHALL make its Player owner, Bound queue, ctrl listeners, and Feed playback available without authenticating Emby or Audiobookshelf.

#### Scenario: No Remote Service is configured
- **WHEN** packaged `mbvd` starts without Emby or Audiobookshelf setup
- **THEN** it SHALL enter its ordinary running state
- **THEN** ctrl clients SHALL be able to inspect and control its representable queue
- **THEN** Feed playback SHALL remain available

#### Scenario: Configured Emby is unreachable
- **WHEN** packaged `mbvd` starts with persisted Emby setup whose server is unavailable
- **THEN** its core Player-owner, ctrl, and Feed behavior SHALL remain available
- **THEN** the persisted Emby setup and credential SHALL remain intact

### Requirement: Emby behavior is owned by an optional runtime
Packaged `mbvd` SHALL start Emby API lookup, Emby item source preparation and reporting, Emby WebSocket handling, Emby remote commands, and Emby capability registration only while it has usable owner-local Emby setup. Absence or loss of that runtime SHALL NOT disable unrelated Services or core daemon control.

#### Scenario: Emby setup is usable
- **WHEN** packaged `mbvd` loads a valid Emby setup and credential
- **THEN** Emby-owned lookup, playback lifecycle, WebSocket, remote-command, and capability behavior SHALL become available

#### Scenario: Emby is not configured
- **WHEN** packaged `mbvd` has no Emby setup
- **THEN** it SHALL NOT start Emby WebSocket activity or advertise Emby-owned remote behavior
- **THEN** an Emby QueueItem SHALL be unplayable for that owner and SHALL NOT enter its Bound queue

#### Scenario: Emby runtime fails after startup
- **WHEN** a runtime Emby request fails because the server is unavailable
- **THEN** the failed Emby operation SHALL be reported without stopping unrelated playback or ctrl
- **THEN** the owner SHALL retain its persisted Emby setup for a later explicit operation or runtime retry

### Requirement: Durable setup revisions and runtime generations have separate roles
Every committed packaged-owner Emby setup SHALL contain a persisted unsigned 64-bit `revision`, initially `1` and incremented exactly once for every successful initial setup, same-server repair, or different-server replacement. The persisted revision SHALL identify the commit to another process. It SHALL be distinct from `service_runtime::SetupGeneration`, which remains in-memory and SHALL advance for each runtime install or replacement so stale asynchronous Emby work cannot affect the current runtime.

#### Scenario: Same-server repair commits
- **WHEN** `mbvd --connect emby` validates a candidate for the installed server
- **THEN** it SHALL persist the next Emby setup revision with the replacement credential
- **THEN** it SHALL preserve Emby-owned state

#### Scenario: Runtime install replaces an earlier runtime
- **WHEN** a packaged daemon installs a newer persisted Emby setup revision
- **THEN** it SHALL create a newer in-memory `SetupGeneration`
- **THEN** completions from an earlier generation SHALL NOT modify the new runtime

### Requirement: Service credentials stay inside their runtime
Packaged `mbvd` SHALL use an Emby credential only for Emby-owned API and playback behavior. It SHALL NOT use an Emby or Audiobookshelf credential to authorize packaged ctrl clients, and SHALL NOT expose a Service credential in queue state, ctrl messages, or logs.

#### Scenario: Client attaches to packaged ctrl
- **WHEN** an `mbv` client attaches over an allowed packaged-daemon ctrl transport
- **THEN** neither peer SHALL send an Emby or Audiobookshelf Service credential as part of ctrl admission

### Requirement: Zero-Service ctrl capability advertisement is service-neutral
Packaged `mbvd` SHALL advertise its supported protocol and ordinary queue/playback capabilities independently of configured Remote Services. A zero-Service packaged daemon SHALL omit Local-only `control-auth`; it SHALL NOT advertise a Service credential, Emby runtime availability, or Emby remote-command availability as a ctrl capability. A v9 client SHALL attach without a Service credential and SHALL treat absent optional Emby behavior as ordinary owner-side unavailability rather than a handshake failure.

#### Scenario: Zero-Service packaged daemon greets a client
- **WHEN** packaged `mbvd` starts with no usable Emby setup
- **THEN** its hello SHALL retain its service-neutral supported ctrl capabilities
- **THEN** its hello SHALL omit `control-auth` and every Service credential field

### Requirement: Shared data remains an independent optional facility
This change SHALL NOT alter shared-data enablement, Emby-scoped identity, authentication, storage, or fallback behavior. Shared-data absence or inability to authenticate SHALL NOT prevent packaged-daemon startup, ctrl, queue control, or playback.

#### Scenario: Emby-independent daemon cannot host usable shared data
- **WHEN** packaged `mbvd` runs without the Emby identity currently required by optional shared data
- **THEN** core daemon behavior SHALL continue without redesigning or substituting shared-data identity

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
