## Why

Feed playback exposed that mbv does not have one playback queue model despite already defining `QueueItem`. The TUI, daemon, ctrl state, persistence, and Player each retain parallel Emby and Feed representations, while Feed playback uses a separate command and lifecycle path. Basic local playback, cold-daemon startup, queue selection, remote playback, mutation, and restoration have consequently failed or diverged in different combinations.

Further patches to Feed-specific paths would preserve the cause. The playback submission and owner-queue boundary must instead be rebuilt around one ordered queue and one set of operations for every `QueueItem`; item variants may differ only where their source is resolved and where progress is reported.

## What Changes

- Make one `PlaybackQueue<QueueItem>` the queue representation used by the TUI, Player owner, daemon state, ctrl synchronization, and persistence instead of mirroring `EmbyItem` and `FeedEntry` collections.
- Give every occurrence in a queue stable slot identity independent of the underlying Emby item ID or Feed identity, and target queue operations at that occurrence.
- Replace the Feed-only `play_feed` / `LoadFeed` path with item-generic append, replace, remove, move, and play-existing-slot operations. Playing an existing slot does not append another copy.
- Route local, stay-alive-daemon, and directly controlled remote playback through the same lifecycle-capable submission boundary.
- Apply owner admission, media-kind classification, queue consumption, cursor/status calculation, and playback failures uniformly to Emby and Feed items.
- Preserve the legitimate variants only at source resolution (authenticated Emby stream versus direct URL) and progress reporting (Emby reporting versus none).
- Add an additive ctrl capability for unified queue state and operations. Keep legacy wire behavior only as a compatibility adapter; do not extend the Feed-tail model.
- Remove the parallel Feed tail, its mutation restrictions, Feed-only lifecycle setup, and projections that silently discard Feed entries during persistence or synchronization.

## Capabilities

### New Capabilities

- `unified-playback-queue`: one authoritative, ordered, slot-identified queue and one item-generic submission/control model across local playback, Player owners, ctrl peers, and persistence.

### Modified Capabilities

- `feed-queue-item`: Feed entries participate in ordinary queue state and operations; the capability-gated `feed_items` tail and `LoadFeed` append-and-play behavior are removed in favor of the unified queue protocol.

## Impact

- **Core model:** `PlaybackQueue`, queue slot identity, queue mutation, media-kind accessors, consumption, and persistence in `mbv-core`.
- **Playback boundary:** `Player`, `PlayerProxy`, `RemotePlayer`, Player commands, mpv queue loading, lifecycle startup/reuse, and reporting setup.
- **Ctrl/daemon:** additive capability and unified queue wire shapes; daemon queue authority, state snapshots, reconnect behavior, cursor arithmetic, and compatibility translation.
- **TUI:** `PlayerTab` queue state, feed and library play/enqueue actions, queue rendering/navigation, queue scope, player events, bootstrap, and shared-state restoration.
- **Compatibility:** `CTRL_PROTOCOL_VERSION` remains unchanged. Mixed-version peers retain their existing Emby-only behavior; unified mixed queues require the new capability.
- **Documentation:** the obsolete Feed-tail decisions in the current `feed-queue-item` specification and feeds-tab design are superseded by this change.
- **No new dependencies.** Feed subscription management, feed parsing, watched/resume state, and unrelated Emby library behavior are outside this change.
