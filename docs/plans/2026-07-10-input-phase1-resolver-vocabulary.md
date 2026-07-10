# Input Phase 1: Resolver Vocabulary + Playback/Help End-to-End — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce the input-resolver vocabulary (`KeyChord`, `InputContext`, `KeyResolution`, `InputSnapshot`, `resolve_key`) and route the two already-pure contexts — Playback and Help — through it end-to-end (snapshot → resolve → dispatch), with zero user-visible behavior change.

**Architecture:** A new module `src/app/input_resolver.rs` holds the pure resolver types and a `resolve_key(context, snapshot, chord)` function. The existing `Action` enum is renamed to `Command`; the existing pure key-translation tables become `KeyChord`-typed helpers that `resolve_key` delegates to. `App::handle_key_help` and `App::handle_playback_key` become thin adapters: build an `InputSnapshot`, call `resolve_key`, act on the `KeyResolution`. This phase does **not** build the full context-priority stack (that is #131 / phase 2) — it proves the per-context resolve→dispatch pipeline on two contexts only.

**Tech Stack:** Rust; `crossterm` (`KeyCode`, `KeyEvent`, `KeyModifiers`) for key events; existing `App`/`Player` test scaffolding (`make_app_stub`, `Player::spy_on_commands`).

## Global Constraints

- Every commit MUST pass, from the repo root: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. (Exact gate from `docs/CHECKIN.md`. `clippy -D warnings` means **no unused-symbol may be committed** — new types must be consumed by non-test code in the same commit.)
- Do NOT add `Co-Authored-By` trailers to commit messages.
- Do NOT push. Commit only. (Pushing requires explicit user permission per `docs/CHECKIN.md` and repo policy.)
- **Behavior-preserving:** no user-visible behavior change in this phase. The App-level characterization tests going through the real `handle_key` entry point are the proof.
- New items are `pub(super)` (module-internal to `src/app`), matching the existing visibility of `Action`/`dispatch`.
- **Symbol-precise renames only.** Never blind-`sed` `Action` → `Command`: substrings like `PendingQueueAction`, `execute_context_action`, `ContextAction`, and `entry.action` MUST NOT change. Use a symbol-aware rename (rust-analyzer rename, or Serena `rename_symbol`).

---

### Task 1: Rename `Action` → `Command`

Pure mechanical rename establishing the `Command` name. No new types, no behavior change.

**Files:**
- Modify: `src/app/action.rs` (enum `Action`, its `impl`/match arms, `dispatch` param, tests)
- Modify: `src/app/input.rs` (mouse-path `Action::ToggleMute` / `Action::TogglePlayPause` / `Action::NextTrack` dispatch calls, and any `super::action::Action` references)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub(super) enum Command` (was `Action`, identical variants); `impl App { pub(super) fn dispatch(&mut self, command: Command) -> bool }`. The pure tables keep their names for now: `playback_action_for_key`, `help_action_for_key` (renamed in Task 2).

- [ ] **Step 1: Rename the enum symbol**

Using a symbol-aware rename, rename the enum `Action` (declared in `src/app/action.rs`) to `Command`. This updates: the `enum Action` declaration, every `Action::Variant` in `dispatch`, the return types `-> Option<Action>` on `playback_action_for_key`/`help_action_for_key`, the `dispatch(&mut self, action: Action)` parameter type, the three `super::action::Action::*` call sites in `src/app/input.rs`'s `handle_mouse`, and all `Action::*` uses in the `action.rs` `tests` module.

Do NOT rename `PendingQueueAction`, `execute_context_action`, `execute_pending_queue_action`, `ContextAction`, or struct fields named `action`.

- [ ] **Step 2: Rename the `dispatch` parameter (cosmetic)**

In `src/app/action.rs`, change `pub(super) fn dispatch(&mut self, action: Command)` to `pub(super) fn dispatch(&mut self, command: Command)` and update the two references to `action` inside the body (the `match action {` and the `Action::ShowSettings => ...` inner re-match on `action`) to `command`.

- [ ] **Step 3: Verify the gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS. Test count unchanged from baseline; no behavior change.

- [ ] **Step 4: Commit**

```bash
git add src/app/action.rs src/app/input.rs
git commit -m "Rename Action to Command"
```

---

### Task 2: Introduce `KeyChord`; retype the translation tables

Create the resolver module with `KeyChord`, and convert the two pure tables to take a `KeyChord` instead of a raw `KeyEvent`. Move the KeyEvent→chord normalization to the two call sites.

**Files:**
- Create: `src/app/input_resolver.rs`
- Modify: `src/app/mod.rs` (add `mod input_resolver;`)
- Modify: `src/app/action.rs` (`playback_action_for_key` → `playback_command_for_key(KeyChord, ...)`, `help_action_for_key` → `help_command_for_key(KeyChord)`, and the `tests::key`/`tests::key_ctrl` helpers)
- Modify: `src/app/input.rs` (`handle_playback_key`, `handle_key_help` call sites build a `KeyChord`)

**Interfaces:**
- Consumes: `Command` (Task 1).
- Produces:
  - `pub(super) struct KeyChord { pub code: KeyCode, pub mods: KeyModifiers }`
  - `impl KeyChord { pub(super) fn new(code: KeyCode, mods: KeyModifiers) -> Self; pub(super) fn from_key(key: KeyEvent) -> Self }`
  - `pub(super) fn playback_command_for_key(chord: KeyChord, active: bool, has_remote_session: bool) -> Option<Command>`
  - `pub(super) fn help_command_for_key(chord: KeyChord) -> Option<Command>`

- [ ] **Step 1: Create the module file with `KeyChord`**

Create `src/app/input_resolver.rs`:

```rust
//! Central input resolution: the single, testable seam that turns a key press
//! (in a given UI context) into a semantic `Command`, a `Swallow`, or a
//! `FallThrough`. See `docs/adr/0002-centralized-input-handling.md`.
//!
//! Phase 1 (#130) covers only the Playback and Help contexts. The full
//! context-priority stack that *selects* the context arrives in phase 2 (#131).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A normalized key press: physical key code plus active modifiers, with the
/// terminal-specific `kind`/`state` fields of `KeyEvent` dropped. This is the
/// unit the resolver matches bindings against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    pub(super) fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code, mods }
    }

    pub(super) fn from_key(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            mods: key.modifiers,
        }
    }
}
```

- [ ] **Step 2: Register the module**

In `src/app/mod.rs`, add the declaration alongside the others (after `mod input;`):

```rust
mod input_resolver;
```

- [ ] **Step 3: Retype the tables to take `KeyChord`**

In `src/app/action.rs`, change the signatures and bodies. `playback_action_for_key` becomes:

```rust
pub(super) fn playback_command_for_key(
    chord: KeyChord,
    active: bool,
    has_remote_session: bool,
) -> Option<Command> {
    let ctrl = chord.mods.contains(KeyModifiers::CONTROL);
    let gated = has_remote_session || active;
    match chord.code {
        KeyCode::Char(' ') if gated => Some(Command::TogglePlayPause),
        KeyCode::Esc if gated => Some(Command::Stop),
        KeyCode::Char('<') if gated => Some(Command::SeekRelative(-5.0)),
        KeyCode::Char('>') if gated => Some(Command::SeekRelative(5.0)),
        KeyCode::Char('N') if gated => Some(Command::NextTrack),
        KeyCode::Char('P') if gated => Some(Command::PreviousTrack),
        KeyCode::Char('z') if !ctrl => Some(Command::CycleOrToggleSubtitle),
        KeyCode::Char('m') => Some(Command::ToggleMute),
        KeyCode::Char('-') => Some(Command::AdjustVolume(-5)),
        KeyCode::Char('+') | KeyCode::Char('=') => Some(Command::AdjustVolume(5)),
        KeyCode::Char('a') if gated => Some(Command::ToggleMuteOrCycleAudio),
        _ => None,
    }
}
```

`help_action_for_key` becomes:

```rust
pub(super) fn help_command_for_key(chord: KeyChord) -> Option<Command> {
    match chord.code {
        KeyCode::Char('q') if chord.mods.is_empty() => Some(Command::Quit),
        KeyCode::Esc | KeyCode::F(1) => Some(Command::CloseHelp),
        KeyCode::F(2) => Some(Command::ShowSettings),
        KeyCode::F(3) => Some(Command::ShowSessions),
        KeyCode::F(4) => Some(Command::ShowPlaylists),
        KeyCode::Up => Some(Command::ScrollBy(-1)),
        KeyCode::Down => Some(Command::ScrollBy(1)),
        KeyCode::PageUp => Some(Command::ScrollBy(-10)),
        KeyCode::PageDown => Some(Command::ScrollBy(10)),
        KeyCode::Home => Some(Command::ScrollHome),
        _ => None,
    }
}
```

Add `use super::input_resolver::KeyChord;` to the top of `src/app/action.rs` (near the existing `use` lines). Keep the existing `use crossterm::event::{KeyCode, KeyModifiers};` (the `KeyEvent` import there may now be unused — if clippy flags it, drop `KeyEvent` from that `use`).

- [ ] **Step 4: Update the two call sites in `input.rs`**

In `src/app/input.rs`, `handle_playback_key`, change the translation line to build a chord:

```rust
let action =
    super::action::playback_command_for_key(KeyChord::from_key(key), active, has_remote_session)?;
