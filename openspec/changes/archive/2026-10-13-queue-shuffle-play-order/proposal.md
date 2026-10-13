# Proposal

## Why

The user can shuffle a collection only by replacing the queue with a shuffled copy. There is no way to keep the current queue order and still hear its items in random order. A shuffle play order lets the user keep the queue as built and play it randomly.

## What Changes

- Add a playback order to the Player owner: Sequential (the current behavior) or Shuffle.
- In Shuffle, each advance picks a random slot from the whole queue, except the slot that plays now. Repeats can occur. There is no played set and no end of cycle. Playback continues until the user stops it, or until the queue has no other slot.
- One pick drives every advance: end of file, the Next command, and the Next Up card.
- In Shuffle, Previous is disabled. MPRIS reports it as unavailable.
- The queue order does not change. Shuffle changes only which slot plays next.
- A toggle entry in the Queue list context menu turns Shuffle on or off. It is offered on every Queue row and nowhere else.
- The footer shows a shuffle indicator while Shuffle is on.
- When the whole queue is replaced, the order goes back to Sequential. Appends, removals, and moves keep the order.
- MPRIS reports the real shuffle state (read-only).
- The order is set through a new ctrl command behind a new capability. No protocol version bump.

## Capabilities

### New Capabilities
- `queue-play-order`: the Player-owned playback order (Sequential or Shuffle), how it picks the next slot, how it affects Next, Previous, and Next Up, when it resets, and how the footer and MPRIS show it.

### Modified Capabilities
- `context-menu`: the rule that Shuffle appears only on collections and multi-selections gets an exception for the Queue list shuffle-order toggle, which is a playback order control, not a queue replacement.

## Impact

- `crates/mbv-ctrl`: new player command, new `PlayerStatus` field (`#[serde(default)]`), new capability string.
- `crates/mbv-player`: next-slot selection in owner state and the playback run (end of file, step, Next Up), Previous gating, reset on queue replacement.
- `src/app/dispatch/context_menu/`: Queue row menu entry and its action dispatch, including non-Emby Queue rows.
- `src/app/state/projection/chrome_status.rs`: footer shuffle indicator.
- `crates/mbv-desktop/src/mpris.rs`: `Shuffle` and `CanGoPrevious` from the status.
- `crates/mbvd`: the packaged daemon forwards the new command like the other player commands.
