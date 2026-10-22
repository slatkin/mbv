# Design

## Context

See proposal.md for the motivation. Current state that shapes the approach:

- The two remote-output fields live on `DaemonRuntimeHooks` (`crates/mbv-daemon/src/run.rs`): `cast_connector: Option<CastConnector>` and `emby_session_output: bool`. `src/local_daemon.rs` supplies `Some(..)` / `true`; `crates/mbvd/src/main.rs` supplies `None` / `false` with comments that state the rule in prose.
- Everything downstream is already derived and conditional: the hello advertises `playback-output` and `emby-session-output` only when the hooks carry them (`core_ctrl_spawn.rs`), `output_state_for_peer` filters the Emby Session output for peers without the capability, `cast/connect.rs` rejects a Cast selection without a connector, and `emby_session/connect.rs` rejects a selection when `emby_session_output` is false.
- `DaemonRole` (`context.rs`) is `Clone, Copy, Debug, Eq, PartialEq` and is compared by value in roughly twenty-five sites across `run.rs`, `event_loop*`, `control.rs`, `owner_settings.rs`, and `core_ctrl_spawn.rs`.
- The tray is the precedent for a Local-only feature: it also rides on `DaemonRuntimeHooks` as a closure the packaged daemon stubs out. That same convention shape is what this change removes for outputs.
- Cast-owner-output design D2 chose hooks-over-features to avoid a build matrix; this change keeps that choice and only re-homes the two fields behind a role boundary.

## Goals / Non-Goals

**Goals:**
- The packaged program cannot be given a Cast connector or an Emby Session output in any build: the exclusion is a property of the packaged entry point's signature, not a value a caller picks.
- The already-correct derived behavior (advertisement, Client offers, rejection with a reason) is pinned by tests that name the contracts.
- The local daemon keeps exactly its current output behavior.

**Non-Goals:**
- No ctrl wire change, no new capability, no version bump.
- No role-typed `PlaybackOutput` inside the daemon core: the internal owner still holds one output enum whose remote arms exist but cannot be reached on the packaged path. Typing the whole core per role would touch every funnel for no additional externally visible guarantee.
- No change to the other convention-hooked Local-only features (tray, Pin swap, notifications). They are out of this issue's scope.

## Decisions

### D1. The remote-output hooks leave `DaemonRuntimeHooks` and live only on the Local entry point

Split the seam at the process boundary:

- `DaemonRuntimeHooks` keeps every current field except `cast_connector` and `emby_session_output`.
- A new `LocalOutputHooks { cast_connector: CastConnector, emby_session_output: bool }` carries the remote-output hooks.
- `mbv-daemon` gets two public run entry points that share one internal run body: the packaged entry (`run_with_options` shape, hooks without remote fields) and a Local entry that additionally takes `LocalOutputHooks`. The internal body receives a private `OwnerOutputs` value derived from which entry point was called.

The packaged binary calls an entry point whose parameters cannot name a `CastConnector`, so the scenario "runtime setup cannot be given a Cast receiver attachment in any build" holds by construction. *Alternatives:* a payload on `DaemonRole::Local` — rejected because the connector is a boxed closure, so the payload would cost `Copy`/`Debug`/`PartialEq` on a type that twenty-five sites compare by value; an `OwnerOutputs` enum with a public `Remote` variant — rejected because `mbvd` could still construct it, which is the convention this change removes; role-typed `PlaybackOutput` through the daemon core — rejected as a far larger refactor with no additional externally visible behavior.

### D2. The Local daemon's Cast connector becomes required

`LocalOutputHooks.cast_connector` is a `CastConnector`, not an `Option`. The Local process is the only Cast-output host and always supplies the connector; an owner that cannot connect to a receiver learns that at selection time, not at startup. The Cast selection rejection path then belongs to the packaged role only. `emby_session_output` stays a flag: the Emby runtime is optional by setup, and the select guard already keys on it. *Alternative:* keep the Option for symmetry — rejected because it would preserve a meaningless "Local daemon without a Cast connector" state that no entry point can produce.

### D3. Internal core fields and guards stay as defense-in-depth

`DaemonCore` keeps `cast_connector: Option<CastConnector>` (now filled only from `LocalOutputHooks`) and `emby_session_output: bool`; the hello derivation, `output_state_for_peer`, and the two selection guards keep their current shape and reason texts. They are now unreachable on the packaged path by construction but remain the stated-reason rejection surface the spec requires. *Alternative:* delete the guards as dead code — rejected: the guards are the observable rejection behavior for any peer that sends a remote selection anyway, and they keep the daemon testable without the packaged binary.

### D4. Pin the derived behavior, do not redesign it

The advertisement, the Client-side offer gating, and the rejection already exist. The change pins them: a packaged hello carries neither remote-output capability; a Client connected to a packaged owner offers no remote output; a remote selection that reaches a packaged owner is rejected with a reason and the output stays Local. Tests live in the existing families (`crates/mbv-daemon/src/tests/`, ctrl wire tests, `src/app/tests/` shell-layer) per the Tests matrix in `docs/architecture/tui-frontend.md`.

## Risks / Trade-offs

- [Entry-point split churn in `run.rs` plumbing] → The internal body is shared; the split adds one parameter and one public wrapper. The two call sites outside `mbv-daemon` are `src/local_daemon.rs` and `crates/mbvd/src/main.rs`, plus test harnesses that build the hooks.
- [A future Local-only feature copies the old convention again] → The spec requirement now states the enforcement rule for outputs; a follow-up can extend the same split to the tray hooks without a new design.
- [The internal `OwnerOutputs` enum is one match away from the old two-field shape] → Acceptable: the unrepresentability guarantee lives at the public seam, which is what the spec scenario pins.

## Migration Plan

Single-workspace change: split the types, move the two call sites, update test harnesses, run the gates. No persisted state, no protocol change, no rollback concern beyond reverting the commit.

## Open Questions

(none)
