## Context

See `proposal.md` - Why for the observed panic and the stale script copy. Two current
state facts shape the approach:

- `src/app/action.rs:518` already routes an explicit play by owner locality
  (`player.is_remote()` -> `player.queue_play_slot(slot_id)`), and
  `PlayerProxy::queue_play_slot` (`crates/mbv-core/src/player/proxy.rs:340`) is the
  existing locality split: `Local` returns `false` (the caller must run the local
  transition), `Remote` sends `CtrlCmd::UnifiedQueuePlaySlot`.
- `crates/mbv-core/src/ctrl.rs:456-465` is a `From<PlayerCommand> for WireCommand`
  conversion whose local-only arm is `unreachable!()`. It is the only place that
  refuses a command with no wire form, and it refuses by aborting the caller thread.

Neither `openspec/specs/mpv-playback-policy/` nor any other spec covers how the mpv
Lua script set is provisioned or resolved; `crates/mbv-core/src/config_paths.rs:48-70`
is the whole current policy, and its second branch (`<repo>/crates/mbv-core/scripts/mbv.lua`)
has never existed on disk.

## Goals / Non-Goals

**Goals:**

- Every client-initiated slot jump reaches the current Player owner through the path
  that owner can accept, with a presentable outcome on rejection.
- A command with no ctrl wire form fails closed at the boundary instead of killing the
  client.
- Exactly one mpv script set is live, the resolved path is observable at startup, and a
  checkout can load the checkout's scripts.

**Non-Goals:**

- Restructuring the Lua layer itself: the `loadstring` concatenation loader, the
  `'next-up-skip'` region string repeated across four sites, or file naming. Those are
  separate hazards and need their own change once the live source is deterministic.
- Changing the mpv overlay's presentation, the toast classification for accept, or the
  decision that the on-screen button is the only next-up affordance.
- Deleting files in a user's home directory, or adding a script-set version/hash
  handshake between Rust and Lua.
- Making `JumpTo` a legal ctrl wire command.

## Decisions

### A1. One slot-jump dispatch helper, keyed on owner locality

Add a single `App`-level operation that takes a resolved `QueueSlotId` and returns a
dispatch outcome; it branches on `player.is_remote()`: remote -> `queue_play_slot`,
local -> `sync_canonical_queue` + `mint_local_transition` + `accept_local_transition` +
`send_command(t.into_jump())`. `src/app/player_event.rs:18`, `:288`, `:371` and
`src/app/action.rs:527` all call it.

Alternatives considered: a next-up-specific owner request (rejected - duplicates
`UnifiedQueuePlaySlot`, which the owner already resolves by request identity); letting
`JumpTo` cross ctrl (rejected - `daemon_control.rs:185` deliberately rejects an inbound
`WireCommand::JumpTo`, and design D6 makes request-identity jumps the only ctrl jump
path); silent drop on the local-only arm (rejected - hides the failure the user sees).

### A2. Fail-closed encoding via a fallible conversion

Replace the local-only `unreachable!()` arm with a fallible conversion at the ctrl
boundary (`Result` carrying the unencodable command), so an unencodable command becomes
a rejection the caller can report and the process stays alive.

Alternatives considered: splitting `PlayerCommand` into local and wire variants at the
type level (strongest guarantee, but `PlayerCommand` is matched exhaustively through
`crates/mbv-core/src/player/run/`, so it is a substantially larger blast radius for the
same outcome); keeping the panic (rejected - a user-reachable affordance triggers it).

### A3. A remote accept does not write a client queue cursor

When the jump is requested from a remote owner, the client SHALL NOT set its queue cursor
from the requested slot; the cursor follows the owner's `UnifiedQueueUpdated` snapshot.
Setting it locally would make a client snapshot look authoritative for active-slot state
(`unified-playback-queue`: "A Client MAY hold a replaceable snapshot ... neither SHALL
independently decide canonical order, active slot ...").

Alternative considered: optimistic local cursor for immediate feedback (rejected - the
existing flash already gives the feedback, and a wrong cursor would be indistinguishable
from an owner-driven one until the next snapshot).

### A4. Rejection is presented with the existing explicit-play wording

A refused remote jump reuses the message explicit play already flashes
("Playback owner rejected the queue selection") instead of a next-up-specific string.

Alternative considered: a distinct next-up message (rejected - one failure, one string;
`ui-design-language` prefers reusing an existing presentation over a new variant).

### B1. Resolution order: checkout, then package, and nothing else

`mbv.lua` resolves to the checkout's `scripts/mbv.lua` when that path exists (the
compile-time checkout root, existence-checked at runtime), otherwise
`/usr/share/mbv/scripts/mbv.lua`. The user-data-directory branch is removed, and
`osc_fonts_dir()` follows the same rule, since the deleted installer populated both.

Alternatives considered: keeping the user-directory copy as an explicit override that
logs loudly (rejected - it preserves a second writable source of truth, and the packaged
copy is always present for installed users); env-var override (rejected - a new user
concept for a case with no user).

### B2. A legacy user-directory copy is ignored loudly, never deleted

When resolution finds a script or font copy at the removed installer's path, startup
logs a warning naming the ignored path and states that the checkout/package copy is in
use. mbv SHALL NOT delete or rewrite it; removal stays a documented manual step.

Alternative considered: migrating/deleting it automatically (rejected - the app must not
write to real user directories to correct its own past installer).

### B3. Resolution is a pure function over injected inputs

Resolution takes the candidate base directories as inputs and returns the chosen path
plus any shadow findings, so unit tests never read a real HOME, config, or state
directory (repository rule: unit tests must not construct real externals).

### B4. The resolved path is logged once at player startup, on the path the scripts are
actually handed to mpv

`crates/mbv-core/src/player/runtime.rs:239` already computes the script option; the
startup log belongs there so it reports the value mpv received, not a second computation.

## Risks / Trade-offs

- [A remaining client-side jump site still sends a local-only command] -> the shared
  helper replaces all known sites, and A2 makes any missed site fail closed and visible
  instead of panicking; the audit is a task row with a repository-wide search as its
  check.
- [A remote owner legitimately rejects the accept request, so the episode does not
  change] -> the rejection is presented; natural end-of-file advance still occurs, which
  is the current visible behaviour anyway.
- [Removing the user-directory preference breaks someone who deliberately keeps a script
  copy there] -> B2 warns by name at startup; the packaged copy is present for every
  installed user.
- [A compile-time checkout path baked into a release binary] -> the path is
  existence-checked at runtime and is absent on an installed system, so the packaged
  copy wins.
- [Volume behaviour changes once `processvolume = false` finally takes effect] -> called
  out as a BREAKING behavioural note in the proposal and as a manual verification row.
- [No automated coverage of real mpv script loading] -> resolution and dispatch are
  unit-tested; the real-mpv run is a manual check row, per the repository rule against
  live tests.

## Migration Plan

1. Ship unit A; the panic stops and the accept completes a jump under every owner.
2. Ship unit B; on the first start after upgrade, startup reports the resolved script
   path. On a machine with the removed installer's copy, the warning names it and the
   user deletes it to silence it; nothing else changes for an installed user.
3. Rollback is a revert of either unit independently: unit A restores the previous
   dispatch, unit B restores the previous resolution order.

## Open Questions

- Whether the script set should later carry a version or content stamp that the player
  compares against the build, so a future skew is detected rather than merely reported
  by path. Deferrable: path reporting already covers the observed failure, and a stamp
  would need a Lua-side protocol addition of its own.
