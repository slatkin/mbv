# Design

## Context

See proposal.md (Why). The three slots today:

| Field (`app_struct.rs`) | Writer | Readers |
|---|---|---|
| `pending_queue_action: Option<PendingQueueAction>` | `replace_queue_or_prompt` (`dispatch/queue/replacement.rs:19`) | `confirm_discard_or_save_dirty_playlist` (`input/confirm_keys.rs`): `[d]` takes it, Esc/`[c]` clears it, and it peeks to compute `play_after`. `handle_playlist_mutation_complete` (`dispatch/run_loop/session.rs:115`) takes it on any successful save of the queue's playlist. |
| `pending_queue_replacement: Option<(PendingQueueAction, ReplacementExecutor)>` | `request_queue_replacement` (`replacement.rs:62`) | `confirm_replace_populated_queue`: `[y]`/Enter takes it, every other key clears it. |
| `pending_local_play: Option<PendingQueueAction>` | the local fall-through prompt (`dispatch/actions.rs:99`) | `confirm_play_locally` clears it on `[n]`/Esc; `play_pending_local_play` (`dispatch/session/switch.rs:338`) takes it. |

Facts that shape the design:
- **Closing a prompt without its arm.** `ask_confirm` / `dismiss_confirm`
  replace or unmount the single confirm modal without running the old
  modal's arm. Callers: `drains.rs:223` (`clear:yes`), `library/event.rs:314`
  (playlist deleted), and any `ask_confirm` while a prompt is showing. The
  held payload survives.
- **Mutation ids are reliable.** Playlist mutations are FIFO per playlist with
  no coalescing (`enqueue_playlist_mutation` / `finish_playlist_mutation`,
  `dispatch/queue/playlist.rs`), so every enqueued save completes under its
  own `mutation_id`.
- **Other paths finish at the same boundary.** `save_playlist_to_emby` is also
  called by the consume auto-save and the quit save (`dispatch/consume_quit.rs`),
  and they complete at the same `PlaylistMutationComplete` boundary.

## Goals / Non-Goals

**Goals:**
- No code outside the deferral module can read or write a slot except
  through its owner's named transition.
- The three payload types are distinct.
- The save boundary can only run a payload bound to the save it is
  completing.

**Non-Goals:**
- Changing the gate or local-play prompt behaviour.
- Carrying payloads inside `ConfirmAction`. It lives in `mbv-ui-model` and is
  handed to `ConfirmComponent`. Domain payloads with `EmbyItem`s and a
  non-`Clone` executor must not cross into components (AGENTS.md,
  Interactive architecture).
- Clearing stale gate or local-play payloads when their prompt closes without
  an answer. Only their own confirm arm reads them, and their writer
  overwrites them before the next prompt, so a stale one stays unused.

## Decisions

**D1. One module, private fields, one type per slot.** Add
`src/app/state/queue_deferrals.rs`:

```rust
pub(in crate::app) struct QueueDeferrals {
    save: Option<SaveDeferral>,
    gate: Option<GatedReplacement>,
    local_play: Option<LocalPlay>,
}
enum SaveDeferral {
    AwaitingAnswer(PendingQueueAction),
    AwaitingSave { mutation_id: u64, action: PendingQueueAction },
}
struct GatedReplacement(PendingQueueAction, ReplacementExecutor);
struct LocalPlay(PendingQueueAction);
```

`App` gets one field, `queue_deferrals: QueueDeferrals` (a `Default` impl in
place of the three `None`s in `construct.rs`). Every transition is a
`pub(in crate::app)` method named for its one caller:

| Slot | Transition | Caller |
|---|---|---|
| save | `defer_for_save_answer(action)` | `replace_queue_or_prompt` |
| save | `save_answer_is_play() -> bool` | `[d]` arm's `play_after` |
| save | `take_on_discard() -> Option<PendingQueueAction>` (only from `AwaitingAnswer`) | `[d]` |
| save | `bind_to_save(Option<u64>)` (`AwaitingAnswer` → `AwaitingSave`; `None` drops) | `[s]` |
| save | `cancel_save_answer()` | Esc/`[c]` |
| save | `take_on_save_complete(mutation_id) -> Option<PendingQueueAction>` (only `AwaitingSave` with that id) | `handle_playlist_mutation_complete` |
| gate | `hold_gated_replacement(action, executor)` | `request_queue_replacement` |
| gate | `take_confirmed_replacement()` | the gate arm |
| gate | `cancel_gated_replacement()` | the gate arm |
| local | `hold_local_play(action)` | `dispatch/actions.rs` |
| local | `take_confirmed_local_play()` | `play_pending_local_play` |
| local | `cancel_local_play()` | `confirm_play_locally` |

Cross-slot misuse no longer compiles, because the payloads no longer share a
type or a field. A wrong reader would have to call a transition named for
another boundary, which review can see.

*Alternative:* merge into one `Option<QueueDeferral>` enum. Rejected: a save
can still be in flight while a new gated prompt is showing (pinned by
`an_in_flight_save_completion_never_executes_an_unconfirmed_gated_replacement`),
so the slots have independent lifetimes. One slot would force one to
overwrite the other.

*Alternative:* newtypes only, with the fields left `pub(in crate::app)`.
Rejected: code could still reach in, and the save state machine is the part
that fixes the live hazard.

**D2. Bind the save deferral to a mutation id.** `save_playlist_to_emby`
returns `Option<u64>`: the enqueued `mutation_id`, or `None` on its early
returns. The `[s]` arm passes it to `bind_to_save`. The consume and quit
callers ignore it.

`handle_playlist_mutation_complete` calls
`take_on_save_complete(mutation_id)` unconditionally, before its existing
guards, and runs the result only when `succeeded`, the origin is current, and
the queue's playlist id matches, as it does today. It then dismisses the
sidebar and focuses the Queue. So a failed or stale-origin bound save consumes
its payload without running it, while every other completion (a different
id) leaves `AwaitingSave` alone.

*Alternative:* bind to the playlist id. Rejected: that is today's rule, and
it lets an auto-save or quit save of the same playlist fire the payload.

## Risks / Trade-offs

- [A failed save now drops the deferred replacement.] Today it stays armed
  until some later save succeeds, which is the hazard. The error toast already
  shows, and the user re-issues the action.
- [Test churn.] `input/confirm_keys/tests.rs` (19 field reads),
  `replacement_gate.rs`, and two route tests assert on the fields. They need
  read-only views: `#[cfg(test)]` accessors on `QueueDeferrals`
  (`is_save_deferred`, `has_gated_replacement`, `has_local_play`). They must
  not be used in production code.
- [`app_struct.rs` is at 517 lines.] The change removes three fields and their
  doc comments, so it shrinks.