```

In `handle_key_help`, change:

```rust
if let Some(action) = super::action::help_command_for_key(KeyChord::from_key(key)) {
    return Some(self.dispatch(action));
}
```

Add `use super::input_resolver::KeyChord;` to the `use` block near the top of `src/app/input.rs`.

- [ ] **Step 5: Update the test helpers to return `KeyChord`**

In `src/app/action.rs`, the `tests` module, change the two helpers so the ~30 existing table tests keep compiling unchanged:

```rust
fn key(code: KeyCode) -> KeyChord {
    KeyChord::new(code, KeyModifiers::NONE)
}

fn key_ctrl(code: KeyCode) -> KeyChord {
    KeyChord::new(code, KeyModifiers::CONTROL)
}
```

Add `use super::super::input_resolver::KeyChord;` inside the `tests` module's `use` section if not already reachable (the module already does `use super::*;`; `KeyChord` is `pub(super)` in a sibling module, so import it explicitly). The existing tests call e.g. `playback_action_for_key(key(...), ...)` — do a symbol-aware rename of `playback_action_for_key` → `playback_command_for_key` and `help_action_for_key` → `help_command_for_key` across the test bodies (they otherwise pass unchanged since `key()` now yields a `KeyChord`).

- [ ] **Step 6: Verify the gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS. The renamed pure-table tests (`space_fires_when_active_only`, `help_esc_fires_close_help`, etc.) still pass unchanged. `KeyChord::new`/`from_key`, `code`, `mods` are all consumed → no dead-code warning.

- [ ] **Step 7: Commit**

```bash
git add src/app/input_resolver.rs src/app/mod.rs src/app/action.rs src/app/input.rs
git commit -m "Introduce KeyChord and type key tables on it"
```

---

### Task 3: Add `resolve_key` + `InputSnapshot`; route Playback and Help through it

Add the remaining vocabulary and the resolver, then convert `handle_key_help` and `handle_playback_key` into snapshot→resolve→dispatch adapters. This is the end-to-end pipeline for two contexts. All new symbols are consumed by the rewires in this same commit (clippy-clean).

**Files:**
- Modify: `src/app/input_resolver.rs` (add `InputContext`, `KeyResolution`, `InputSnapshot`, `resolve_key`, `impl App { input_snapshot }`, tests)
- Modify: `src/app/input.rs` (`handle_playback_key`, `handle_key_help` rewired)

**Interfaces:**
- Consumes: `Command` (Task 1); `KeyChord`, `playback_command_for_key`, `help_command_for_key` (Task 2).
- Produces:
  - `pub(super) enum InputContext { Help, Playback }`
  - `pub(super) enum KeyResolution { Command(Command), Swallow, FallThrough }`
  - `pub(super) struct InputSnapshot { pub player_active: bool, pub has_remote_session: bool }`
  - `pub(super) fn resolve_key(context: InputContext, snapshot: &InputSnapshot, chord: KeyChord) -> KeyResolution`
  - `impl App { pub(super) fn input_snapshot(&self) -> InputSnapshot }`

- [ ] **Step 1: Write the pure resolver unit tests first (red)**

Append to `src/app/input_resolver.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::action::Command;

    fn snap(active: bool, remote: bool) -> InputSnapshot {
        InputSnapshot {
            player_active: active,
            has_remote_session: remote,
        }
    }

    #[test]
    fn help_context_maps_bound_key_to_command() {
        let r = resolve_key(
            InputContext::Help,
            &snap(false, false),
            KeyChord::new(KeyCode::Esc, KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::Command(Command::CloseHelp));
    }

    #[test]
    fn help_context_swallows_unbound_key() {
        // The help overlay consumes every key while open.
        let r = resolve_key(
            InputContext::Help,
            &snap(false, false),
            KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::Swallow);
    }

    #[test]
    fn playback_context_maps_gated_key_to_command_when_active() {
        let r = resolve_key(
            InputContext::Playback,
            &snap(true, false),
            KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::Command(Command::TogglePlayPause));
    }

    #[test]
    fn playback_context_falls_through_when_gate_closed() {
        // Space is a no-op that must reach the view handler when nothing plays.
        let r = resolve_key(
            InputContext::Playback,
            &snap(false, false),
            KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::FallThrough);
    }

    #[test]
    fn playback_context_falls_through_on_unbound_key() {
        let r = resolve_key(
            InputContext::Playback,
            &snap(true, false),
            KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::FallThrough);
    }
}
```

- [ ] **Step 2: Run the resolver tests to confirm they fail to compile (red)**

Run: `cargo test input_resolver 2>&1 | head -20`
Expected: FAIL — `cannot find type InputSnapshot` / `resolve_key` not found. That is the red state.

- [ ] **Step 3: Add the types + `resolve_key`**

Insert into `src/app/input_resolver.rs`, above the `#[cfg(test)]` block, after the `KeyChord` impl:

