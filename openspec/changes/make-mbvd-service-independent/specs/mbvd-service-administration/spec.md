## Purpose

Defines owner-local administration for installing or replacing singleton Remote Services used by packaged `mbvd`, beginning with Emby setup.

## ADDED Requirements

### Requirement: mbvd connect installs a Service locally
`mbvd --connect <service>` SHALL run on the packaged-daemon host under the daemon runtime identity and SHALL install or replace that owner's singleton Remote Service. It SHALL NOT act as an `mbv` client login or send Service credentials through ctrl.

#### Scenario: Administrator starts Emby setup
- **WHEN** the administrator runs `mbvd --connect emby`
- **THEN** the command SHALL prompt locally for Emby server URL, username, and password
- **THEN** it SHALL NOT require interactive `mbv` to run under the packaged-daemon identity

#### Scenario: Unsupported Service is requested
- **WHEN** `mbvd --connect` receives a Service name not implemented by that binary
- **THEN** it SHALL exit without changing any persisted or active Service state
- **THEN** it SHALL identify the supported Service names

### Requirement: mbvd connect is an interactive exclusive action
The supported invocation SHALL be exactly `mbvd --connect emby`. It SHALL require an interactive terminal for local prompts and SHALL reject unknown, missing, or additional action selectors rather than combining connection administration with daemon serving, export, or quit behavior. It SHALL not read credentials from environment variables, command-line arguments, or files in this change.

The command SHALL exit `0` for a committed setup that is active live or will load on next startup, `1` for validation, persistence, cleanup, or local-lock failure, `2` for usage, unsupported Service, or non-interactive-terminal failure, and `3` when commit succeeded but live reconciliation requires restart.

#### Scenario: Standard interactive invocation
- **WHEN** an administrator runs exactly `mbvd --connect emby` from an interactive terminal
- **THEN** the command SHALL prompt locally for candidate values before validating them

#### Scenario: Non-interactive invocation
- **WHEN** standard input or output is not an interactive terminal
- **THEN** the command SHALL exit `2` without changing persisted or active Service state

#### Scenario: Conflicting action selectors
- **WHEN** `--connect emby` is combined with daemon, export, or quit action selectors
- **THEN** the command SHALL exit `2` without changing persisted or active Service state

### Requirement: Connect diagnostics do not disclose Service credentials
The command SHALL never print, log, serialize, or retain a candidate password or Emby token. It SHALL not include raw Remote Service response bodies in diagnostics. A normalized server URL MAY identify the target in a success or classified failure diagnostic; entered username and returned user ID SHALL not be emitted.

#### Scenario: Authentication fails with a verbose remote response
- **WHEN** Emby returns an authentication failure containing request or credential material
- **THEN** the command SHALL emit only a classified authentication failure
- **THEN** no password, token, username, user ID, or raw response body SHALL be emitted

### Requirement: Candidate setup validates before commit
The Emby connect command SHALL authenticate the complete candidate against Emby, obtain and validate the resulting token and remote identity, and only then commit setup. The entered username and password SHALL remain transient; only required setup identity and the resulting long-lived Emby token SHALL be persisted.

#### Scenario: Candidate validates
- **WHEN** Emby accepts the server URL, username, and password and returns a usable token and identity
- **THEN** the command SHALL atomically commit the owner-local Emby setup and Service credential
- **THEN** it SHALL NOT persist the entered password

#### Scenario: Candidate is rejected or unreachable
- **WHEN** candidate authentication cannot complete successfully
- **THEN** the command SHALL report the failure
- **THEN** it SHALL preserve the previous persisted and active Emby setup unchanged

#### Scenario: Durable commit fails
- **WHEN** a validated candidate cannot be durably committed
- **THEN** the command SHALL restore or preserve the previous persisted setup and credential
- **THEN** it SHALL NOT ask a running owner to adopt the uncommitted candidate

### Requirement: Repair and replacement use the same command
Re-running `mbvd --connect emby` SHALL be the supported setup, authentication-repair, and server-replacement path. Same-server repair SHALL mean equality of normalized `EmbySetup.server_url`, using `EmbySetup::new` normalization (trim whitespace and one trailing slash); user ID changes alone SHALL NOT select replacement. Same-server repair SHALL preserve Emby-owned state. A validated different-server replacement SHALL invalidate previous-server Emby items and persisted state before the running owner may use the replacement, while preserving non-Emby state.

#### Scenario: Existing server receives a new credential
- **WHEN** a validated candidate identifies the currently installed Emby server
- **THEN** the command SHALL replace the Emby credential without clearing Emby-owned queue or persisted item state

#### Scenario: Different Emby server validates
- **WHEN** a validated candidate identifies a different Emby server
- **THEN** the command SHALL clear state owned by the previous Emby Service without clearing Feed or other Service state
- **THEN** IDs from the previous server SHALL NOT be resolved or reported against the replacement server

#### Scenario: Different-server cleanup fails
- **WHEN** previous-server Emby state cannot be cleared safely
- **THEN** the replacement SHALL fail without committing or activating a mixed old-state/new-server result

### Requirement: Running owner reconciles from persisted state
After durable commit, the administrative command SHALL request that a running packaged daemon reread its own Service storage by sending `CtrlCmd::ApplyServiceSetup { kind: ServiceKind::Emby, revision }` over the packaged daemon's local Unix ctrl socket only. `revision` SHALL be the committed persisted `EmbySetup.revision: u64`; it SHALL not be an identity hash or an in-memory `SetupGeneration`. Credentials and candidate setup values SHALL NOT appear in the request.

The daemon SHALL reread its own setup and secret, compare the persisted revision exactly to the request revision, and return either `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }`. `reason` SHALL be exactly one of `UnsupportedService`, `RevisionMismatch`, `StorageUnavailable`, or `TransitionRejected`. Local-daemon ctrl and every TCP ctrl transport SHALL reject `ApplyServiceSetup`.

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
- **WHEN** a TCP client or Local-daemon client submits `ApplyServiceSetup`
- **THEN** the receiver SHALL reject it without rereading Service storage or changing runtime state

### Requirement: Different-server replacement clears owner state before activation
For a different-server Emby replacement, the connect command under its owner-local administration lock SHALL snapshot and invoke `clear_emby_owned_state` before committing the new setup, token, and revision. The seam SHALL clear only the previous Emby Service's Bound and persisted queue items, library positions, routes, and caches. If clearing or setup/secret/revision persistence fails, it SHALL restore the complete snapshot and SHALL not request live adoption.

On acknowledged live adoption, the packaged daemon SHALL remove all old-server Emby items from its in-memory Bound queue before installing the replacement runtime. If the active item is an old-server Emby item, it SHALL stop the Player run and complete terminal lifecycle reporting within `EMBY_REPLACEMENT_FINALIZE_HARD_BOUND` of five seconds. It SHALL install the replacement runtime only after that finalization succeeds. If finalization fails or times out, it SHALL return `TransitionRejected`, leave the durable commit intact, and require restart. Non-Emby active playback SHALL continue.

#### Scenario: Active old-server Emby item is replaced
- **WHEN** a different-server Emby replacement commits while its old-server Emby item is active
- **THEN** live adoption SHALL stop and finalize that run before purging old-server Emby items
- **THEN** it SHALL install the replacement runtime only after finalization and purge succeed

#### Scenario: Active Feed item survives an Emby replacement
- **WHEN** a different-server Emby replacement commits while a Feed item is active
- **THEN** live adoption SHALL preserve the active Feed run
- **THEN** it SHALL purge only old-server Emby queue items before installing the replacement runtime
