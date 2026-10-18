# Proposal

## Why

The Queue list offers a "Shuffle Play" entry that turns on a persistent owner-side playback
order. That entry does two things the user does not want: it names the action oddly, and it
turns shuffle into a second playback mode (a random next slot on every advance, Previous
disabled, a footer indicator, an MPRIS shuffle flag) that the queue's own order never reflects.

The user wants one simple action: shuffle the queue's contents, once. If an item is playing, it
must stay first so playback is not interrupted; everything under it is reordered at random.

## What Changes

- **BREAKING** Remove the owner's playback order (Sequential/Shuffle) entirely: the
  `PlayOrder`/`PlayOrderMode` types, the owner-side next-slot pick, `PlayerStatus.play_order`,
  `CtrlCmd::SetPlayOrder`, the `play-order` ctrl capability, the footer shuffle indicator, and
  MPRIS shuffle reporting. Previous is no longer disabled by shuffle.
- The Queue list row menu offers one entry labelled **Shuffle**, on every Queue row, in place of
  the "Shuffle Play" / "Stop Shuffle" toggle.
- Choosing Shuffle reorders the queue's slots at random as one queue edit. The slot that plays
  now moves to the front and keeps playing; the remaining slots are shuffled underneath it. When
  no slot is playing, the whole queue is shuffled.
- The reorder reaches every Client in one snapshot, and reaches the playback run (and mpv's
  playlist) as one reorder, so natural advance, Next, Previous, and the Next-Up card all follow
  the new order.
- The reorder is a real queue edit, so it persists with the queue.
- The ctrl command behind the entry is new and behind a new capability. No protocol version bump.

## Capabilities

### New Capabilities
- `queue-content-shuffle`: the Queue list Shuffle entry, the random reordering of the queue's
  slots with the playing slot kept first, its atomicity, and the ctrl command and capability
  that carry it.

### Modified Capabilities
- `context-menu`: the rule that Shuffle appears only on collections and multi-selections now
  points at the new Queue-list Shuffle entry (capability `queue-content-shuffle`), which
  reorders the existing queue instead of toggling a playback order.
- `owner-playback-output`: the two requirements that name a shuffle pick (the Emby Session next
  item, and the owner exit hand-over) lose their Shuffle scenarios, because the owner no longer
  has a separate play order.

### Removed Capabilities
- `queue-play-order`: every requirement is removed. There is no owner-side playback order, no
  next-slot pick, and no current-order reset. The Queue list Shuffle entry no longer belongs to
  this capability. Its emptied spec directory goes away with the archive.

## Impact

- `crates/mbv-queue`: delete `play_order.rs` and its re-exports.
- `crates/mbv-player`: `owner_state.rs` loses the order field, its hooks, and the random pick;
  the run loses the `upcoming` advance and reverts to `current_idx + 1`; `proxy.rs` swaps
  `supports_play_order`/`set_play_order` for `supports_queue_shuffle`/`shuffle_queue`.
- `crates/mbv-ctrl`: delete `PlayerStatus.play_order` and `PlayerCommand::SetUpcoming`; replace
  `CtrlCmd::SetPlayOrder` with `CtrlCmd::UnifiedQueueShuffle`; replace the `play-order`
  capability with `queue-shuffle` in `hello.rs`.
- `crates/mbv-daemon`: delete `play_order.rs` and every order hook; add the shuffle handler and
  one `QueueReorder` command that carries the new order to the playback run.
- `crates/mbv-desktop/src/mpris.rs`: `Shuffle` reports false and `CanGoPrevious` true, with the
  helpers and their tests removed.
- `src/app/`: replace the context-menu action and label, drop `QueueView.play_order`, and drop
  the footer shuffle indicator.
- `openspec/specs/`: `queue-play-order/` is removed; `context-menu/` and `owner-playback-output/`
  take delta edits.
