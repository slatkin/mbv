# Design

## Context

See proposal.md (Why). The pinned view state is `App.pinned_width` (`Collapsed` or `Expanded`).
`pinned_view_toggle` (`src/app/dispatch/action.rs`) flips it: it sends one off-thread pinwin
request through `begin_pinned_request` with `PinnedRequestPending::View(width)`, and the drained
outcome runs `apply_pinned_view`. That sets `QueueOnly` with queue focus on collapse, or flashes
the reason on refusal. `pinned_request_pending` blocks a second request.

A user play that starts playback reaches the Owner through `replace_queue_on_owner` from five
tails. Each tail returns early on `QueueOpEdit::NotApplied` and then sends `SetMute`:

| Tail | File |
|---|---|
| `play_items_routed` | `src/app/dispatch/actions.rs` |
| `play_series_continuation` | `src/app/dispatch/actions.rs` |
| `play_single_item` | `src/app/dispatch/actions.rs` |
| `submit_queue_item` (`start_playback == true`) | `src/app/dispatch/actions.rs` |
| `start_pending_queue_playback` | `src/app/dispatch/queue/pending_playback.rs` |

These tails also serve the confirm gate (`run_replacement`), the saved-playlist prompt, and the
Playlists sidebar. A Cancel never reaches them. The websocket Play handler
(`src/app/dispatch/session/ws_event.rs`) calls `replace_emby_queue_on_owner` directly, and it
calls `set_panel_focus(PanelFocus::Queue)` first.

## Goals / Non-Goals

**Goals:**
- One collapse-only step that the five user tails call after an accepted replace.
- The websocket path stays a raw replace with no UI step.

**Non-Goals:**
- Plays from the queue panel (slot jump, queue cursor play). The expanded pinned view shows only
  the library, so these are not reachable there, and they do not replace the queue.
- A change to the existing toasts of any path, including the websocket path.
- A change to the focus rules of the unpinned layouts.

## Decisions

### D1: Call the step from the user tails, not from `replace_queue_on_owner`

`replace_queue_on_owner` is the common choke point, but the websocket path also uses it. A
hook there needs an origin flag on every caller. The five tails are the user paths, and each
already has the "accepted" point (after the `NotApplied` return). The new step goes there.
`replace_queue_on_owner` stays a raw send with no UI effect.

Alternative: an `origin` parameter on `replace_queue_on_owner`. Rejected: it adds a parameter to
every caller to serve one UI rule, and a new caller can pass the wrong origin with no error.

### D2: Share the request with `x`

Split the request part of `pinned_view_toggle` into a private
`request_pinned_view(width)` step. It carries the one-in-flight guard that is in
`pinned_view_toggle` today, so `x` and the play collapse share one pending check.
`pinned_view_toggle` calls it with `pinned_width.toggled()`. The new
`collapse_pinned_view_after_play` calls it with `Collapsed`, but only when
`pinned_width == Expanded`. The outcome then goes through the existing
`PinnedRequestPending::View` drain and `apply_pinned_view`. Thus the queue view, the initial
queue item, and the refusal toast are the same as for `x`.

### D3: Gate on `pinned_width`, not on `crate::pin::is_pinned()`

`pinned_width` becomes `Expanded` only through the pinned toggle and the pinned F2 width rows
(`note_pinned_width_applied` runs only while pinned). Thus `Expanded` already implies pinned.
The gate on `pinned_width` alone keeps the step testable with `make_app_stub`, because
`is_pinned` reads the process identity, which a unit test cannot set.

### D4: One collapse per action

Feed and Audiobookshelf multi-select paths call `submit_queue_item(_, true)` for the first item
and `submit_queue_item(_, false)` for the rest. Only the `true` call runs the step. The second
guard is the pending request: a later call before the outcome finds `pinned_request_pending`
set and does nothing. After the outcome, `pinned_width` is `Collapsed`, so the step does nothing.

### D5: Remove the websocket focus move

Delete `set_panel_focus(PanelFocus::Queue)` from the websocket Play handler. Keep its toasts and
its replace. The handler never calls the D2 step.

## Risks / Trade-offs

- [A new user play tail is added later and does not call the step] -> The panel-mode scenarios
  name play, play all, and shuffle. The tests own the step at the tails that exist now. A
  recipe-like note in the step's doc comment names the rule: every user tail that starts
  playback calls it after the accepted replace.
- [A legacy Owner (`SentLegacy`) has no answer, so the collapse can happen for a replace that the
  Owner later drops] -> This is the same trust that the tails already give `SentLegacy` for
  `SetMute` and the queue scope. Accept it.
- [The collapse focuses the queue while the user still wanted the library] -> The user can press
  `x` to expand again. This is the requested behavior.
