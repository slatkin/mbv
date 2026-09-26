# Design

## Context

See proposal.md, Why. Current facts that shape the approach (re-measured on `main` @ `922802192`, after #814 Tier 1 and the #804 lint pass):

- All `App` fields are `pub(in crate::app)`, and tests construct `App` through `App::build(AppInit)` (`src/app/state/construct.rs`). Every moved field therefore forces edits at each access site. Those edits are mechanical and found by the compiler.
- Measured spread of the candidate seams (files that reference them): image fields ~18 files, mostly `src/app/infra/images*`, `shell/`, `render/components/card.rs`; setup/startup receivers ≤5 files each; remote tracking ≤11 files each, except `connected_session_id`/`connected_session_state` (34 and 21 files); `lib_tx`/`lib_rx` 24 files, mostly tests.
- The `lib`, `search`, `sessions`, `notif_action` and `card_image` channel pairs are not created in `App::build`. They are created at three `AppInit` construction sites and passed in: `App::new_independent` (`src/app/state/construct.rs`), `App::new_remote_optional_with_config` (`src/app/state/construct/remote.rs`) and the test helper in `src/app/tests.rs`. Only the `cast` pair is created inside `build`.
- The #804 lint pass left two approved suppressions that name this change as their blocker: `#[expect(clippy::too_many_lines)]` on `App::build` (`construct.rs`, reason: splitting is "blocked on" decompose-app-god-type) and `#[expect(clippy::struct_excessive_bools)]` on `App` (`app_struct.rs`, reason counts 26 bools). Three of those bools move into seams here: `image_protocol_enabled`, `remote_stalled_while_paused`, `direct_remote_connected`.
- `PlayerEvent` is an `mbv-core` type (`crates/mbv-core/src/player/types.rs`) shared with the daemon, so it cannot be split into nested enums without crossing the crate and protocol boundary. `LibEvent` is TUI-local (`src/app/state/types/events.rs`, 35 variants, ~300 uses across 58 files).

## Goals / Non-Goals

**Goals**
- Each of the four seams has one named owner type, and its fields are reachable only through that type's field on `App`.
- Both event dispatchers are exhaustive single matches, so adding an event variant fails the build at the dispatch site. This follows AGENTS.md: "exhaustive dispatch arm or documented no-op — never wildcard-hidden".
- The `too_many_lines` suppression on `App::build` is removed: seam constructors shrink `build`, and the #804 reason names this change as the blocker. The `struct_excessive_bools` suppression on `App` gets its reason count updated to the bools still on `App`, and is removed if that count drops to 3 or fewer.

**Non-Goals**
- Shell `Model`. It has ~18 fields, each already documented as shell-owned projection/arbitration state. It has no cross-cutting subsystem cluster comparable to `App`'s. Revisit it in its own change if a seam appears.
- `connected_session_id` / `connected_session_state`. Their 39/24-file spread makes them session identity rather than tracking internals. They stay on `App`.
- Replacing the direct-remote trio with a state enum. Current writes are not always paired: the route switch in `dispatch/session/switch.rs` clears `connected` and `session_id` but keeps `label`, and `connect.rs` sets `connected` without a label. Encoding a state machine would change behaviour. This change only groups the fields and leaves the enum as a follow-up.
- Splitting `LibEvent` into nested per-family enums. It would rewrite ~300 constructor/match sites for the same compile-time guarantee the exhaustive match already gives.
- Shrinking the 85-file `impl App` spread for methods that span several subsystems. Only single-seam methods move.

## Decisions

**D1. Sub-struct per seam, owned by value on `App`.**
The fields are `pub(in crate::app) images: ImageCache`, `setup: ServiceSetup`, `remote: RemoteTracking`, `channels: RuntimeChannels`. The alternatives were `Arc`/shared handles (rejected by AGENTS.md: prefer owned data) and traits over `App` (hide nothing, since every `impl App` still reaches every field). Sub-struct fields keep `pub(in crate::app)` visibility for this change, so the move stays compile-forced and mechanical. Tightening to private fields plus methods is per-seam follow-up work, done once call sites are few.

**D2. File placement.**
- `ImageCache` → `src/app/infra/images/cache.rs` (next to its worker code).
- `ServiceSetup` → `src/app/state/service_setup.rs`.
- `RemoteTracking` → `src/app/state/remote_tracking.rs`.
- `RuntimeChannels` → `src/app/state/runtime_channels.rs`.

Each file has `pub(in crate::app) fn new(...) -> Self` (or a constructor taking the values `construct.rs` builds today). `App::build` calls that constructor. The constructor lands before any field is deleted, so every step compiles.

Channels are the exception to "`App::build` calls the constructor", because their pairs are created before `build` (see Context). `RuntimeChannels::new()` creates all five pairs, `cast` included, and replaces the separate `mpsc::channel()` calls at the three `AppInit` sites. `AppInit` carries `channels: RuntimeChannels` in place of its loose sender/receiver fields, and `build` moves it onto `App`. `card_image_tx`/`card_image_rx` still start at the same three sites; `AppInit` carries them into `ImageCache`'s constructor.

`images/cache.rs` stays inside `src/app/infra/images`, which #814 Tier 5 marks for extraction as an `mbv-images` crate, so grouping the cache there moves it toward that boundary.

**D3. Test-only instrumentation follows its data.**
`card_image_fetch_calls` and `image_protocol_builds` (`#[cfg(test)]`) move into `ImageCache`, still `#[cfg(test)]`.

**D4. Player dispatch returns `PlayerEventFlow`.**

```rust
pub(in crate::app) enum PlayerEventFlow { Proceed, RestartLoop }
```

`RestartLoop` replaces `true` ("caller `continue`s the run loop"; see `drains.rs::drain_player_events`). `handle_player_event` becomes one `match ev { ... }` with an arm per `PlayerEvent` variant. Each arm calls the existing per-event helper or holds its existing inline body, and returns a flow. The three `Result<bool, PlayerEvent>` stages (`handle_player_event_playback`, `_notices`, `_queue_state`) and the bool-returning `handle_player_event_progress` are all deleted. Where their arms had bodies, those bodies move into the single match or into a named helper when longer than ~10 lines. Wildcard `_ =>` arms are forbidden. Variants that are intentionally ignored get an explicit arm with a comment (for example `NextUpThreshold`, `IntroEnded`). The production callers are `drains.rs` and `action.rs`; tests also call the handler directly. They compare against the enum or ignore it as they do today.

**D5. Library dispatch is one exhaustive match.**
`handle_lib_event` matches all 35 `LibEvent` variants once in one wildcard-free match, and each arm routes its typed fields to one named handler. Inline arm bodies in `handle_browse_event`, `handle_music_event`, `handle_playlist_event` and `handle_audiobookshelf_event` become `fn handle_<variant_snake>(&mut self, ...)` methods, placed in the same file their body lives in today. The four family dispatchers and both `unreachable!()` arms (`event.rs` playlist and trailing arms) are deleted. Intentionally ignored variants use explicit, documented no-op arms. Do not retain LibEvent-taking recheck wrappers or silent mismatch returns. The `unreachable!()` in `dispatch/library/browse/tv.rs` is not event routing and is out of scope.

The user approved one per-instance exception to the no-suppressions rule for this exhaustive routing function: `#[expect(clippy::too_many_lines, reason = "Exhaustive LibEvent routing keeps each variant visible at one dispatch site")]` on `src/app/dispatch/library/event.rs::handle_lib_event`. This exception is approved for that function only; it does not authorize other suppressions. It records the approved design, not confirmation that the code has implemented it.

## Risks / Trade-offs

- [Large mechanical diff touching many test files] → One seam per task group, each ending green on check/clippy/nextest before the next starts. No behaviour edits are mixed into the moves.
- [Inline arm moved into a helper silently changes an early `return`] → Early `return None` / `return Ok(..)` inside a moved body becomes `return` from the new helper. The implementer re-reads each moved arm for control flow. Existing tests in `src/app/tests/` cover the drained events.
- [New seam files trip pedantic workspace lints (for example `must_use_candidate`, `struct_excessive_bools`)] → Fix at the source. Any new `#[expect]` needs per-instance user approval (AGENTS.md).
- [Files cross 800 lines after new helpers] → The file-lines check runs only before push (AGENTS.md). Split then if needed.

## Migration Plan

Internal refactor, no persisted-state or protocol impact. Rollback is `git revert` of the task-group commits.
