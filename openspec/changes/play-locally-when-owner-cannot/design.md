## Context

See `proposal.md` for motivation and `specs/non-audio-fall-through/spec.md` for
the requirements this design satisfies.

Two attachment shapes reach the same problem, and they carry different
information today:

- **ctrl-attached owner.** `PlayerProxy::Remote` over `RemotePlayer`. The
  handshake already learns peer capabilities: `CtrlHello::current()` lists
  `CTRL_CAP_*` strings (`crates/mbv-core/src/ctrl.rs:79`), `validate_peer`/
  `compatibility` resolve them into `CtrlCompatibility` (`ctrl.rs:183`), and
  `perform_handshake` copies the per-peer flags onto it
  (`crates/mbv-core/src/remote_player/connect.rs:60-85`). The client already
  exposes one capability this way: `PlayerProxy::can_admit_audiobookshelf`
  reads `remote.ctrl_compatibility.supports_abs_queue`
  (`crates/mbv-core/src/player/proxy.rs:82`). No playable-media capability
  exists, and no capability string for audio-only exists at all.
- **Emby-session control.** `connected_session_id`/`connected_session_state`
  (`src/app/app_struct.rs:254`). `SessionInfo` parses `SupportedCommands` only
  (`crates/mbv-core/src/api_client_sessions.rs:98`,
  `crates/mbv-core/src/api_types.rs:413`), so the client cannot see the
  `PlayableMediaTypes: ["Audio"]` that an audio-only `mbvd` already advertises
  (`crates/mbv-core/src/api_client_reporting.rs:251-284`).

A refusal cannot be used as the trigger. On the ctrl path, `daemon_admits`
already strips non-audio items before the admission check
(`crates/mbv-core/src/daemon_control_queue.rs:231`), so a wholly-video
submission fails the "nothing admitted" check and is rejected with the generic
`"Playback owner rejected the queue replacement"`
(`crates/mbv-core/src/daemon_control.rs:341-348`) — the audio-only-specific
message (`crates/mbv-core/src/daemon_core.rs:555`) is not reachable from there,
and `CtrlEvent::CommandRejected` carries only a human string (`ctrl.rs:512`).
On the Emby-session path there is no refusal signal at all: the client's
`session_play` POST succeeds (`api_client_sessions.rs:204`) while the daemon
logs and drops the request (`crates/mbv-core/src/daemon_ws.rs:44`).

Explicit Emby play funnels through `App::play_item` and
`App::play_items_routed` (`src/app/actions.rs:257` and `:221`), which between
them cover library Enter, Ctrl+P, album/artist tracks, shuffle, context-menu
play/shuffle, Home Continue Watching, TV episodes, the selection modal, and the
queue-PlayItems executor. Both already branch on `connected_session_id` and on
`has_direct_remote_queue()`.

The confirmation modal is shared infrastructure: `ConfirmAction`/`ConfirmModal`
(`src/app/types_confirm.rs`), key map (`src/app/components/confirm.rs:123`),
effect dispatch (`src/app/input_confirm_keys.rs:15`), dismissal
(`src/app/shell_modal_actions.rs:146`), raised through `App::ask_confirm`
(`src/app/app_struct.rs:394`). The pattern for a payload too large for
`ConfirmAction` (which is `Clone, Debug, PartialEq`) already exists:
`App::pending_queue_action: Option<PendingQueueAction>` with the unit
`ConfirmAction::DiscardOrSaveDirtyPlaylist` (`src/app/queue_actions.rs:247`,
`src/app/types_playback.rs:194`).

Ending an attachment and returning to local playback exists as
`App::restore_local_mode` (`src/app/session_switch.rs:280`). It restores a
suspended local Player when one exists and reconnects the local daemon when the
app's baseline is the local daemon; for a client launched straight onto a remote
daemon it severs the connection and leaves no local Player at all. The ordinary
local construction path is `Player::new` + `PlayerProxy::local`
(`src/app/construct.rs:259-276`).

ADR 0017 currently prescribes the parked-owner model and states that
`restore_local_mode` is explicitly *not* the path back from fall-through
(`docs/adr/0017-composed-and-bound-queue-stages.md:55-66,124-131`).

## Goals / Non-Goals

**Goals:**

- Decide eligibility and route the play before anything is submitted or local
  state is mutated.
- Keep the decision and the prompt in one place that every explicit Emby play
  entry point already passes through.
- Make the confirmed path end up as an ordinary local play of the selection,
  with no new playback mechanism.
- Change no daemon command, no queue authority rule, and no protocol version.

