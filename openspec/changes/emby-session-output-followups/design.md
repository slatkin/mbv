# Design

## Context

- `CtrlCmd::UnifiedQueueLoadIdle` has no sender since #945 deleted the
  attached-session path. Its footprint: the `UnifiedQueueLoadResult` event and
  `QueueLoadResult`/`QueueLoadRequestId` vocabulary, `OwnerGateRejection::QueueLoadIdle`,
  `RemotePlayer::load_queue_idle` and its result relay (`connect.rs`), the daemon's
  `control/queue_load.rs` with `pending_idle_load` (`core.rs`) and its arms in
  `event_loop/player_events.rs` and `event_loop/control_events.rs`, and the dead
  result arm in `src/app/dispatch/session/player_event.rs`.
- `PeerBuild::MustMatch` (`crates/mbv-remote-player/src/connect/handshake.rs`) compares
  `CtrlHello.app_version` with `env!("CARGO_PKG_VERSION")` before `send_client_hello`
  (docs/invariants/15). The live test attached a stale same-version Owner and Enter on a
  Session was a silent no-op.
- The Emby Session output loads the active item plus the next admissible slot (D14 of
  the archived `emby-session-owner-output` change). At the queue's end the device holds
  one item, and its own next-episode autoplay starts; today that item is presented as
  content the owner did not load.
- The `owner-queue-load` hello capability also gates `UnifiedQueueSourceUpdate`, which
  stays.

## Goals / Non-Goals

**Goals:**
- The three actionable items of issue #949 land as one change with three task groups.
- The populate-only owner load leaves no dead vocabulary anywhere (wire, daemon, TUI,
  tests, spec text).
- The build gate refuses a stale same-version Owner loudly, before any credential
  leaves the terminal.
- The device's own autoplay past the queue's end becomes queue content the owner tracks.

