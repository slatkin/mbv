# Tasks

## 1. Shared types and ctrl wire

- [x] 1.1 In `crates/mbv-queue`, add `PlayOrderMode { Sequential, Shuffle }` (serde, `Default = Sequential`) and the owner-side `PlayOrder { Sequential, Shuffle { upcoming: Option<QueueSlotId> } }` with a `mode()` accessor (design D1). Verify: `cargo check -p mbv-queue`.
- [x] 1.2 In `crates/mbv-ctrl`, add `CtrlCmd::SetPlayOrder(PlayOrderMode)`, the `CTRL_CAP_PLAY_ORDER` capability (advertised in `CtrlHello::current()`), and `#[serde(default)] play_order: PlayOrderMode` on `UnifiedQueueStateData`. Do not change `CTRL_PROTOCOL_VERSION` (design D5). Contract test in the crate's `tests/` binary: a `UnifiedQueueStateData` JSON without `play_order` decodes as Sequential. Verify: `cargo nextest run -p mbv-ctrl`.

## 2. Owner core picks the next slot

- [ ] 2.1 In `crates/mbv-player`, add the pure pick function (candidate slots, active slot, `&mut impl rand::Rng`) → `Option<QueueSlotId>` that never returns the active slot and returns `None` when no other slot exists (design D2). Tests with a seeded RNG: two cases in one `#[case]` table — one-slot queue gives `None`; multi-slot queue never gives the active slot. Verify: `cargo nextest run -p mbv-player`.
- [ ] 2.2 Add `play_order: PlayOrder` to `PlayerOwnerState` with a transition `set_play_order(PlayOrderMode)` and the re-pick hooks: on Shuffle on, in `note_observed_active_slot`, after a remove that drops `upcoming`, and after an append when `upcoming` is `None` (design D2). Reset to Sequential where the core installs a new `QueueLineage` (design D4). Tests in `owner_state.rs`: replacement resets to Sequential; removing `upcoming` re-picks a present non-active slot. Verify: `cargo nextest run -p mbv-player`.
- [ ] 2.3 Change `relative_step_target`: in Shuffle, `Next` returns `Jump(upcoming)` or `AtEdge` when `None`; `Previous` returns `AtEdge` (design D3). Extend the existing step test with one Shuffle Next case and one Shuffle Previous case. Verify: `cargo nextest run -p mbv-player`.

## 3. Playback run obeys the pick

- [ ] 3.1 Add the in-process `PlayerCommand::SetUpcoming(Advance)` with `Advance { Sequential, To(QueueSlotId), End }` (never on the ctrl wire, like `JumpTo`). Handle it in `crates/mbv-player/src/run/commands.rs` by storing the value on `PlaybackRun` (design D2). Verify: `cargo check -p mbv-player`.
- [ ] 3.2 In `settle_forced_jump` (`run/events/queue_advance.rs`), with no forced jump: `To(slot)` → that slot's index (fall back to `current_idx + 1` if the slot is gone); `End` → `queue_len()` so `stop_at_queue_end` runs; `Sequential` → `current_idx + 1`. Unit-test the index choice as a pure helper with one `#[case]` table of the three variants. Verify: `cargo nextest run -p mbv-player`.
- [ ] 3.3 Change `queue_next_up_decision` (`run/decisions.rs`) to take the next index (or none) as input instead of `current_idx + 1`; the caller in `run/events/track_progress.rs` passes the index derived from the stored `Advance`. Update the existing `tests/decisions.rs` cases to the new signature; add one case where the next index is not `current_idx + 1`. Verify: `cargo nextest run -p mbv-player`.

## 4. Daemon wiring

- [ ] 4.1 In `crates/mbv-daemon`, handle `CtrlCmd::SetPlayOrder` by calling `set_play_order`, and send `PlayerCommand::SetUpcoming` to the Playback run every time the core's pick or order changes (Shuffle on/off, re-pick, lineage reset). Project `play_order.mode()` into `UnifiedQueueStateData`. Give the new command an exhaustive dispatch arm in every match over `CtrlCmd` (including `crates/mbvd`). Test in `crates/mbv-daemon/src/tests/queue_ops/`: `SetPlayOrder(Shuffle)` sends `SetUpcoming(To(slot))` with a non-active slot, and the next snapshot reports Shuffle. Verify: `cargo nextest run -p mbv-daemon`.

## 5. TUI: menu, footer, MPRIS

- [ ] 5.1 Project `play_order` from the queue state into the shell (next to `source`), and add a `PlayerProxy` method that sends `CtrlCmd::SetPlayOrder` only when the owner advertises `CTRL_CAP_PLAY_ORDER`. Verify: `cargo check -p mbv`.
- [ ] 5.2 Add `ContextAction::SetPlayOrder(PlayOrderMode)` in `crates/mbv-ui-model/src/context_menu.rs` with an exhaustive dispatch arm in `src/app/dispatch/context_menu/actions.rs`. In `menu.rs`, let every Queue row open a menu: Emby rows keep their entries and append the toggle last; Feed and Audiobookshelf Queue rows show only the toggle. Build the action with the opposite of the current mode; hide the entry when the capability is missing (design D6). Extend `src/app/tests/context_menu_entries.rs`: an Emby Queue row ends with the toggle; a Feed Queue row offers only the toggle; a library row has no toggle. Verify: `cargo nextest run -p mbv context_menu`.
- [ ] 5.3 In `src/app/state/projection/chrome_status.rs`, show the shuffle indicator while `play_order` is Shuffle, independent of Queue focus (design D7). One projection test: Shuffle shows the indicator with the Library panel focused. Verify: `cargo nextest run -p mbv chrome_status`.
- [ ] 5.4 In `crates/mbv-desktop/src/mpris.rs`, feed the play order into the MPRIS snapshot; `shuffle()` returns true in Shuffle and `can_go_previous()` returns false in Shuffle. Delete the two now-unfulfilled `#[expect(clippy::unused_self)]` attributes. Verify: `cargo nextest run -p mbv-desktop`.

## 6. Integration checks

- [ ] 6.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. All pass.
- [ ] 6.2 Sync the spec deltas into `openspec/specs/` (`queue-play-order` new, `context-menu` modified). Verify: `openspec validate queue-shuffle-play-order`.