```rust
use super::action::Command;
use super::App;

/// A UI context that can bind keys. Phase 1 has only the two contexts that
/// already had a pure translation seam; phase 2 (#131) adds the rest and the
/// priority stack that selects among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputContext {
    Help,
    Playback,
}

/// The outcome of resolving a chord in a context.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum KeyResolution {
    /// Dispatch this semantic command.
    Command(Command),
    /// Consume the key with no action (e.g. an overlay eating unknown keys).
    Swallow,
    /// Decline the key; a lower-priority context (or the view handler) handles it.
    FallThrough,
}

/// The plain-data view of app state the resolver reads, so resolution stays a
/// pure function testable without constructing an `App`. Phase 1 carries only
/// the fields the Playback gate needs; phase 2 grows this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InputSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
}

/// Resolve a chord within a single context. Pure: no `App`/`Player` access.
pub(super) fn resolve_key(
    context: InputContext,
    snapshot: &InputSnapshot,
    chord: KeyChord,
) -> KeyResolution {
    match context {
        // The help overlay consumes every key: bound keys become commands,
        // everything else is swallowed (never falls through).
        InputContext::Help => match super::action::help_command_for_key(chord) {
            Some(cmd) => KeyResolution::Command(cmd),
            None => KeyResolution::Swallow,
        },
        // Playback keys are gated; an unbound or gate-closed key falls through
        // to the handlers below it in `handle_key`.
        InputContext::Playback => {
            match super::action::playback_command_for_key(
                chord,
                snapshot.player_active,
                snapshot.has_remote_session,
            ) {
                Some(cmd) => KeyResolution::Command(cmd),
                None => KeyResolution::FallThrough,
            }
        }
    }
}

impl App {
    /// Build the input snapshot from current app state. Single build-site so
    /// "what does input depend on?" has one auditable answer.
    pub(super) fn input_snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            player_active: self.player.status.lock().unwrap().active,
            has_remote_session: self.connected_session_id.is_some(),
        }
    }
}
```

