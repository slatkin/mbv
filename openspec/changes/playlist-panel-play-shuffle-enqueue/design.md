# Design

## Context

See proposal.md (Why) for the motivation and `specs/playlist-management/spec.md` for the behaviour. The current code has this shape:

- `PlaylistsComponent` (`crates/mbv-components/src/playlists.rs`) handles its keys directly and emits `ShellRequest::PlaylistsActivate { open, index }`.
- `src/app/shell/playlists.rs` handles that request. On a playlist row it calls `App::load_and_play_playlist` (`src/app/dispatch/library/load.rs`), which fetches the items synchronously. In the open view it builds the action from the already-loaded `playlists_open_items`. Both paths build `PendingQueueAction::PlayItems { autostart: false }` with `ReplacementExecutor::Pending`.
- `run_replacement` (`src/app/dispatch/queue/replacement.rs`) closes the Playlists sidebar and focuses the Queue after a `Pending` replacement, but only when the action's source is `QueueSource::Playlist`. The two F4 paths are the only production callers of `Pending`.
- `execute_pending_play_items` (`src/app/dispatch/queue/pending_playback.rs`) branches on `autostart`. `false` goes to `load_idle_queue_on_owner`, and `true` goes to `start_pending_queue_playback`.
- The Music tree already sends `MusicTreeAction { Play, Enqueue, Shuffle }` (`crates/mbv-ui-msg/src/intents.rs`) on the keys `p`/`a`/`s`.

## Goals / Non-Goals

**Goals:**
- One request shape for the three panel actions in both views.
- Remove `autostart` entirely. Do not replace it with another flag.
- One row presentation shared by the playlist list and the open-playlist view.

**Non-Goals:**
- Moving the panel keys into the configurable keybind registry. The panel's existing `n`/`d`/`r` keys are hardcoded the same way.
- Making the playlist item fetch asynchronous. Play already blocks on it today. Shuffle and Enqueue reuse that path.
- Changing the owner idle-load protocol (`UnifiedQueueLoadIdle`). Attached Session and cast playback still use it through `load_idle_queue_on_owner`.

## Decisions

### D1. `PlaylistsActivate` becomes `PlaylistsAction { open, index, action: MusicTreeAction }`

The component emits Enter → `Play`, `s` → `Shuffle`, `a` → `Enqueue`. `open` and `index` mean the same thing as in today's `PlaylistsActivate`. The mouse double-click also emits `Play`.

`MusicTreeAction` is reused as it is. It is already the closed set `{Play, Enqueue, Shuffle}` in `mbv-ui-msg`, and its doc comment is reworded so that it no longer names only the Music artist. *Alternative:* a new `PlaylistAction` enum identical to it. Rejected because two copies of the same three cases would drift apart.

### D2. Shell resolution: one helper for items, one arm per action

`load_and_play_playlist` becomes a resolver, `playlist_playable_items(&mut self, playlist_id) -> Option<Vec<EmbyItem>>`. It does the existing synchronous fetch and the folder filter, and it shows the same messages for unavailable, error, empty, and nothing-playable. In the open view the items come from `playlists_open_items` (non-folders) with no fetch. The shell arm then does this:

| Action | List view (playlist row) | Open view (item row) |
|---|---|---|
| Play | `PlayItems{ items, start 0, source Playlist }` | `PlayItems{ items, start = cursor item, source Playlist }` |
| Shuffle | `PlayItems{ shuffled items, start 0, source Shuffle }` | same, using the whole open playlist |
| Enqueue | `append_on_owner(viewed scope, all items)` | `append_on_owner(viewed scope, [cursor item])` |

Play and Shuffle go through `request_queue_replacement`, so the populated-queue confirmation and the dirty saved-playlist prompt behave as they do today. Shuffle uses `rand`'s `shuffle`, as `ContextAction::ShuffleSelection` does. Enqueue sends a single `append_on_owner` with every item instead of one `submit_queue_item` per item, so one owner answer covers the whole append. Enqueue leaves the sidebar open, so the user can add several playlists in a row.

### D3. `ReplacementExecutor::Pending` becomes `ReplacementExecutor::PlaylistsSidebar`

Today the sidebar dismissal checks for `source: Playlist`. A shuffled queue has `source: Shuffle`, so the Shuffle action would leave the sidebar open. Rather than add another source to that check, the executor variant states its own after-step. `Pending` already has only the Playlists sidebar as its production caller, so it is renamed. After the replacement runs and no overlay was raised, `PlaylistsSidebar` always closes the sidebar and focuses the Queue.

*Alternative:* add `QueueSource::Shuffle` to the `matches!`. That is smaller, but it keeps inferring the caller from the data it carries.

### D4. `PendingQueueAction` = `PlayItems { items, start_idx, source }` + `ClearQueue`

The field and the `if !autostart` branch in `execute_pending_play_items` are deleted, and the function always starts playback. The remote-disconnected guard becomes unconditional, which matches its current behaviour for every remaining caller.

`load_idle_queue_on_owner` and `send_idle_queue_load` stay because of their other callers in `src/app/dispatch/actions.rs` (attached Session, cast).

*Alternative:* restore a third `LoadItems` case, as discussed earlier. Rejected because nothing would construct it once Enter plays. A case that is never constructed is dead code under `-D warnings`.

### D5. One row painter for both views

`render_open_playlist_row` is replaced by the list-row painter, generalised over its leading and trailing spans:
- List rows: bold title, then a muted `(count)`.
- Open rows: a muted `NN. ` prefix, then the title.

The bg/fg rule is the same for both:
- selected: `SELECTED_ROW_BG` + `SELECTED_ROW_FG`
- odd absolute index: `PLAYLIST_STRIPE_BG`
- otherwise: none

Rows are one line high, so the open view's variable-height machinery is deleted: the `item_lines` closure, the line-summing scroll loop, and the line-based scrollbar maths. The open view uses the same cursor/scroll clamp and the same `render_sidebar_scrollbar(len, scroll)` call as the list. `PlaylistsRenderGeometry::open_rows` stays, so mouse hit-testing keeps working, and each row is one line high.

## Risks / Trade-offs

- [Behaviour change: Enter now interrupts playback] → It is still gated by the replace-queue confirmation whenever the queue is non-empty. The hint has always said "play".
- [Enqueue onto a queue bound to another playlist makes that playlist dirty] → The user explicitly accepted this, because it is how playlists get merged.
- [The list-view Shuffle and Enqueue fetch synchronously on the UI thread] → This is the same as today's Play. Making the fetch asynchronous is out of scope (Non-Goals).
- [Spec wording: idle-load requirements lose their playlist example] → They are reworded around the attached Session and cast triggers that remain (see the delta specs). The behaviour of those paths is unchanged.
