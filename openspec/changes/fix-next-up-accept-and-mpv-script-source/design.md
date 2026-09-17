## Context

See `proposal.md` - Why for the observed panic and the stale script copy. Three current
state facts shape the approach:

- `src/app/action.rs:501-507` already routes an explicit play by owner kind
  (`player.is_remote()` -> `player.queue_play_slot(slot_id)`), and
  `PlayerProxy::queue_play_slot` (`crates/mbv-core/src/player/proxy.rs:340`) is the
  existing owner split: `Local` returns `false` (the caller must run the local
  transition), `Remote` sends `CtrlCmd::UnifiedQueuePlaySlot`.
- `crates/mbv-core/src/ctrl.rs:456-465` is a `From<PlayerCommand> for WireCommand`
  conversion whose local-only arm is `unreachable!()`. It is the only place that
  refuses a command with no wire form by aborting the caller thread;
  `RemotePlayer::send_command` (`crates/mbv-core/src/remote_player/mod.rs:173-190`)
  already refuses the three queue-mutation variants by returning `false`.
- An out-of-process owner's refusal of a queue command already reaches the caller as
  `CtrlEvent::CommandRejected` -> `PlayerEvent::CommandRejected`
  (`crates/mbv-core/src/daemon_control_queue.rs:268`,
  `crates/mbv-core/src/remote_player/connect.rs:199`,
  `src/app/player_event.rs:441`), which is the presentation path
  `unified-playback-queue` already requires. This change reuses it and adds no
  second refusal message.

Neither `openspec/specs/mpv-playback-policy/` nor any other spec covers how the mpv
Lua script set is provisioned or resolved; `crates/mbv-core/src/config_paths.rs:48-70`
is the whole current policy, and its second branch (`<repo>/crates/mbv-core/scripts/mbv.lua`)
has never existed on disk.

## Goals / Non-Goals

**Goals:**

- Every client-initiated slot jump reaches the current Player owner through the path
  that owner can accept; a refusal is presented through the existing command-rejection
  path.
- A command with no ctrl wire form fails closed at the boundary instead of killing the
  client.
- Exactly one mpv script set is live, the resolved path is observable at startup, and a
  checkout can load the checkout's scripts.

**Non-Goals:**

- Making the Player owner act on its own on-screen Next-Up accept with no Client
  attached: the affordance stays client-driven, and N attached Clients may each request
  the jump (latest-wins serialization resolves them).
- Restructuring the Lua layer itself: the `loadstring` concatenation loader, the
  `'next-up-skip'` region string repeated across four sites, or file naming. Those are
  separate hazards and need their own change once the live source is deterministic.
- Changing the mpv overlay's presentation, the toast classification for accept, or the
  decision that the on-screen button is the only next-up affordance.
- Deleting files in a user's home directory, or adding a script-set version/hash
  handshake between Rust and Lua.
- Making `JumpTo` a legal ctrl wire command.

## Decisions

### A1. Two owner-kind seams: dispatch an accepted transition, or request a new one

Add two `App`-level operations and route every client-side jump send through one of
them:

- `dispatch_jump(transition) -> bool` carries a transition the owner state has already
  accepted (returned by the expire/settle paths). Out-of-process owner ->
  `queue_play_slot(slot_id_to_u64(transition.target))`; this process is the owner ->
  `send_command(transition.into_jump())`.
- `request_slot_jump(slot_id)` is the fresh-jump path: out-of-process owner ->
  `queue_play_slot`, reporting `false`; this process is the owner ->
  `sync_canonical_queue` + `mint_local_transition` + `accept_local_transition` +
  `dispatch_jump` of the returned `DispatchDecision::DispatchNow` transition.

The expire and settle paths already hold a promoted transition and must NOT re-mint:
`OwnerTransitionState::expire`/`settle`
(`crates/mbv-core/src/playback/transition.rs:104-140`) move `queued_latest` into
`in_flight` and hand it back as `dispatch_next`, so re-accepting it returns
`DispatchDecision::Queued` and strands it until the in-flight timeout. `src/app/player_event.rs:18`
and `:288` therefore call `dispatch_jump` with the transition they already hold;
`src/app/player_event.rs:371` and `src/app/action.rs:527` call `request_slot_jump`.
The two dispatch sites are already reachable only when this process is the owner
(`player_event.rs:8` returns early, `:277` guards the block), so `dispatch_jump` is
what keeps them from diverging if that ever changes.