- [ ] **Step 4: Run the resolver tests to confirm they pass (green)**

Run: `cargo test input_resolver 2>&1 | tail -20`
Expected: the five `input_resolver::tests::*` tests PASS.

- [ ] **Step 5: Rewire `handle_key_help` through the resolver**

In `src/app/input.rs`, replace the body of `handle_key_help` with:

```rust
fn handle_key_help(&mut self, key: KeyEvent) -> Option<bool> {
    if !self.show_help {
        return None;
    }
    let snapshot = self.input_snapshot();
    match super::input_resolver::resolve_key(
        super::input_resolver::InputContext::Help,
        &snapshot,
        super::input_resolver::KeyChord::from_key(key),
    ) {
        super::input_resolver::KeyResolution::Command(cmd) => Some(self.dispatch(cmd)),
        // Help swallows unknown keys; FallThrough is unreachable for this
        // context but treated identically (still consumed) to preserve today's
        // "help eats every key" behavior.
        super::input_resolver::KeyResolution::Swallow
        | super::input_resolver::KeyResolution::FallThrough => Some(false),
    }
}
```

- [ ] **Step 6: Rewire `handle_playback_key` through the resolver**

In `src/app/input.rs`, replace the body of `handle_playback_key` with:

```rust
fn handle_playback_key(&mut self, key: KeyEvent) -> Option<bool> {
    let snapshot = self.input_snapshot();
    match super::input_resolver::resolve_key(
        super::input_resolver::InputContext::Playback,
        &snapshot,
        super::input_resolver::KeyChord::from_key(key),
    ) {
        super::input_resolver::KeyResolution::Command(cmd) => Some(self.dispatch(cmd)),
        // Swallow is unreachable for Playback today; both non-command outcomes
        // mean "not a playback key" → let it fall through (`None`).
        super::input_resolver::KeyResolution::FallThrough
        | super::input_resolver::KeyResolution::Swallow => None,
    }
}
```

