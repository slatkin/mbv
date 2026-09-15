## Why

mbv can attach to a Player owner that cannot play video — a packaged `mbvd
--audio-only` reached over ctrl (direct daemon attach or Sessions-panel direct
remote control; the Local stay-alive daemon is never audio-only) or an Emby
session that advertises audio media types
only. Pressing play on a movie there submits the item anyway:

- over ctrl the owner refuses it and the client shows a generic 5-second error
  line ("Playback owner rejected the queue replacement") — nothing plays;
- through the Emby session API the request is reported as accepted and the item
  silently never plays, because the owner drops it after the fact.

Neither ending offers the user the one useful next step, playing the item on this
machine, and the only workaround today is to disconnect by hand, play it, and
reconnect. The client cannot even see the fact it would need to decide
beforehand: an audio-only owner advertises that fact to the Emby server
(`PlayableMediaTypes: ["Audio"]`) and, per the existing `ctrl-protocol`
requirement, to ctrl peers — mbv reads neither.

`openspec/specs/non-audio-fall-through/` already specifies a client-side answer:
automatic, prompt-free routing to the client's own Player while the owner stays
attached, parked, and commandable (ADR 0017). It was never implemented — the
change was archived 2026-08-12 with its tasks unchecked and its uncommitted plan
audited and deleted in #484 — and it is not the behaviour requested here. This
change replaces it with the chosen behaviour: tell the user the attached session
cannot play the item, and offer to play it here, which ends the attachment.

## What Changes

- Implement the existing `ctrl-protocol` audio-only capability advertisement: an
  audio-only daemon advertises it in the hello handshake, and the client keeps
  the peer's fact on the connection. Additive capability only — no
  protocol-version change.
- Read the attached Emby session's advertised playable media types, so a client
  controlling a session through the Emby session API knows the same fact.
- Before an explicit play, when the attached owner is known to be unable to play
  any of the selection, raise the shared confirmation prompt instead of
  submitting: "This session can't play video. Play '<name>' on this machine
  instead? [y] Play here [n] Cancel".
- On confirm: stop the owner, end the attachment, and play the selection on a
  local Player — constructing one when this client was launched straight onto
  the daemon and has no suspended local Player.
- On decline: submit nothing and leave the attachment and its queue untouched.
- A mixed selection is not prompted: it submits the selection to the owner
  and reports how many items the owner cannot play.
- Explicit enqueue keeps today's behaviour: no prompt, no staging on the
  client's own queue.
- Rewrite `openspec/specs/non-audio-fall-through/` in place around
  check-before-submit, the prompt, and ending the attachment.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `non-audio-fall-through`: how an explicit play is routed when the attached
  owner cannot play the selection. The check the client makes before submitting,
  the confirmation prompt, the owner stop, the ended attachment, and local
  playback replace the automatic fall-through and the attachment-preserving,
  parked-owner requirements.

## Impact

**Protocol and core** — `crates/mbv-core/src/ctrl.rs` (audio-only capability
constant and compatibility flag), `remote_player/connect.rs` (handshake),
`remote_player/mod.rs`, `player/proxy.rs` (`owner_is_audio_only` alongside the
existing `can_admit_audiobookshelf` capability query),
`api_client_sessions.rs` and `api_types.rs` (`SessionInfo` gains the session's
advertised playable media types).

**Client** — `src/app/types_confirm.rs` (new `ConfirmAction` variant),
`components/confirm.rs` (accept/cancel keys), `input_confirm_keys.rs` (the
confirmed effect), `actions.rs` (`play_item`, `play_items_routed` guards),
`queue_actions.rs` (reuses `execute_pending_queue_action`), `session_switch.rs`
(stop the owner, end the attachment, restore or construct the local Player), and
the MPRIS rebind for the new Transport owner.

**Docs** — `openspec/specs/non-audio-fall-through/spec.md` is rewritten. ADR 0017
currently prescribes the parked-owner model and argues explicitly that
`restore_local_mode` is *not* the way back from fall-through; this change
reverses that decision and the ADR needs a dated correction or a superseding
record.

**Deliberately not changed** — enqueue handling, Library-route eligibility,
feed/queue-item playback, and the daemon's admission filter, which already
guarantees an audio-only owner never binds an unplayable item. The
`ctrl-protocol` spec's stale "protocol version SHALL be 9" line (the code is at
10) is pre-existing drift, left to a separate sync because this change does not
touch the version.
