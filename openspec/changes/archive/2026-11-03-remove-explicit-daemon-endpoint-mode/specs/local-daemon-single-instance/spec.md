# Spec Delta

## REMOVED Requirements

### Requirement: Clients take no lock

**Reason**: the "Explicit-endpoint client starts" scenario describes the removed explicit
daemon-endpoint mode; the requirement is re-added without it.

## ADDED Requirements

### Requirement: A client never holds the lock

A client SHALL NOT acquire the lock. Only the Player owner SHALL hold it.

#### Scenario: Several clients run at once

- **WHEN** several clients of the same local daemon are running
- **THEN** exactly one process — the daemon — SHALL hold the lock
