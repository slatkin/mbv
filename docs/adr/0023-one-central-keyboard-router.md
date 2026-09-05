---
status: accepted
---

# One Central Keyboard Router

mbv has exactly one keyboard routing authority. It lives in the `UiRoot`
Interactive Component. No other component, subscription, or shell method
resolves a chord that is not its own local interaction.

## Problem

ADR 0002 made keyboard precedence explicit as an ordered, first-match
`CONTEXT_STACK` returning `Command` / `Swallow` / `FallThrough`. ADR 0022
replaced the surrounding framework with TuiRealm. TuiRealm's dispatch has no
representation for that model:

```rust
// tuirealm 4.1, core/application.rs — Application::tick
let mut messages = events.iter()
    .filter_map(|x| self.forward_to_active_component(x))   // focused component
    .collect();
if !self.sub_lock {
    self.forward_to_subscriptions(&events, &mut messages); // AND every satisfied sub
}
```

Delivery is unconditional fan-out. There is no consumed signal, no priority
between subscriptions, and `sub_lock` is all-or-nothing. `Swallow` and
`FallThrough` — the two outcomes that carry precedence — cannot be expressed by
a `SubClause`.

The migration's working assumption was that the global bindings would become
gated subscriptions on their nearest owners. That does not reproduce
first-match: it requires every gate to encode the negation of every
higher-priority claimant's condition, and `SubClause` can only read
`mounted()`, `state()`, and `query(Attribute)`. Every runtime condition behind a
precedence layer would have to be mirrored onto a component as an attribute —
rebuilding the `App`-wide input snapshot as distributed mirror state, which is
what the migration exists to delete.

The observable result was a live keyboard endpoint back through `App` that no
teardown task could remove, because nothing had replaced the ordering relation
it implemented.

## Decision

**Keyboard precedence is a function, not an emergent property of delivery.**

`UiRoot` is the **Keyboard Router**. It is permanently mounted and already
subscribes to every event (`EventClause::Any`, `SubClause::Always`), so it
observes every chord regardless of focus. It resolves each chord against an
ordered policy and returns ADR 0002's three outcomes unchanged.

A **leaf** — the focused Interactive Component — interprets only chords that
mean something inside its own surface, and emits a typed semantic request. It
never interprets a global chord, and it never forwards an unclaimed chord
anywhere.

`Application::tick` returns messages in a defined order: the focused
component's message first, then subscribers'. With `PollStrategy::Once` there is
at most one terminal event per tick, so both the leaf's request and the
router's resolution for the same chord are available together, and the router's
outcome selects between them:

```
key ──┬─▶ focused leaf ─────────▶ Option<Msg>   (local meaning, or None)
      │
      └─▶ UiRoot router ────────▶ Command  → run it, discard the leaf's Msg
                                  Swallow  → run nothing, discard the leaf's Msg
                                  FallThrough → the leaf's Msg stands
```

This is ADR 0002's semantics exactly, with `CONTEXT_STACK`'s handler bodies
replaced by the leaf's own local interpretation. A blocking overlay is
`Swallow`. A global chord is `Command`. Anything the policy declines is
`FallThrough`, which is how the leaf gets its key.

Fall-through that depends on router state needs no special case. The Space and
Escape double-tap returns `FallThrough` on the first press — so browse
`go_back` or Audiobookshelf play happens, from the leaf, as it does today — and
`Command(Stop)` / `Command(TogglePlayPause)` on the second press within 300 ms,
discarding the leaf's request.

`UiRoot` is skipped by `forward_to_subscriptions` while it holds focus, and
receives the event as the active component instead. Delivery is therefore
exactly once whether or not a leaf is focused.

## Rules

- Exactly one router. Adding a second keyboard resolution site — a shell
  pre-router, a gated keyboard subscription on another component, a per-screen
  copy of a global binding — is a violation regardless of where the code lives.
- A leaf that does not recognize a chord returns `None`. It does not forward,
  wrap, or re-emit the key.
- No Interactive Component accepts or emits a raw `crossterm::event::KeyEvent`.
  Cross-boundary requests are semantic.
- Router policy reads a plain-data snapshot, as ADR 0002 specified. It does not
  read component attributes to reconstruct precedence.
- Keyboard subscriptions carry no precedence. `UiRoot`'s universal observer
  subscription is the only keyboard subscription.

## Considered Options

- **Gated subscriptions per owner** (the migration's original assumption):
  rejected. It cannot express `Swallow`/`FallThrough`, and reconstructing
  precedence as mutually exclusive `SubClause` conjunctions requires a
  distributed attribute mirror of shell state.
- **`Component::perform(Cmd)` as the policy execution path** (design D15,
  declined at task 5.4): rejected here as orthogonal. `Cmd` carries no
  modifiers and no payload, so the policy would resolve modifier-sensitive
  chords shell-side anyway; the routing problem is unchanged by it. Left
  available as a later, separate input-API decision.
- **A shell pre-router in `Model`**: rejected. Same ordering relation, but sited
  outside the component framework, which is the shape ADR 0022 removes.
- **Per-leaf mirrors of global behavior** (e.g. each screen implementing the
  double-tap): rejected. Per-screen resolution of a global binding is the
  decentralization ADR 0002 exists to prevent.

## Consequences

- `CONTEXT_STACK`, `Model::handle_legacy_key`, `App::handle_key_with_home_context`,
  `GlobalViewKey`, the raw `*Key` shell requests, and the TuiRealm-to-Crossterm
  reconstruction adapter are all removable, because the ordering relation they
  stood in for now exists elsewhere.
- `key_policy.rs` stops being a descriptive shadow table with
  `#![allow(dead_code)]` and becomes the router's live policy, asserted through
  the router rather than against `CONTEXT_STACK`.
- Precedence remains readable and assertable as data, satisfying ADR 0002's
  guardrail, and remains testable through `Application::tick()` rather than only
  through direct handler calls.
- ADR 0022's completion contract is unchanged; this ADR supplies the input
  mapping its Consequences section requires.
- Deferred user-configurable keybindings (ADR 0002) stay a data-loading phase
  over one table, not a re-architecture.
