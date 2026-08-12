## Context

See `proposal.md` for motivation and ADR 0017 for the domain decision.

The current client usually lets one binary answer stand for several facts:
`player.is_remote()` influences which Player receives commands, whether a remote
queue exists, which Queue scopes are available, and where queue events apply.
Those facts coincide during ordinary local or remote playback but diverge during
fall-through: the owner stays attached with a live Bound queue while the local
Player receives transport controls.

Relevant current constraints:

- `self.player`, `player_rx`, `ws_rx`, and `ws_send_tx` represent the Player
  currently driven by the application.
- `suspended_local` preserves a previously constructed local Player while a
  remote owner is driven. An explicit-endpoint client may not have constructed a
  local Player yet; it can construct one when fall-through first needs it.
- `remote_player_tab` is the attached owner's queue projection, but
  `has_direct_remote_queue()` currently also requires `player.is_remote()`.
- The single `handle_player_event` path has no event origin and applies queue and
  lifecycle effects through the currently driven Player.
- Library routes and Sessions-panel Direct remote control are deliberately
  distinct relationships under ADR 0011.

## Goals / Non-Goals

**Goals:**

- Represent attachment, Transport owner, queue availability, visible Queue
  scope, Submission destination, and event origin as separate responsibilities.
- Keep the attached owner and its Bound queue live and commandable while local
  fall-through playback owns transport controls.
- Make invalid player arrangements difficult to represent by keeping active and
  parked session resources behind one typed arrangement boundary.
- Decide routing synchronously before existing play/enqueue sites mutate state.
- Preserve the provenance and lifecycle semantics of Direct remote control and
  explicit remote daemon attachment.

**Non-Goals:**

- Fall-through through a Library route.
- A general multi-owner or arbitrary routing framework.
- Changing Session watch, Library-route resolution, or daemon queue admission.
- Making Queue scope choose the Transport owner or Submission destination.
- Changing the ctrl protocol version.

## Decisions

### Player arrangement owns the real sessions

Introduce a typed player-arrangement boundary whose states contain the actual
active and parked session resources. Conceptually it distinguishes:

- local playback with no qualifying attached owner;
- an eligible owner receiving transport controls, optionally retaining a local
  session for reuse; and
- local fall-through playback with the eligible owner parked but attached.

The boundary owns each session's `PlayerProxy`, player-event receiver, and any
local websocket resources that move with that session. It exposes semantic
queries and command accessors rather than a global local/remote answer:

- whether an eligible owner is attached;
- which session is the Transport owner;
- whether Local and Remote Queue scopes are available;
- the session that owns a given queue; and
- whether a fallen-through item is playing.

The exact enum and helper names may follow repository conventions, but it SHALL
not pair independent `active_target` / `is_fall_through` flags with unrelated
session fields. Variant/resource placement is the source of truth.

*Alternative considered:* retain the current fields and add checks for
`suspended_remote.is_some()`. Rejected because the existing helpers would still
conflate queue availability with command destination, spreading fall-through
exceptions across callers.

*Alternative considered:* add a stored active-target enum beside `self.player`.
Rejected because it duplicates what the contained active session already says
and permits contradictory combinations.

### Queue visibility and command destination are independent

Remote Queue scope remains available whenever an eligible attached owner has a
Bound queue, including during local fall-through playback. Visible scope is the
user's requested Queue scope constrained only by queue availability; it is not
derived from the Transport owner.

Transport commands use the Transport-owner session. Queue mutations use the
session that owns the selected queue. Therefore a Remote queue append, remove,
move, or replace reaches the parked owner during fall-through rather than the
active local Player.

Explicit play/enqueue actions do not infer their Submission destination from
visible Queue scope. A pure routing decision considers relationship eligibility,
the owner's advertised capability, action kind, and selection contents.

### Eligibility is relationship-specific

Fall-through is eligible only when either:

