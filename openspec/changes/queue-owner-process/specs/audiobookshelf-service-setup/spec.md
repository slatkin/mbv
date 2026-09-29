## RENAMED Requirements

- FROM: `### Requirement: Bare-mode Audiobookshelf changes apply to a running same-user Local daemon`
- TO: `### Requirement: Audiobookshelf changes apply to a running same-user Local daemon`

## ADDED Requirements

### Requirement: Committed Audiobookshelf owner state is reconciled by the owner rereading its storage
Committed Audiobookshelf owner state SHALL be reconciled by signaling what changed and making the owner reread its own storage. The owner SHALL compare the persisted revision to the signaled revision, apply the committed state when they match, and reject a stale signal.

#### Scenario: Owner applies a matching revision
- **WHEN** an owner receives a reconciliation signal whose revision equals the persisted Audiobookshelf setup revision
- **THEN** the owner SHALL reread its own setup and secret and install the committed runtime state with an advanced generation

#### Scenario: Owner rejects a mismatched revision
- **WHEN** an owner receives a reconciliation signal whose revision differs from the persisted setup revision
- **THEN** the owner SHALL reject the signal and keep the installed runtime unchanged

## REMOVED Requirements

### Requirement: Committed Audiobookshelf owner state is reconciled by rereading owner storage
**Reason**: Its bare-mode direct-apply path no longer exists; every owner is a daemon reconciled by signal.
**Migration**: Replaced by "Committed Audiobookshelf owner state is reconciled by the owner rereading its storage".
