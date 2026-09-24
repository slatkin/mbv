## Context

There are two queue-replacement executors (#754):
- `play_items_routed` handles route switching, unplayable deferral, and panel focus.
- `execute_queue_replacement` → `replace_queue_or_prompt` → `execute_pending_queue_action` handles direct-remote staging, `autostart:false`, and the dirty-playlist prompt.

The D6 gate (`request_queue_replacement`) sits in front of the second executor only, and only the grouped tree calls it.

## Goals / Non-Goals

**Goals:** one gate, used by every user-initiated replacement; executor behaviour unchanged after confirmation.
**Non-Goals:** merging the executors; gating autoplay, `play_item`, or session replays.

## Decisions

**D1 — The pending payload carries its executor.** Add
```rust
pub(super) enum ReplacementExecutor {
    Routed(RoutedReplacementPrep),
    Pending,
}
```
with `RoutedReplacementPrep { Album, MusicAlbums, Folder, ShuffleFolder, Selection }`, one variant per gated routed entry point (`Folder` is a unit variant; folder-play's `QueueSource::Collection{..}` rides on the action's `source`). Make the slot `pending_queue_replacement: Option<(PendingQueueAction, ReplacementExecutor)>`. A `Routed` payload is always a `PlayItems`. The executor is a type rather than a flag, so the confirm arm can't pick the wrong one.

**D2 — `request_queue_replacement(action, via)`.** It uses the same predicate and modal as today. On an empty queue or a confirm, it calls `run_replacement(action, via)`:
- `Routed(prep)`: call `run_routed_replacement(action, prep)`, which replays that entry point's pre-play prep and then calls `play_items_routed` (it never replaces the queue itself).
- `Pending`: call `execute_queue_replacement` (unchanged).

**D3 — Call-site mapping.**

| Entry point | via |
|---|---|
| `replace_and_route_album_queue` (album + artist tracks) | Routed |
| `shuffle_folder_actions.rs` (3 sites) | Routed |
| `context_menu_actions` PlaySelection / ShuffleSelection | Routed |
| `shell_playlists.rs` playlist load | Pending |
| `load_and_play_playlist` (`library_load_actions.rs`, `PlaylistsActivate{open:false}`) | Pending |
| `play_grouped_track` | Pending (unchanged) |

Untouched: `library_load_actions.rs` autoplay, `play_item`, `session_switch`, `run_loop_events_session`, `input_confirm_keys` discard arm, and `ClearQueue` callers.

**D4 — Side effects before the gate.** `context_menu_actions` calls `rebuild_queue_for_selection` before `play_items_routed`. That call must move after the gate, into the confirmed path, or cancelling would leave the queue mutated. Do the same for any pre-play mutation found at the shuffle sites or in `replace_and_route_album_queue`. If a site can't move its mutation cleanly, the implementer stops and reports; they don't improvise. The folder-play `QueueSource::Collection{..}` source set is likewise deferred into the confirmed path — it rides on the gated action's `source`, so a cancel must not leave the source label changed.

**D5 — Prompt order** follows from D2. The gate runs first. `defer_local_play` lives inside `play_items_routed`, and the dirty-playlist prompt lives inside `replace_queue_or_prompt`, so both fire only after the user confirms.

## Risks / Trade-offs

- Two executors remain, so future play features may still land on only one of them. Accepted; option (b) was rejected.
- Invariant 10: the slot keeps one writer and one reader, and only its payload type changes. Update the doc's scope line and signatures.
