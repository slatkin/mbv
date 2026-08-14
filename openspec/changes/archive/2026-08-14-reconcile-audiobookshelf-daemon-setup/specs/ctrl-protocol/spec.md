## ADDED Requirements

### Requirement: Same-user Local daemon reconciles owner service setup on client signal
The v9 ctrl protocol SHALL let an attached same-user client signal a Local daemon to reread its own owner-local Service setup by sending `CtrlCmd::ApplyServiceSetup { kind, revision }` over the Local daemon's own control socket. The Local daemon SHALL respond with `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }`. The request and response SHALL carry no setup values, identity hash, or Service credential.

#### Scenario: Local daemon applies a client's setup signal
- **WHEN** an attached same-user client sends `ApplyServiceSetup` to its Local daemon for a Service revision matching the daemon's persisted setup
- **THEN** the Local daemon SHALL reread its own storage and return `ServiceSetupApplied`

#### Scenario: Local daemon rejects a mismatched or unsupported signal
- **WHEN** an attached same-user client sends `ApplyServiceSetup` for a revision other than the persisted one, or for a Service the Local daemon does not hold owner context for
- **THEN** the Local daemon SHALL return `ServiceSetupRejected` with `RevisionMismatch` or `UnsupportedService`
- **THEN** it SHALL keep its runtime unchanged

#### Scenario: Credentials never cross the signal
- **WHEN** a client signals a Local daemon to reread owner Service setup
- **THEN** no Service credential, Authorization value, or resolved setup value SHALL appear in the request or response

## MODIFIED Requirements

### Requirement: Packaged owner-service reconciliation is local-only
The v9 ctrl protocol SHALL carry `CtrlCmd::ApplyServiceSetup { kind, revision }` and the matching `CtrlEvent::ServiceSetupApplied { kind, revision }` or `CtrlEvent::ServiceSetupRejected { kind, revision, reason }` for `ServiceKind::Emby` and `ServiceKind::Audiobookshelf`. It SHALL carry no setup values, identity hash, or Service credential. `reason` SHALL be one of `UnsupportedService`, `RevisionMismatch`, `StorageUnavailable`, or `TransitionRejected`.

#### Scenario: Packaged Unix ctrl applies a persisted setup
- **WHEN** a packaged-daemon local Unix client sends `ApplyServiceSetup` for a committed Emby or Audiobookshelf Service revision
- **THEN** the daemon SHALL return an applied or explicitly rejected response for that request

#### Scenario: TCP client sends owner administration to a packaged daemon
- **WHEN** a TCP client sends `ApplyServiceSetup` to a packaged daemon
- **THEN** the daemon SHALL reject the request without changing Service runtime state

#### Scenario: Cross-owner client sends owner administration
- **WHEN** a client attached to one owner sends `ApplyServiceSetup` to a different owner process it is not the local client of
- **THEN** the receiving owner SHALL reject the request without rereading Service storage or changing runtime state
