## MODIFIED Requirements

### Requirement: Running owner reconciles from persisted state
After durable commit, the administrative command SHALL request that a running packaged daemon reread its own Service storage by sending `CtrlCmd::ApplyServiceSetup { kind: ServiceKind::Emby, revision }` over the packaged daemon's local Unix ctrl socket only. `revision` SHALL be the committed persisted `EmbySetup.revision: u64`; it SHALL not be an identity hash or an in-memory `SetupGeneration`. Credentials and candidate setup values SHALL NOT appear in the request.

The daemon SHALL reread its own setup and secret, compare the persisted revision exactly to the request revision, and return either `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }`. `reason` SHALL be exactly one of `UnsupportedService`, `RevisionMismatch`, `StorageUnavailable`, or `TransitionRejected`. A packaged daemon SHALL accept `ApplyServiceSetup` only over its own local Unix ctrl socket; a same-user Local daemon SHALL accept it only from its own attached local client, and every TCP ctrl transport SHALL reject it.

#### Scenario: Running owner applies the commit
- **WHEN** a running packaged daemon acknowledges the committed Emby setup revision
- **THEN** the command SHALL report that the setup is active without requiring restart
- **THEN** unrelated active playback SHALL continue unless different-server cleanup made the active item unplayable

#### Scenario: No running owner exists
- **WHEN** setup commits while packaged `mbvd` is stopped
- **THEN** the command SHALL report success
- **THEN** the next daemon startup SHALL load the committed setup

#### Scenario: Live reconciliation is unavailable or rejected
- **WHEN** setup commits but the running owner cannot acknowledge rereading it
- **THEN** the command SHALL preserve the durable setup
- **THEN** it SHALL report clearly that restart is required and SHALL NOT claim the setup is active

#### Scenario: Reconciliation revision differs from storage
- **WHEN** the packaged daemon receives `ApplyServiceSetup` with a revision other than the stored Emby setup revision
- **THEN** it SHALL return `ServiceSetupRejected` with `RevisionMismatch`
- **THEN** it SHALL keep the installed runtime unchanged

#### Scenario: Reconciliation cannot read owner storage
- **WHEN** the packaged daemon cannot load the committed setup or secret while processing `ApplyServiceSetup`
- **THEN** it SHALL return `ServiceSetupRejected` with `StorageUnavailable`
- **THEN** the command SHALL exit `3` and report restart required

#### Scenario: Reconciliation is attempted over an ineligible transport or role
- **WHEN** a TCP client, or a client of a different owner, submits `ApplyServiceSetup`
- **THEN** the receiver SHALL reject it without rereading Service storage or changing runtime state

## ADDED Requirements

### Requirement: mbvd connect installs Audiobookshelf locally
`mbvd --connect abs` SHALL run on the packaged-daemon host under the daemon runtime identity and SHALL install or replace that owner's singleton Audiobookshelf Service. It SHALL prompt locally for the Audiobookshelf server URL and API key, validate them with `GET /api/me` using the API key as `Authorization: Bearer <api-key>`, and commit only after validation succeeds. It SHALL NOT offer a username/password flow and SHALL NOT send Service credentials through ctrl.

#### Scenario: Administrator starts Audiobookshelf setup
- **WHEN** the administrator runs `mbvd --connect abs` from an interactive terminal
- **THEN** the command SHALL prompt locally for server URL and API key
- **THEN** it SHALL NOT require interactive `mbv` to run under the packaged-daemon identity

#### Scenario: Non-interactive or conflicting invocation
- **WHEN** `mbvd --connect abs` runs without an interactive terminal, or is combined with daemon, export, or quit action selectors
- **THEN** the command SHALL exit `2` without changing persisted or active Audiobookshelf state

### Requirement: Audiobookshelf connect validates before commit
The `mbvd --connect abs` command SHALL validate the complete candidate through `GET /api/me` and commit setup and secret only after validation confirms the associated active user. A rejected or unreachable candidate SHALL preserve the previous working setup, credential, runtime identity, and Service-owned state. A validated candidate that cannot be durably committed SHALL restore or preserve the previous state.

