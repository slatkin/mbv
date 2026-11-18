# Tasks

Every task: run `cargo fmt` after each Rust change. Never run `cargo nextest`, `cargo test`, or `cargo clippy` in the background; run them in the foreground and wait. The visual reference is `assets/queue-panel-redesign.pdf`.

## 1. Tokens and the queue module split (design D1, D10)

- [ ] 1.1 Add the `--ef-*` token block from design D1 to `crates/mbv-gui/assets/app.css` and regenerate `assets/app.generated.css` with `make gui-css`. Verify `cargo check -p mbv-gui`.
- [ ] 1.2 Move the row-size constants and the row-geometry helpers out of `crates/mbv-gui/src/window/components/queue_panel.rs` into a new child module `queue_panel/geometry.rs` with no behavior change (design D10). Verify `cargo nextest run -p mbv-gui` passes and `queue_panel.rs` is well under 800 lines.

## 2. Buffered position on the ctrl wire (design D6, spec `ctrl-protocol`)

- [ ] 2.1 Follow `docs/recipes.md` "Add a ctrl protocol variant", steps 1–4 and 8: add `#[serde(default)] pub buffered_ticks: i64` to `PlayerStatus` (`crates/mbv-ctrl/src/player.rs`) with a doc comment (0 = unknown); add `CTRL_CAP_BUFFERED_POSITION = "buffered-position"` in `protocol.rs`, re-export it, advertise it from `CtrlHello`, and add `supports_buffered_position`; add a case to `crates/mbv-ctrl/tests/ctrl/handshake.rs` and a round-trip case (field present and field absent → 0) to `wire.rs`; regenerate the `protocol/ctrl/v10/` fixtures with `MBV_PROTOCOL_BLESS=1 cargo nextest run -p mbv-ctrl conformance`. Verify `cargo nextest run -p mbv-ctrl`.
- [ ] 2.2 Set the capability in `DaemonHelloCaps` (`crates/mbv-daemon/src/core_ctrl_spawn.rs`) and check the `CtrlEvent` decode in `crates/mbvd/src/main.rs` still matches. Fix every `PlayerStatus` literal the field breaks. Verify `cargo nextest run -p mbv-daemon -p mbvd`.
- [ ] 2.3 In `mbv-player`, observe `demuxer-cache-time` (`Format::Double`) on the next free observe id in `runtime.rs`, store `seconds_to_ticks(value)` in `status.buffered_ticks` in the run loop's property handler, and reset it to 0 where a new file resets `position_ticks`. Use no live mpv in any test. Verify `cargo nextest run -p mbv-player`.
- [ ] 2.4 Add `buffered_ticks: i64` to `mbv_ui_model::playback::PlaybackState`; set it from the local `PlayerStatus` in the displayed-state builders (`src/app/state/playback_target.rs` and `playback_target/local.rs`) and 0 for Cast, Emby Session, and peer paths with no field. Verify `cargo check --workspace --all-targets` and `cargo nextest run -p mbv`.

## 3. Delete the dead scrubber data (design D5)

- [ ] 3.1 Remove `chapters`, `intro_mark`, `credits_mark`, and `trickplay` from `QueuePlaybackProjection` and `GuiQueuePlayback`, and `chapter`, `chapter_number`, and `tile` from `GuiScrubberHover` (`bridge/frame/playback.rs`, `bridge/frame.rs`, `window/sections.rs`), with `GuiChapter`, `GuiSegmentMark`, `GuiScrubberTile`, and their tests. Remove the chapter ticks, the intro and credits blocks, the hover line, the trickplay tile, and the chapter label from `scrubber.rs` so it compiles. Verify `cargo check -p mbv-gui --all-targets`.
- [ ] 3.2 Delete `src/app/playback_trickplay.rs` and its tests, the `trickplay_sheets*` fields in `app_struct.rs` and `construct.rs`, `drain_trickplay_sheet_events` in `src/app/client_core.rs`, the sheet-bytes answer in `crates/mbv-gui/src/core_thread/tick_host.rs`, and the re-exports in `src/app.rs` and `src/lib.rs`. Verify `cargo check --workspace --all-targets`.
- [ ] 3.3 Delete `src/app/playback_metadata.rs` and its tests, `playback_metadata_cache` and its fetch call sites, `crates/mbv-emby/src/client_item_metadata.rs` with its tests in `crates/mbv-emby/tests/emby/client.rs`, and `EmbyChapter`, `EmbyMediaSegment`, and `EmbyTrickplayGrid` in `mbv-emby-model` (and any channel in `src/app/state/runtime_channels.rs` that only carried them). Verify `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` show no dead code.

## 4. Player bottom layer and seekbar (design D2, D3, D4, D6; spec `gui-client`)