The now-unused `KeyChord` import added to `input.rs` in Task 2 stays used (via `KeyChord::from_key` here). If clippy flags any leftover `use super::action::playback_command_for_key`-style import in `input.rs`, remove it.

- [ ] **Step 7: Add App-level characterization tests (through the real entry point)**

Append a test module to `src/app/input_resolver.rs` (kept next to the resolver it verifies):

```rust
#[cfg(test)]
mod app_level_tests {
    use crate::app::action::Command;
    use crate::app::tests::make_app_stub;
    use crate::player::PlayerCommand;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn help_f1_closes_help_via_handle_key() {
        let mut app = make_app_stub();
        app.show_help = true;
        let quit = app.handle_key(ev(KeyCode::F(1), KeyModifiers::NONE));
        assert!(!quit);
        assert!(!app.show_help, "F1 closes the help overlay");
    }

    #[test]
    fn help_swallows_unbound_key_via_handle_key() {
        let mut app = make_app_stub();
        app.show_help = true;
        let quit = app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(!quit);
        assert!(app.show_help, "an unbound key is swallowed; help stays open");
    }

    #[test]
    fn space_toggles_pause_when_active_via_handle_key() {
        let mut app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
        }
        let rx = app.player.spy_on_commands();
        app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
    }

    #[test]
    fn space_does_not_toggle_pause_when_idle_via_handle_key() {
        let mut app = make_app_stub();
        let rx = app.player.spy_on_commands();
        // Idle home tab: Space must not emit a transport command (it falls
        // through to the view handler, which ignores it).
        app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(
            !matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)),
            "Space is inert while nothing plays"
        );
    }
}
```

Note: if `PlayerCommand`'s path is not `crate::player::PlayerCommand`, match the import used by the existing `playback_header_mouse_tests` in `src/app/input.rs` (it imports `PlayerCommand` via `use super::*;`). Confirm the correct path with `grep -rn "enum PlayerCommand" src/` and adjust the `use`.

- [ ] **Step 8: Run the full gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: PASS. New tests green; all pre-existing tests (including the `dispatch_*` and `playback_header_mouse_tests`) still green — proving no behavior change.

- [ ] **Step 9: Commit**

```bash
git add src/app/input_resolver.rs src/app/input.rs
git commit -m "Route playback and help through input resolver"
```

---

## Self-Review

**Spec coverage (issue #130):**
- `KeyChord` incl. printable-char handling → `KeyChord` delivered (Task 2). Printable-char *helper* deliberately deferred: it would be dead code under `clippy -D warnings` until text-entry contexts consume it in phase 2. Noted here so it is a conscious cut, not a gap.
- `InputContext`, `KeyResolution { Command | Swallow | FallThrough }`, `InputSnapshot` → Task 3. ✓
- `Command` (superset of `Action`) → renamed in Task 1; grows in later phases. ✓
- Route Playback + Help end-to-end (snapshot → resolve → dispatch) → Task 3, Steps 5–6. ✓
- Pure resolver unit tests → Task 3, Step 1. ✓
- No behavior change elsewhere; other contexts stay in `handle_key` → only `handle_key_help`/`handle_playback_key` bodies change; `handle_key`'s branch order is untouched. ✓

**Placeholder scan:** No TBD/TODO; every code step shows complete code. The two "confirm the import path" notes (Task 3 Step 7; `KeyEvent` drop in Task 2 Step 3) are concrete verification instructions with the exact `grep` to run, not deferred work.

**Type consistency:** `Command` used consistently after Task 1. `playback_command_for_key(chord, active, has_remote_session)` and `help_command_for_key(chord)` signatures match between definition (Task 2) and call in `resolve_key` (Task 3). `InputSnapshot { player_active, has_remote_session }` field names match between struct def, `input_snapshot()`, the pure-test `snap()` helper, and `resolve_key`. `KeyResolution` variant names (`Command`/`Swallow`/`FallThrough`) match across resolver, both rewires, and tests.

**Clippy-at-every-commit check:** Task 1 adds no new symbols. Task 2's `KeyChord::new`/`from_key`/`code`/`mods` are all consumed by non-test code in the same commit (test helpers, tables, `input.rs` call sites). Task 3's `InputContext`/`KeyResolution`/`InputSnapshot`/`resolve_key`/`input_snapshot` are all consumed by the two rewired handlers in the same commit. No dead-scaffold commit.