- Sessions-panel Direct remote control has established a ctrl connection; or
- the application was launched against an explicit non-local daemon endpoint.

An active Library route is ineligible even though it also uses ctrl and may
advertise audio-only. Session watch is ineligible because it has no ctrl queue
control. The user-session Local daemon does not advertise audio-only.

Connection code may retain capability facts generically on `RemotePlayer`; the
fall-through predicate still checks relationship eligibility before using them.

### Submission routing is per action

For an eligible attached audio-only owner:

- wholly audio play/enqueue targets the owner;
- mixed play/enqueue targets the owner after stripping non-audio items and
  reporting their count; and
- wholly non-audio play/enqueue targets the client's own queue/Player.

Every explicit action is evaluated independently. A non-audio enqueue changes no
transport ownership because it starts nothing. A non-audio play first prepares
a local Player, constructing one if no reusable session exists. Only after that
preparation succeeds does it stop the owner, transition the arrangement to local
fall-through playback, and submit local play. An owner-directed explicit play
while fall-through is active first ends local playback and transitions transport
ownership back to the owner.

Preparation or playback-start failure follows the ordinary local playback error
path. Preparation failure leaves owner playback undisturbed; a later start
failure may leave the owner stopped but does not end the attachment.

### Player events carry client-side origin

`PlayerEvent` remains unchanged in `mbv-core` and on the wire. The application
knows which owned session receiver produced each event and supplies an origin of
Local or Attached-owner to its reducer.

During fall-through:

- a Local terminal event ends fall-through and returns transport ownership to
  the owner if it remains attached;
- an Attached-owner `QueueUpdated` refreshes only the Remote queue;
- an Attached-owner stop/completion updates owner state only and cannot mutate,
  consume, or stop the Local queue;
- an Attached-owner rejection is reported as an owner-command failure;
- an Attached-owner disconnect or shutdown ends the attachment and removes
  Remote scope while local playback continues; and
- owner authority notifications do not change local transport ownership.

*Alternative considered:* drain and discard parked-owner events. Rejected
because the Remote queue must remain live and attachment lifecycle events remain
material.

*Alternative considered:* add origin to `mbv-core::PlayerEvent`. Rejected
because Local and Attached-owner are roles relative to one App, not facts known
by the emitting player or ctrl peer.

### Capability advertisement gets a daemon constructor

Add `CtrlHello::current_daemon(audio_only: bool)`, delegating to `current()` and
appending the additive capability only for audio-only daemons. `current()` and
the client-hello constructors keep their existing meanings. Thread `audio_only`
to daemon hello construction for local and TCP ctrl listeners.

Remote handshake state records whether the peer advertised audio-only. An absent
capability means false and preserves current mixed-version behavior.

### The pinned row is a projection

When Remote Queue scope is visible during local fall-through playback, render
the local now-playing item above the owner's items using selected-row styling,
an explicit client-playing marker, and local progress. Do not insert it into the
owner's queue model. It has no queue slot, cannot receive cursor focus, and
cannot be the subject of a queue command.

## Risks / Trade-offs

- **The arrangement boundary touches many existing helpers.** → Convert callers
  by responsibility—transport, attachment, queue availability, queue owner, or
  relationship provenance—and avoid compatibility helpers with ambiguous
  local/remote meaning.
- **Late events arrive after an arrangement transition.** → Preserve stable
  Local/Attached-owner origin from the receiver that produced them and reduce
  them against that origin, never the currently active variant alone.
- **The owner disconnects during local playback.** → Continue local playback,
  remove the attachment and Remote scope, and report the loss prominently.
- **Stopping the owner discards its position.** → Accepted per ADR 0017.
- **A client exits during fall-through.** → The owner remains stopped; no
  recovery or automatic restart is introduced.

## Migration Plan

No data migration. The ctrl capability is additive and the protocol version is
unchanged. Older clients ignore it; newer clients treat its absence as false.
