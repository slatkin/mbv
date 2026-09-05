Sequencing follows `design.md` — Migration Plan: deletions, then identity, then
rejection, then lifecycle, then Consume. Groups 1–3 are one work unit; groups
4–6 are a second. Do not start a later group before the previous one's gate
passes.

Standing gate for every task: `cargo check -p mbv-core` (or `-p mbv` when the
edit is under `src/`), then `cargo nextest run -p <package>`, then
`cargo clippy --workspace --all-targets`, then `cargo fmt`. Accept all rustfmt
reflow. `crates/mbv-core/src/player.rs` `include!`s ten sibling files into one
module — an edit in `player_run_*.rs` is compiled as part of `player.rs`, so
`cargo check` failures point at `player.rs` line numbers, not the file you
edited.

## 1. Delete the queue-addressing wire surface

- [ ] 1.1 Remove `JumpTo`, `QueueAppend`, `QueueRemove`, `QueueMove`, `ReplaceQueue` and `LoadNew` from `WireCommand` in `crates/mbv-core/src/ctrl.rs`, and remove their arms from both `From<PlayerCommand> for WireCommand` and `From<WireCommand> for PlayerCommand`. Both conversions are exhaustive matches with no wildcard arm — verify by confirming the resulting compile errors name exactly the `PlayerCommand` variants that no longer have a wire mapping, and no others.
- [ ] 1.2 Make the six now-unmappable `PlayerCommand` variants in-process-only: `From<PlayerCommand> for WireCommand` must no longer accept them. Follow the existing precedent for `PlayerCommand::SubmitQueue` in `ctrl.rs` (an `unreachable!` arm with a comment saying the command is local-only). Verify `cargo check -p mbv-core` is clean.
- [ ] 1.3 Delete the `PlayerCommand::ReplaceQueue` and `PlayerCommand::QueueAppend` arms from the `CtrlCmd::PlayerCmd` match in `crates/mbv-core/src/daemon_control.rs` (near line 176). These are the arms that answer an append by re-submitting the whole queue as `SubmitQueue`. Verify by confirming `CtrlCmd::UnifiedQueueAppend` (same file, near line 391) is now the only append entry point.
- [ ] 1.4 Delete the wire round-trip assertions for the removed variants from `crates/mbv-core/src/ctrl_tests.rs` (`QueueAppend` near line 100, `ReplaceQueue` near line 153). Verify `cargo nextest run -p mbv-core` passes.
- [ ] 1.5 Confirm nothing outside tests still sends a removed variant over ctrl: `rg -n 'WireCommand::(JumpTo|QueueAppend|QueueRemove|QueueMove|ReplaceQueue|LoadNew)' crates/ src/` returns no non-test hits.

## 2. Collapse the near-end rule and remove the dead condition

- [ ] 2.1 In `crates/mbv-core/src/player_run_events.rs`, replace the inlined `self.last_valid_pos * 20 / runtime >= 19` at the quit path (near line 311) with a call to `is_near_end` (defined in `player_proxy.rs`, in scope via the `include!` chain — no `use` needed). Pass the completed occurrence's runtime, not `status.runtime_ticks`. Verify the existing `player_tests_*` suites still pass.
- [ ] 2.2 Same substitution for the shutdown path (near line 586). Note this site currently gates on `self.reporter.has_session()` where `is_near_end` gates on `!natural`; preserve the `has_session()` check as a separate condition at the call site rather than folding it into the helper. Verify with `cargo nextest run -p mbv-core`.
- [ ] 2.3 Remove the dead `let natural_end = reason == mpv_end_file_reason::Eof && runtime > 0;` at `player_run_events.rs:307` and simplify `(natural_end || near_end)` at line 319 to `near_end`. This binding sits inside a branch already gated on `reason == mpv_end_file_reason::Quit`, so it is unconditionally `false`. Verify clippy reports no new warnings and the near-end tests still pass.
- [ ] 2.4 Add a test asserting the near-end verdict is identical across the advance, quit and shutdown paths for one completed occurrence at one position (spec: "Same completion, different exit path"). Test the decision helper directly — do not simulate a full playback session.

## 3. Gate for groups 1–3

- [ ] 3.1 Full gate: `cargo nextest run -p mbv-core && cargo nextest run -p mbv && cargo clippy --workspace --all-targets && cargo fmt --all -- --check`. Groups 4–6 must not begin until this passes.

## 4. Thread slot identity across the owner boundary