**Non-Goals:**

- Keeping the attachment alive during local playback (the parked-owner model and
  its arrangement boundary, origin-tagged events, independent queue scope, and
  pinned owner-queue row).
- Prompting for enqueue, for feed/`QueueItem` playback, or for Library-route
  attachments (see Decisions D7).
- Changing the daemon's admission filter, the ctrl wire shape of
  `CommandRejected`, or the Emby session API's play request.
- Reattaching to the owner automatically after a fall-through.

## Decisions

### D1. Decide from an advertised capability, before submitting

The guard runs before submission and consults the owner's advertised
playable-media fact.

*Alternatives considered:* reacting to `CommandRejected` and offering the
prompt afterwards. Rejected — the ctrl path's rejection reason is generic
(see Context), the Emby-session path produces no signal at all, and by the time
a rejection arrives the play sites have already replaced the client's queue
presentation and set queue scope, which the prompt would then have to unwind.
This matches ADR 0017's own reasoning ("routing is capability-led: the client
decides before submitting") even though this change reverses its parked-owner
conclusion.

### D2. The ctrl fact rides the existing capability path

Add `CTRL_CAP_AUDIO_ONLY`, a `CtrlHello::supports_audio_only()` reader, a
`CtrlCompatibility::supports_audio_only` flag assigned in `perform_handshake`
like `supports_lifecycle_shutdown`, and `PlayerProxy::owner_is_audio_only()`
(false for an in-process Player) modeled on `can_admit_audiobookshelf`.

Additive only: `ctrl-protocol` already requires an audio-only daemon to
advertise this capability and explicitly forbids a protocol-version bump for it
alone. An older daemon that never advertises it reads as "able to play", which
is exactly today's behavior.

*Alternatives considered:* a required hello field or a version bump (rejected —
breaks mixed-version pairs for no gain); deriving it from the Emby session list
alone (rejected — unavailable for a direct daemon attachment, which is the
common case).

### D3. The Emby-session fact comes from the sessions API

Add `playable_media_types: Vec<String>` to `SessionInfo`, parsed beside
`supported_commands` (`api_client_sessions.rs:98`). Because
`connected_session_state` *is* a `SessionInfo`, the fact is then available
wherever the client holds session state, with no second cache.

Empty/absent types mean "unknown" and read as able to play, consistent with the
ctrl rule.

### D4. One guard, at the two explicit-play entry points

`App::owner_cannot_play(&self, items: &[EmbyItem]) -> PlaybackEligibility` (or an
equivalent internal helper) evaluates: an attached, non-route owner known to be
audio-only, and whether any item is non-audio.

- wholly unplayable → defer the play, raise the prompt;
- mixed → filter nothing, submit as today, flash the dropped count;
- wholly playable, or capability unknown, or a Library route → proceed exactly
  as today.

The guard is called at the top of `App::play_item` and
`App::play_items_routed`, before any queue replacement, scope change, focus
change, or status flash. No per-UI-call-site checks are added; the funnel is
assumed to stay the funnel, and the `shell_emby_library` request handlers keep
translating straight into these two methods.

*Alternatives considered:* guarding inside `submit_queue_item`/
`execute_pending_queue_action` (rejected as the primary site — the single-item
`play_item` path does not pass through them, and they are also the local
executors the confirmed path must reuse).

### D5. The deferred play reuses `PendingQueueAction`

`ConfirmAction::PlayLocallyInstead` is a unit variant; the pending play lives in
a new `App::pending_local_play: Option<PendingQueueAction>` field holding
`PlayItems { items, start_idx, source, autostart: true }`. This follows
`DiscardOrSaveDirtyPlaylist`'s existing shape, keeps the modal's `Clone/Debug/
PartialEq` state small, and lets the confirmed effect run the *ordinary* local
play path (`execute_pending_queue_action`) after the attachment is gone, when
`has_direct_remote_queue()` is false and the queue replacement targets Local.

### D6. Confirmed effect: prepare, stop, detach, then play

Order:

1. prepare a local Player — restore `suspended_local` when present, otherwise
   construct one through `Player::new` + `PlayerProxy::local`; if preparation
   fails, report it and leave the attachment untouched;
2. stop the attached owner (ctrl `stop`, or the session transport Stop), so it
   does not keep playing underneath;
3. end the attachment and clear the presentation state that
   `restore_local_mode`'s tail already clears (`remote_player_tab = None`,
   `connected_session_id`/`connected_session_state = None`, queue scope to
   Local, direct-remote flags, `active_route`), and rebind MPRIS to the new
   Transport owner;
