Tasks are grouped as coherent ownership-boundary slices rather than one task per file. Each implementation task includes adjustment of directly affected existing tests; add a new focused regression test only where the required behavior has no effective coverage.

## 1. Canonical core queue

- [x] 1.1 Make `QueueItem` media-kind classification total: carry the subscription `FeedKind` fallback in queued Feed snapshots, use enclosure MIME only as a recognized refinement, preserve legacy deserialization, and remove contradictory Feed defaults.
- [x] 1.2 Complete the item-generic `PlaybackQueue<QueueItem>` operations needed for replace, append, remove, move, clear, play-slot selection, and slot-identity consumption; eliminate core helpers that project mixed queues to Emby-only collections where queue truth is required.

## 2. Shared Player submission and lifecycle

- [x] 2.1 Introduce one item-generic submission/control boundary across `Player` and `PlayerProxy` that distinguishes queue mutation from playing an existing slot and can start a cold owner or control an active owner without relying on a pre-existing run-command channel.
- [x] 2.2 Build every mpv run and playlist from the canonical queue, branching by `QueueItem` only for source URL resolution and reporting context; remove Feed-only headless, fake-client, reporter, and run-construction policy.
- [x] 2.3 Route bare local and stay-alive local playback through the shared boundary, surface unreachable/start failures through the existing notification path, and verify that playing an existing Feed slot neither duplicates it nor diverges queue status.

## 3. Unified ctrl and daemon authority

- [x] 3.1 Add the additive unified-queue ctrl capability plus item-generic queue state and operations, keeping the protocol version unchanged and isolating old Emby commands and decode-only `LoadFeed` translation in compatibility adapters.
- [x] 3.2 Replace daemon `items + feed_items` authority with one `PlaybackQueue<QueueItem>`; derive snapshots, reconnect state, cursor, queue length, mutation, admission, and mpv synchronization from that queue for arbitrary mixed ordering.
- [x] 3.3 Migrate `RemotePlayer` submission, queue updates, and capability negotiation to the unified ctrl path; report missing capability and closed-channel failures instead of dropping commands, and prevent legacy peers from overwriting canonical slots they cannot represent.

## 4. TUI and state-boundary migration

- [x] 4.1 Replace `PlayerTab.items + feed_items + queue` mirroring with one canonical queue plus presentation-only cursor/scope state; migrate rendering, title lookup, navigation, current-slot handling, and Player events to its coordinates, including `insert_item_at` and `move_slot` cursor clamps that currently use the Emby-only length.
- [x] 4.2 Send Feed and Emby Play/enqueue actions through the same route, submission-destination, admission, fall-through, and queue-operation paths; ensure mixed queues support ordinary append, move, remove, clear, and play-existing-slot behavior.
- [x] 4.3 Make save/restore, local-daemon bootstrap, shared state, and queue adoption carry the tagged canonical queue without projecting away Feed entries; specifically make `build_queue_state()` serialize the canonical queue rather than `player_tab.items`, while retaining reads of legacy untagged Emby queue state.

## 5. Remove the split model and verify the replacement

- [x] 5.1 Delete current-code emission and internal use of `play_feed`, `LoadFeed`, parallel `feed_items`, Feed-tail mutation guards, GUID-wide consumption, and synchronization/mirroring helpers; retain only explicitly documented wire-compatibility adapters and update nearby stale comments or domain documentation.
- [ ] 5.2 Verify the specification matrix for Emby and Feed items across bare local playback, cold stay-alive playback, warm playback, direct remote control, reconnect, mixed reordering/removal, duplicate slots, persistence, failure visibility, and audio-only admission/fall-through.
- [ ] 5.3 Run `rtk cargo check -p mbv-core`, `rtk cargo check`, `rtk cargo test -p mbv-core`, `rtk cargo clippy --workspace --all-targets`, and `rtk make check-code-file-lines`; resolve failures without restoring parallel queue state or Feed-specific lifecycle paths.