- [ ] 4.1 Reshape `ControlRow` (`queue_playback_panel/art_stage.rs`) into the two rows of design D3: the seek row (whole row is `data-target="seekbar"`, 14 px × scale) and the transport row (play/pause 44 px round, previous, −10, +30, next at the left; elapsed and `−remaining` at the right). Remove `Scrubber`'s `center` prop and clock line. Set `BOTTOM_FULL_HEIGHT_PX` to `104.0` and paint the bottom layer's gradient scrim. Update the `player_scale` `#[case]` values in `art_stage/tests.rs` to the new constant. Verify `cargo nextest run -p mbv-gui art_stage`.
- [ ] 4.2 Add `buffered_ticks` to `GuiPlayhead` and a `buffered_fraction` to the window's `Playhead` (`crates/mbv-gui/src/window/playhead.rs`). Paint the seekbar per design D4: rest and hover track, buffered range only when ahead of the played fraction, played fill, thumb with its ring, and the time tip from `hover.time_label` while the `hover` section is `Some`. Extend `scrubber/tests.rs` with one test per contract, citing this change: (a) the time tip paints with no frame and no chapter while hovered; (b) the buffered range paints only when the buffered fraction is ahead of the played fraction; (c) no tick or segment element paints for a frame whose item had chapters. Verify `cargo nextest run -p mbv-gui scrubber`.
- [ ] 4.3 Paint the panel layering of design D2: stage on `--ef-player`, title block on `--ef-bg0` with its padding, title in `--ef-fg`, parent line in `--ef-red`, and the 1 px `--ef-bg3` rule. Update the paint tests in `crates/mbv-gui/src/window/app/tests/art.rs` and `paint.rs` that assert the old arrangement, and extend one existing arrangement test to assert the transport row paints under the seek row with the clocks after the next button. Verify `cargo nextest run -p mbv-gui`.

## 5. Queue header and chooser (design D7, D8; spec `gui-queue-chooser`)

- [ ] 5.1 Add `shown_len: usize` to `GuiQueueHeaderFacts` from the shown queue view's `total_queue_len()`, and `len: usize` to `QueueChooserEntryProjection` and `GuiQueueChooserEntry`, set in `App::queue_chooser_entries` per design D8. Extend `src/app/input/gui/tests/queue_chooser.rs` `the_chooser_projects_its_entries_and_rename_draft` (or one new test beside it) to assert the counts for the current, a resting, and the peer entry, and one test that the current queue's count follows a removal. Delete `GuiQueueHeaderFacts.entries` if nothing reads it after 5.2. Verify `cargo nextest run -p mbv queue_chooser`.
- [ ] 5.2 Rewrite `queue_header.rs` per design D7: the `QUEUE` label, the always-present switcher with the count chip and the chevron (up while the chooser overlay is open), new-queue and close beside it, the spacer, and the filter at the trailing edge; 48 px bar; button states from design D7. Keep every region handle and title. Set `--canvas-queue-header-height` to 48 px. Update the header paint tests (`queue_panel/tests.rs` or `queue_panel/tests/harness.rs` as they fail) and add one test, citing this change, that a one-entry list paints the switcher with the count and the controls in design D7 order. Verify `cargo nextest run -p mbv-gui`.
- [ ] 5.3 Restyle `queue_chooser_menu.rs` per design D8 and paint each entry's count at its right after its mark. Add one paint test, citing this change, that each entry paints its count. Verify `cargo nextest run -p mbv-gui queue_chooser`.

## 6. Queue rows (design D9; spec `gui-client` "The queue shows a row menu and drag reorder")

- [ ] 6.1 Set the row height to 34 px and the stride to 36 px in `queue_panel/geometry.rs`, and replace every literal `30px` row size in `queue_panel.rs` (rows, spacer, ghost, actions button, grip offset, insertion math) with the constants. Verify `cargo nextest run -p mbv-gui queue_panel`.
- [ ] 6.2 Move the row progress bar in `queue_panel/row_cells.rs` to an absolute bar inside the row (2 px, bottom 4 px, inset 54 px left and 12 px right), and paint the row states of design D9 (now-playing, active and marked, hover, watched). Add one paint test, citing this change, that a partial row and an unwatched row paint the same height. Verify `cargo nextest run -p mbv-gui`.
- [ ] 6.3 Delete the zebra stripe: `GuiQueueRow::Item::striped`, its computation in `crates/mbv-gui/src/bridge/frame/queue.rs`, `ItemRow::striped`, and the stripe arm of `row_background`, with the tests that assert stripes. Verify `cargo check --workspace --all-targets` and `cargo nextest run -p mbv-gui`.
- [ ] 6.4 Point the footer band's fill at `--ef-bg0` and its top border at `--ef-bg3` (`queue_footer.rs`); delete every `--canvas-*` token in `app.css` that no component reads any more (check with `rg -n "canvas-<name>" crates/mbv-gui/src`), and regenerate with `make gui-css`. Verify `cargo nextest run -p mbv-gui`.

## 7. Documentation and gates

- [ ] 7.1 Update `docs/architecture/gui-client.md`: the two-row bottom layer and its 104 px full height, the tick-free seekbar and its time tip, the buffered position path (mpv → `PlayerStatus` → `PlaybackState` → `GuiPlayhead`), the removal of the chapter, segment, and trickplay fetches, the `--ef-*` token set, the queue header order, the chooser counts, and the 34 px rows. Verify that `rg -n "trickplay|chapter tick|zebra" docs/architecture/gui-client.md` returns no stale paragraph.
- [ ] 7.2 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`; all pass.

## Workflow follow-up

- Sync the delta specs and archive the change only when the user approves the PR for merge, as the last commit before the merge.