**Non-Goals:**
- The Brave Linux stall (#949 item 4). Reopen if it reproduces.
- A ctrl protocol bump (user decision, see D2).
- Adoption of mid-item foreign content; the stale-item correction (D14) is unchanged.
- Changing `PeerBuild::Any` for explicit `unix://`/`tcp://` endpoints and packaged `mbvd`.

## Decisions

### D1. Remove the idle-load path as one unit; keep the `owner-queue-load` capability

Everything named in Context goes in one sweep: the vocabulary dies with its only
handler, so no half-state can send a command nothing decodes. `broadcast_player_event_if_not_replaced`
collapses into `broadcast`: its `replacement_committed` flag is only ever true for a
committed pending load (`event_loop/player_events.rs`), so after the sweep it is a
constant. `CTRL_CAP_OWNER_QUEUE_LOAD` and `supports_owner_queue_load` stay because
`QueueOp::SourceUpdate` still requires them. Owner restore of a persisted queue is a
separate idle-install path in the queue-persistence code and is untouched. The test
family that owns the removed contract dies with it; the surviving contract "an answered
replacement replaces and plays" is already owned by the replace tests.

*Alternative:* deprecate the command but keep the handler. Rejected: dead vocabulary
with a live handler is exactly the state #949 asks to end, and the handler would be
exercised by nothing.

### D2. No protocol bump (user decision)

`CTRL_PROTOCOL_VERSION` stays 10. A stale pre-change peer's `UnifiedQueueLoadIdle` line
fails deserialization and is skipped, the same fate the ctrl-protocol spec's version
history documents for v9 `UnifiedQueue*` lines. The local-attach case — the one that bit
the live test — is covered by D3's build gate. Explicit endpoints keep `PeerBuild::Any`
and accept the stale-peer risk.

*Alternative:* bump to 11 so stale binaries are refused at the handshake. Rejected by
the user: it kills every running daemon and client for a command no shipped sender emits.

### D3. Build identity: git SHA plus dirty marker, embedded by `mbv-ctrl/build.rs`

`CtrlHello` gains `build_id: String` with `#[serde(default)]`. Both hellos share
`CtrlHello`, so the Owner's hello carries it without a second type, and old peers decode
and ignore it (additive — no bump). `crates/mbv-ctrl/build.rs` runs
`git rev-parse --short=12 HEAD`, appends `-dirty` when `git status --porcelain` is
non-empty, and falls back to `unknown` when git is unavailable (tarball builds);
the build script emits `cargo:rerun-if-changed` for the paths
`git rev-parse --git-path HEAD`, the symbolic ref's `--git-path`, the branch ref file
under `<git-common-dir>/refs/heads/<branch>`, the `packed-refs` report, and the
per-worktree HEAD reflog (`--git-path logs/HEAD`), so the embedded value stays fresh
across commits. Each path is emitted only when it exists. The common-dir branch ref is named because in
a linked worktree the per-worktree path for a shared branch does not exist. The reflog
is the commit signal when the branch ref is packed: `git pack-refs` folds the loose ref
into `packed-refs`, which a later commit never rewrites, while every commit appends to
the reflog. It never names a literal `.git/HEAD`: in a worktree `.git` is a file, and
Cargo reruns a build script on every build when a named path is missing, which would
recompile `mbv-ctrl` and every dependent each time. When git is unavailable it emits no
git path. The `-dirty` marker is computed only when the script reruns (a HEAD, ref, or
reflog change), so edits made on the same commit keep the old marker.

`read_server_hello` refuses under `PeerBuild::MustMatch` when the `app_version` differs
or the `build_id` differs, and treats an empty owner `build_id` as a mismatch — that is
precisely the stale-Owner case, since a pre-change binary sends none. The refusal stays
ahead of `send_client_hello` (invariant 15). `RemotePlayerErrorKind::OwnerBuildMismatch`
carries the Owner's version and build identity; `src/owner_restart.rs` prints both sides'
values; the restart/quit contract is unchanged.

*Alternatives:* a build timestamp or binary mtime — rejected: every rebuild would prompt
a restart of a healthy Owner after an unrelated recompile. A content hash — rejected: the
Client cannot recompute the Owner's binary identity at handshake time.

### D4. Adoption fires on the end rule, not on succession

The trigger (owner-playback-output ADDED requirement) is the tracked item ending within
10 seconds of its runtime — the same end rule as the D14 device-move completion — while
the device holds no queued next item and the Session starts an item the owner did not
load. Two observation paths reach it. (a) Direct: one observation presents the tracked
item and the next presents the foreign item. (b) After an idle gap: the idle observation
already ended the tracked item (`ended_naturally` records completion and leaves the output
idle at the queue edge), and the first foreign item the Session presents within
`EMBY_SESSION_LOAD_TIMEOUT` (30 s) after that end is adopted. The 30 s bound keeps another
app's playback minutes later present-only. Path (b) records no second completion: the
natural end already did, and the ended slot records completion once on either path. Emby's
behavior between two items is not documented and a 1 s poll can see either path, so both
are covered. A mid-item interruption (another app) fails the end rule and stays present-only; a
stale queued item keeps its correction path because the transition starts from a
registered queued item. The adopted item is appended at the queue tail (past any
unplayable tail slots), becomes the active slot, and is the tracked item from then on:
Session progress, completion recording, and end inference treat it exactly like a loaded
item, and the ended slot records completion under the device-move rule. After an adoption
the device holds only the adopted item, so a later user append takes the existing "queue
grows after the last slot" path (`play_last`).

The adopted slot's item comes from an Emby lookup: `SessionApi` gains `get_item(item_id)`
(the `EmbyClient` impl uses `get_items_by_ids`; the fake returns a canned item). The
observation alone cannot build the slot, because `QueueItem::Emby` holds a full `EmbyItem`
(episode and season index, path, played state) that `SessionInfo` does not carry. On a
foreign start that meets the trigger the owner submits one lookup on the Session worker.
When the result returns, the owner appends the slot only while the Session still presents
that item; a failed lookup, or a device that moved on, leaves the item presented and not
adopted, and the failure is logged. Adoption marks the owner queue dirty: the
`DaemonEvent::EmbySession` arm returns a clean `EventOutcome`, so without it the slot would
not persist until a later command.

Next and previous on an adopted item resolve over the queue like any active slot
("Next and previous load the target slot directly"): next at the queue edge does nothing
(spec scenario "Next on an adopted item follows the queue"). The device's autoplay chain
keeps growing the queue at each item end. *Alternative:* forward Next at the edge to the
device's own next command — rejected: it splits Next semantics between queue-relative and
device-relative depending on where the queue happens to end.

Detection extends the Session observation table beside the D14 device-move row ("the
Session presents an item the owner did not load after the tracked item reached its end
with no queued next item" installs the adoption), so the 1 s poll needs no new mechanism.
Hand-over at exit needs no change: adopted items are queue slots and ride the existing
`play_last` hand-over.

### D5. Docs ride with the code

`docs/invariants/15` states that the gate compares version and build identity (and fixes
its stale path reference — the check lives in `connect/handshake.rs`), including the
`unknown` fallback and the same-commit limitation. `docs/invariants/13`'s "until the next
idle load or clear" becomes "until the next queue replacement or clear". `docs/architecture/owner-client-operation-split.md` (Client column) and the `QueueEpoch` entry in `CONTEXT.md` stop naming `UnifiedQueueLoadIdle` and idle loads.

## Risks / Trade-offs

- [A stale pre-change peer on an explicit endpoint sends the removed command] → its line
  is skipped silently on the wire (D2). Accepted: nothing shipped sends it, and the local
  attach case is refused loudly by D3.
- [Another app starts an item exactly as the tracked item reaches its end] →
  indistinguishable from autoplay; the end rule adopts it. Accepted: the window is one
  poll tick at an item boundary.
- [The item lookup is slow, fails, or the device moves on first] → the item stays presented
  and not adopted (D4). The next autoplay at that item's end is judged by the same rule, but
  with no tracked slot it is not adopted: accepted, the failure is logged.
- [A non-git build compiles `build_id` as `unknown`] → two `unknown` builds degrade to
  today's version-only comparison. Documented in invariant 15.
- [The same commit is rebuilt with no tree change] → identical identity, so a restarted
  stale Owner is not detected. The gate is build differentiation, not build freshness.
  Documented in invariant 15.
- [A dev who rebuilds often sees the mismatch prompt] → that is the intended behavior
  (it is the bug this fixes); the prompt is one key to restart or quit.

## Migration Plan

None. No persisted state changes and no wire negotiation to roll out; old binaries
interoperate per D2. The change ships in one release.

## Open Questions

None.