#### Scenario: Candidate validates
- **WHEN** `GET /api/me` accepts the server URL and API key for an active user
- **THEN** the command SHALL commit the owner-local Audiobookshelf setup and Service credential
- **THEN** it SHALL NOT persist the API key anywhere other than the Audiobookshelf Service secret file

#### Scenario: Candidate is rejected or unreachable
- **WHEN** validation of a candidate fails
- **THEN** the command SHALL report a classified failure without persisting the candidate
- **THEN** an existing working setup SHALL remain unchanged

#### Scenario: Durable commit fails
- **WHEN** a validated candidate cannot be durably committed
- **THEN** the command SHALL restore or preserve the previous persisted setup and credential
- **THEN** it SHALL NOT ask a running owner to adopt the uncommitted candidate

### Requirement: Audiobookshelf connect repair and replacement use the same command
Re-running `mbvd --connect abs` SHALL be the supported Audiobookshelf setup, authentication-repair, and server-replacement path. Same-server repair SHALL preserve Audiobookshelf-owned state. A validated different-server replacement SHALL clear state owned by the previous setup before committing, while preserving non-Audiobookshelf state.

#### Scenario: Existing server receives a new credential
- **WHEN** a validated candidate identifies the installed Audiobookshelf server
- **THEN** the command SHALL replace the credential without clearing Audiobookshelf-owned state

#### Scenario: Different Audiobookshelf server validates
- **WHEN** a validated candidate identifies a different server
- **THEN** the command SHALL clear state owned by the previous Audiobookshelf Service before committing the replacement
- **THEN** non-Audiobookshelf state SHALL remain

### Requirement: mbvd disconnect abs removes Audiobookshelf without confirmation
`mbvd --disconnect abs` SHALL remove the packaged owner's Audiobookshelf setup, API key, and Audiobookshelf-owned state without a confirmation prompt. It SHALL report explicitly that the durable credential was removed. An unsupported or non-interactive invocation SHALL exit `2` without changing state.

#### Scenario: Administrator disconnects Audiobookshelf
- **WHEN** the administrator runs `mbvd --disconnect abs`
- **THEN** the command SHALL delete the setup, API key, and Audiobookshelf-owned state with no confirmation prompt
- **THEN** it SHALL report that the durable credential was removed

#### Scenario: No Audiobookshelf setup exists
- **WHEN** `mbvd --disconnect abs` runs while no Audiobookshelf setup is installed
- **THEN** the command SHALL report the absent state without erroring on missing files

### Requirement: Audiobookshelf administration reconciles a running owner
After durable Audiobookshelf commit or removal, the administrative command SHALL request that a running packaged daemon reread its own Audiobookshelf storage by sending `CtrlCmd::ApplyServiceSetup { kind: ServiceKind::Audiobookshelf, revision }` over the packaged daemon's local Unix ctrl socket only. A successful commit SHALL be preserved if live reconciliation fails, reporting restart required. If disconnect reconciliation fails, the command SHALL state that the running process may retain the deleted key in memory.

#### Scenario: Running owner applies the commit
- **WHEN** a running packaged daemon acknowledges the committed Audiobookshelf setup revision
- **THEN** the command SHALL report that the setup is active without requiring restart

#### Scenario: Live reconciliation is unavailable or rejected
- **WHEN** setup commits but the running owner cannot acknowledge rereading it
- **THEN** the command SHALL preserve the durable setup
- **THEN** it SHALL report clearly that restart is required and SHALL NOT claim the setup is active

#### Scenario: Disconnect cannot reconcile a running owner
- **WHEN** `mbvd --disconnect abs` durably removes the credential but the running owner cannot acknowledge rereading its storage
- **THEN** the command SHALL report that restart is required
- **THEN** it SHALL state that the running process may retain the deleted key in memory

#### Scenario: No running owner exists
- **WHEN** Audiobookshelf administration commits while packaged `mbvd` is stopped
- **THEN** the command SHALL report success
- **THEN** the next daemon startup SHALL load the committed state