- [ ] 4.1 Change `PlayerCommand::SubmitQueue` and `PlayerCommand::QueueAppend` in `crates/mbv-core/src/player_types.rs` to carry `Vec<(QueueSlotId, QueueItem)>` instead of `Vec<QueueItem>`. Verify the compile errors enumerate every construction site; that list is the work for 4.2–4.4.
- [ ] 4.2 Update the daemon's senders in `daemon_control.rs` and `daemon_reconciliation.rs` to pass the canonical queue's own `(slot_id, item)` pairs. `PlaybackQueue::slots()` already yields `QueueSlot { slot_id, item, .. }`; do not re-derive ids. Verify each call site reads its pairs from the same `PlaybackQueue` it just mutated.
- [ ] 4.3 In `crates/mbv-core/src/player_run_queue.rs`, construct the run's queue with `PlaybackQueue::from_slot_items(pairs, active_slot_id, revision)` instead of `from_queue_items`, in both `new_from_queue_items` and `init_from_queue`. Verify by asserting in a test that a `PlaybackRun` built from owner slot ids reports those same ids back, not ids starting at 1.
- [ ] 4.4 Update `cmd_append_queue` and `append_items_to_queue` in `player_run_commands.rs` to insert at the supplied slot ids. `PlaybackQueue::append` allocates a new id — add an id-preserving insert alongside it rather than changing `append`'s signature (`append` has other callers that legitimately allocate).
- [ ] 4.5 Change `PlayerCommand::JumpTo`, `QueueRemove` and `QueueMove` to take `QueueSlotId` (`QueueMove` becomes `(QueueSlotId, usize)` — the destination stays an index because it names a position, not an occurrence). Update `player_proxy.rs`'s `next()` / `previous()` to resolve their target to a slot id before sending. Verify `cargo check -p mbv-core`.
- [ ] 4.6 Change `PlayerEvent::TrackChanged`, `TrackCompleted.idx` and `Stopped.idx` in `player_types.rs` to carry `QueueSlotId`. Note the existing comment in `player_run_events.rs` claiming "PlayerEvent indices remain local UI snapshots" — that rationale is what this change reverses; delete the comment rather than leaving it contradicting the code.
- [ ] 4.7 Update the emit sites in `player_run_events.rs` to send `self.active_slot_id()` / `completed_slot_id` directly instead of resolving them to an index first. Verify the `unwrap_or(self.current_idx)` fallbacks are gone, not merely relocated.
- [ ] 4.8 Update `src/app/player_event.rs` to consume slot ids: the `resolve_slot_at(idx)` calls in the `TrackCompleted` and `TrackChanged` arms become direct slot use. Keep `queue_cursor` derivation working by resolving the slot to an index locally after the queue has settled — that index is presentation-only and stays inside this file.

## 5. Reject stale addressing

- [ ] 5.1 In `crates/mbv-core/src/daemon_run.rs`, delete the `idx.min(queue.len() - 1)` clamp in the `TrackChanged` handler (near line 334) and its comment. Resolve the incoming slot id against the canonical queue; when it is absent, leave the active slot unchanged and skip the broadcast. Verify with a test that a `TrackChanged` for a removed slot changes neither the active slot nor the broadcast queue state.
- [ ] 5.2 Make every `QueueMutationResult::NotFound` from a Client-initiated mutation in `daemon_control.rs` reach the existing `reject_command` path rather than being discarded via `let _ =`. `rg -n 'let _ = queue\.|let _ = self\.queue\.' crates/mbv-core/src/` lists the sites. Verify a rejected mutation produces a `CommandRejected` event.
- [ ] 5.3 Confirm reports originating from the owner's own Playback run are discarded silently (no `CommandRejected`), while Client-initiated mutations surface. Verify by test that a stale `TrackChanged` emits no toast-bearing event.
- [ ] 5.4 Add tests for the two "Stale slot addressing is rejected" scenarios: a mutation crossing a removal in flight, and a report naming a slot the owner no longer holds. Assert no neighbouring slot is touched in either case.

## 6. Collapse the queue-start lifecycle

- [ ] 6.1 Extract `begin_queue(&mut self, pairs, start_idx)` in `player_run_commands.rs` covering the per-item reset shared by `cmd_submit_queue`, `replace_with_queue_items` and `accept_stopped_replacement`: `begin_item_lifecycle()`, `stop_report`, `load_state`, `pending_initial_playlist_layout`, the `status` projection, and the reporter restart. Leave `cmd_load_new` alone — it is a different lifecycle (Standalone, caller-supplied URL), not a fourth copy.
- [ ] 6.2 Delete `cmd_replace_queue` (near line 211) and `PlayerCommand::ReplaceQueue`. Verify no caller remains: `rg -n 'ReplaceQueue' crates/ src/` returns only archived-spec text.
- [ ] 6.3 Verify the three remaining start paths set `stop_report` and `load_state` through `begin_queue` only — grep each field name in `player_run_commands.rs` and confirm the only writes outside `begin_queue` are `cmd_load_new`'s.

## 7. Move Consume to the queue owner

- [ ] 7.1 Carry the consume policy (`consume_videos` / `consume_audio`) with queue submission so an owner can decide consumption without a Client attached. Verify a daemon owner holds the policy after `SubmitQueue` with no Client connected.
- [ ] 7.2 Add a `PlayerEvent::TrackCompleted` handler in the daemon event loop (`daemon_run.rs`) that applies the consume policy to the canonical queue by slot id and broadcasts the shortened queue. Verify the "Out-of-process owner consumes a completed slot" scenario.
- [ ] 7.3 In `src/app/player_event.rs`, stop mutating the queue on `TrackCompleted` when the owner is out of process (`has_direct_remote_queue()`); keep the UI reactions (`on_audio_consumed` / `on_video_consumed`, feed lifecycle persistence, toasts). Verify the in-process path still consumes exactly one slot.
- [ ] 7.4 Delete `pending_queue_removal` and its `TrackChanged` drain in `src/app/player_event.rs`. It exists only to defer a removal until the completed *index* still resolved; with slot identity the removal is immediate. Verify by confirming no field or call site named `pending_queue_removal` remains.
- [ ] 7.5 Add a test for "Completion arrives with no Client attached": a slot completes on an owner with no Client, the canonical queue is shortened, and a Client attaching afterwards observes the shortened queue.

## 8. Final gate

- [ ] 8.1 `cargo nextest run --workspace && cargo clippy --workspace --all-targets && cargo fmt --all -- --check && ast-grep scan && make check-code-file-lines`.
- [ ] 8.2 Confirm `CTRL_PROTOCOL_VERSION` is unchanged and no new ctrl capability string was introduced: `git diff` on `crates/mbv-core/src/ctrl.rs` shows removals of `WireCommand` variants only, no version or capability edits.
- [ ] 8.3 Manual check across the three owners (Bare, Local daemon, packaged `mbvd`): start a multi-item queue, remove a slot while the last track is finishing, and confirm the correct next track plays in all three. This is the behaviour the index clamp was masking; it is the one thing the unit tests cannot fully establish.