4. run the deferred local play.

Extract the shared tail of `restore_local_mode` into one helper used by both
paths rather than adding a boolean mode flag to it; the fall-through entry owns
the "construct when nothing is suspended" step that `restore_local_mode`
deliberately does not do.

*Alternatives considered:* calling `restore_local_mode` unchanged (rejected —
for a client launched onto a daemon it leaves a disconnected remote proxy and no
local Player, so step 4 could not start local playback).

### D7. Scope exclusions, deliberately

- **Library routes** stay ineligible (spec: "Library route is ineligible"). A
  route is a routing *policy* with its own restore semantics
  (`apply_route_for_playback`, `last_remote_connection`); having the prompt sever
  it would also re-route the next play of the same item straight back to the
  audio-only owner.
- **Enqueue** never prompts and never stages locally (spec, REMOVED
  requirement "Enqueuing locally does not change transport ownership"). Today's
  behavior is retained: the append is submitted, the owner's admission drops
  what it cannot play, and a wholly-unplayable append surfaces the owner's
  refusal.
- **Feed/`QueueItem` playback** is out of scope: `submit_queue_item` takes a
  `QueueItem`, not an `EmbyItem`, and reusing the deferred payload for it would
  add a second arm for a case that is audio in practice. Add it if a video feed
  ever prompts for it.

### D8. Prompt wording and keys

Title names the owner or session, message names the selection, hint reads
`[y] Play here    [n] Cancel`. `y`/`Y`/Enter accept (`ConfirmIntent::Accept`),
`n`/`N`/Esc decline (`ConfirmIntent::Cancel`), and every other key stays inside
the modal without acting. This means an explicit `Key::Char('n')` arm in
`confirm_intent_for_key` (existing arms do not bind `n`) and a matching arm in
`confirm_key_dismisses`.

Declining clears `pending_local_play` and changes nothing else.

### D9. Mixed selection reports through a Neutral toast

The count of items the owner cannot play is information about an action that did
proceed, so it uses the existing Neutral class (2 s, in-app, no desktop
notification) rather than Warning. `toast-notification-semantics` needs no
delta: no class is added or changed.

## Risks / Trade-offs

- [The daemon in use may not advertise the capability, so no prompt appears]
  → Unknown reads as capable, so behavior is never worse than today; the
  improvement lands as soon as both ends are new. No version bump is needed, and
  the capability is ignored by peers that do not know it.
- [Severing ends control of the owner and stops its playback] → Accepted (the
  user chose this over the parked-owner model). Stopping rather than pausing
  prevents two audio sources, and the owner's queue is untouched by the
  fall-through and re-adopted on reattach.
- [A headless client may have no usable local player (no mpv output)] →
  Preparation is a distinct step whose failure keeps the attachment and reports
  the failure instead of half-detaching.
- [Local playback replaces the client's own local queue] → Same as any local
  play; the owner's queue and the daemon's state are unaffected.
- [ADR 0017 now contradicts the shipped behavior] → Add a dated correction to
  ADR 0017 in this change (its accepted rationale for parking the owner is
  explicitly overridden); do not silently diverge from an accepted ADR.
- [Reading `play_item` suggests a single-item library play on a remote-daemon
  attachment may never submit the selection at all: the `direct_remote` branch
  skips `replace_playback_queue` and then submits the existing remote queue
  (`src/app/actions.rs:283-300`), and no test covers it] → This is the same
  boundary the guard sits on, so verify it with a test first; if it is real, fix
  it here (minimal, one boundary) rather than shipping a prompt on top of a path
  that never submits.
- [Two entry-point guards could drift from the funnel if a future surface calls
  the queue executors directly] → Keep the guard in one helper with its own
  tests; the confirmed path is the only intended direct caller of the executor
  for this case.

## Migration Plan

No persisted state, no protocol-version change, no data migration:

- Old daemon + new client: no capability advertised → no prompt, current
  behavior. New daemon + old client: capability ignored, as `ctrl-protocol`
  requires. New + new: the prompt appears.
- Rollback is reverting the change; the only durable artifact touched is the
  `non-audio-fall-through` spec, whose Purpose is updated in this change and
  whose requirements are synced on archive.
- Sequence: capability (core) first, then the client fact and guard, then the
  modal and the confirmed effect, then the ADR correction and the spec sync.

## Open Questions

- Exact prompt and toast wording is not fixed; tests should assert that the
  prompt is raised, that it names the owner and the selection, and that the
  count is reported, not the prose itself.