Alternatives considered: one `QueueSlotId`-taking helper for all four sites (rejected -
the two expire/settle sites hold an already-accepted transition, so re-minting both
loses the promoted jump and corrupts in-flight tracking); one `Transition`-taking
helper only, minting inline at the two fresh-jump sites (rejected - duplicates the
mint/accept block, the duplication class this change closes); a next-up-specific owner
request (rejected - duplicates `UnifiedQueuePlaySlot`, which the owner already
resolves by request identity); letting `JumpTo` cross ctrl (rejected -
`daemon_control.rs:185` deliberately rejects an inbound `WireCommand::JumpTo`, and
design D6 makes request-identity jumps the only ctrl jump path); silent drop on the
local-only arm (rejected - hides the failure the user sees).

### A2. Fail-closed encoding via a fallible conversion

Replace the local-only `unreachable!()` arm with a fallible conversion at the ctrl
boundary (`Result` carrying the unencodable command), so an unencodable command becomes
a rejection the caller can report and the process stays alive.

Alternatives considered: splitting `PlayerCommand` into local and wire variants at the
type level (strongest guarantee, but `PlayerCommand` is matched exhaustively through
`crates/mbv-core/src/player/run/`, so it is a substantially larger blast radius for the
same outcome); extending `RemotePlayer::send_command`'s existing pre-filter list
(`remote_player/mod.rs:173-190`) with the remaining local-only variants (rejected -
leaves the aborting conversion as a latent trap for the next variant, and the
requirement is that the transport refuse to *encode*, not that every caller remember a
filter); keeping the panic (rejected - a user-reachable affordance triggers it).

### A3. A jump requested from an out-of-process owner does not write a client queue cursor

When the jump is requested from an out-of-process owner, the client SHALL NOT set its
queue cursor from the requested slot; the cursor follows the owner's
`UnifiedQueueUpdated` snapshot. Setting it locally would make a client snapshot look
authoritative for active-slot state (`unified-playback-queue`: "A Client MAY hold a
replaceable snapshot ... neither SHALL independently decide canonical order, active
slot ...").

Alternative considered: optimistic local cursor for immediate feedback (rejected - the
existing flash already gives the feedback, and a wrong cursor would be indistinguishable
from an owner-driven one until the next snapshot).

### B1. Resolution order: checkout, then package, and nothing else

`mbv.lua` resolves to the checkout's `scripts/mbv.lua` when that path exists (the
compile-time checkout root, existence-checked at runtime), otherwise
`/usr/share/mbv/scripts/mbv.lua`. The user-data-directory branch is removed, and
`osc_fonts_dir()` follows the same rule, since the deleted installer populated both.

The checkout candidate is derived from `env!("CARGO_MANIFEST_DIR")`, which is
`<checkout>/crates/mbv-core`, so the entry script is `<checkout>/scripts/mbv.lua` and
the fonts are `<checkout>/fonts` (both reached through `../..` from the manifest
value). On an installed system the baked path does not exist and the packaged copy
wins. Resolution order is exactly: checkout if present, else `/usr/share/mbv`; the
removed installer's user-directory path is never a candidate.

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

Resolution takes the candidate paths as inputs - the checkout entry script (or font
directory), the packaged `/usr/share/mbv/...` path, and the removed installer's
user-directory path - and returns the chosen path plus any unused legacy copy found, so
unit tests never read a real HOME, config, or state directory (repository rule: unit
tests must not construct real externals). The legacy path stays an input only so it can
be named in the startup warning; it is never a resolution candidate.

### B4. The resolved path is logged once at player startup, on the path the scripts are
actually handed to mpv

`crates/mbv-core/src/player/runtime.rs:239` already computes the script option; the
resolved-path log and the unused-legacy warning belong there, so they report the value
mpv received rather than a second computation. Both are emitted only where a script set
is actually handed to mpv (the existing `!no_scripts && !use_mpv_config && script.exists()`
gate), which is exactly the condition the spec scenario names.

## Risks / Trade-offs

- [A remaining client-side jump site still sends a local-only command] -> the shared
  seams replace every client-side send site, and A2 makes any missed site fail closed
  and visible instead of panicking; the audit is a task row with a repository-wide
  search as its check, and the reviewer must separate the owner-side dispatch sites in
  `crates/mbv-core/src/daemon_core.rs` from client-side ones.
- [An out-of-process owner legitimately rejects the accept request, so the episode does
  not change] -> the refusal is presented through the existing command-rejection path;
  natural end-of-file advance still occurs, which is the current visible behaviour
  anyway.
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

1. Ship unit A; the panic stops and the accept completes a jump for both owner kinds.
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
