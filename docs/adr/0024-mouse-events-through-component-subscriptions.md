# ADR 0024: Mouse events through component subscriptions

- **Status:** Accepted
- **Date:** 2026-09-02

## Decision

Mounted interactive parents receive terminal mouse events through TuiRealm
subscriptions, using an any-position mouse clause and `SubClause::Always`.
Mounted parents own gesture recognition and hit-test their own painted geometry.
The shell folds simultaneous mouse messages with fixed priority: topmost
mounted overlay/modal, active panel, other visible panel, then chrome. A
blocking overlay swallows messages from components beneath it.

This records the mouse counterpart to [ADR 0022](0022-migrate-existing-tui-framework-to-tuirealm.md)
and [ADR 0023](0023-one-central-keyboard-router.md). It forbids a global hit map or
router and keeps coordinate resolution in the component that paints the
geometry.

## Consequences

Mouse delivery is independent of keyboard focus while preserving TuiRealm's
mount and focus model. Gesture timing is private to each mounted parent, so
future drag and hover gestures can be added without a second global clock.
Canonical list controls remain responsible for their own row geometry.
