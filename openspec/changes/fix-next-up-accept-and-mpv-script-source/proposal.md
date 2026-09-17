## Why

The mpv Next-Up overlay's accept button is the only offered affordance for
"play the next episode now" (`openspec/specs/toast-notification-semantics/spec.md`), and
pressing it terminates the TUI with a panic instead of jumping:

```
07:18:21 player: next-up: mbv-next-up-play received from Lua
07:18:21 app:    next-up: play triggered
07:18:21 crash:  PANIC ctrl.rs:465:17
                 local-only PlayerCommand never crosses ctrl
```

`src/app/player_event.rs:371` (and the two sibling jump sites at `:18` and `:288`)
mint a local transition and send `PlayerCommand::JumpTo` to `app.player`, which is
the Local daemon's ctrl client whenever a daemon owns playback — the transport has no
wire form for `JumpTo` and asserts that by `unreachable!()`. The same command sent to
the same slot from the explicit-play path is routed by locality instead
(`src/app/action.rs:518` -> `player.queue_play_slot`), so the missing branch is a
duplication defect, not a missing capability.

Separately, no Lua edit made in this repository has any effect on a machine where a
user-data script copy exists. `osc_script_path()`
(`crates/mbv-core/src/config_paths.rs:48`) prefers
`~/.local/share/mbv/scripts/mbv.lua`, which is orphaned output of the `make install`
target deleted on 2026-06-16 (`a6349a30`), still the Jun 21 2545-line monolith
(`ass:append('SKIP')`, `processvolume = true`). It silently beats both the package
copy in `/usr/share/mbv/scripts/` and the checkout, so the label change that produced
`42ef3c5b` never reached the screen, and the divergence is behavioural, not cosmetic.
This is why the two units belong to one change: unit A is unrunnable-in-practice
without unit B, because no Lua affordance can be verified from a checkout today.

## What Changes

**Unit A — client-initiated slot jump is dispatched by owner locality**

- Introduce one slot-jump dispatch path used by every client-initiated jump: when the
  Player owner is remote, request the jump from the owner (the existing
  `PlayerCommand`-free owner request, as explicit play already does); when this client
  is the owner, keep the local mint/settle/jump sequence.
- Apply it to the Next-Up accept event and to the two sibling sites that currently
  send a local-only command unconditionally.
- Make encoding a command with no ctrl wire form fail closed: the client reports a
  rejection it can present, and the process SHALL NOT terminate. Replacing
  `unreachable!()` with a typed refusal is the mechanism, not the requirement.
- Do not write a client-side queue cursor on completion of a remote owner jump; the
  owner snapshot remains the only active-slot authority.

**Unit B — the mpv OSC/overlay scripts have exactly one runtime source**

- Define one resolution order for `mbv.lua` and its sibling fragments, with the
  removed installer's user-data copy no longer silently winning, and log the resolved
  script path at startup.
- Fix the dead development fallback (`<repo>/crates/mbv-core/scripts/mbv.lua`, a path
  that has never existed) so a checkout can load the checkout's scripts.
- **BREAKING (behavioural, local machine state only)**: with the shadow copy out of
  the way, `processvolume = false` takes effect for the first time since June, so
  volume scaling is applied once instead of twice. No wire or file-format change.

## Capabilities

### New Capabilities

- `mpv-overlay-scripts`: how mbv provisions and resolves the mpv Lua script set
  (the OSC/overlay bundle and its fragments), including the guarantee that exactly
  one copy is live and that the resolved copy is observable.

### Modified Capabilities

- `unified-playback-queue`: adds the requirement that a client-initiated slot jump is
  dispatched according to the current Player owner's locality, and that the Next-Up
  accept affordance completes that jump for every owner kind instead of only a local
  owner.
- `ctrl-protocol`: adds the requirement that a command with no ctrl wire form is
  refused fail-closed and never terminates the client process.

## Impact

- `src/app/player_event.rs` (Next-Up accept and the two sibling jump sites),
  `src/app/action.rs` (existing explicit-play precedent), `crates/mbv-core/src/player/proxy.rs`
  and `crates/mbv-core/src/ctrl.rs`.
- `crates/mbv-core/src/config_paths.rs` (`osc_script_path`, `osc_fonts_dir`),
  `crates/mbv-core/src/player/runtime.rs` (script option), `scripts/*.lua` (unchanged
  content), `Cargo.toml` / `PKGBUILD` install mapping if the packaging path changes.
- Local machine state: the stale `~/.local/share/mbv/scripts/mbv.lua` must be removed
  or stop being consulted; nothing in this change writes to a real user directory.
- Tests: app-level dispatch tests with a spy proxy, ctrl-level refusal test, hermetic
  resolution tests with injected directories. Real-mpv verification is a manual check.
