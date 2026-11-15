# Tasks

## 1. One row rule in mbv-ui-model

- [x] 1.1 In `crates/mbv-ui-model/src/media_list.rs`, gate `MediaSemanticState::from_progress` with `mbv_emby_model::should_resume` (design D1) and update its doc comment to say that a position Play would resume wins over played. Verify: `cargo check -p mbv-ui-model`.
- [x] 1.2 Add `MediaSemanticState::from_feed_entry(&FeedEntry)` and `MediaSemanticState::from_audiobookshelf(finished, position_ticks, duration_ticks)` per design D1, and route `from_queue_item` through `from_emby`, `from_feed_entry`, `from_audiobookshelf` (with `AudiobookshelfItem::played()` as finished), and `Ordinary` for Twitch, with no wildcard arm. Verify: `cargo check -p mbv-ui-model`.
- [x] 1.3 In `crates/mbv-ui-model/tests/media_list/`, add one named `#[case]` table for the contract "a row is Active exactly when Play would resume" (spec `playback-resume`, cite #1005), with only cases that differ in outcome: Emby played with a resumable position -> Active; Emby played below the gate -> Played; positive position with unknown runtime -> Active with no percentage; Audiobookshelf finished with a position -> Played; Audiobookshelf unfinished above the gate -> Active; feed entry played with a resumable position -> Active; unplayed below the gate -> Ordinary. Move existing assertions that pin "position above zero is Active" to the new outcome. Verify: `cargo nextest run -p mbv-ui-model`.
- [ ] 1.4 Add `docs/invariants/33-row-resume-state-matches-play.md` (design D6, follow the format of the other invariants: the invariant, why it matters, how the code keeps it, where it can fail) and add the one *Playback resume* sentence to `CONTEXT.md`. Verify: the file exists and names `MediaSemanticState::from_progress`, `should_resume`, `resume_source`, and the Audiobookshelf server's finished rule.

## 2. TUI destination rows use the rule

- [ ] 2.1 In `crates/mbv-components/src/podcast_content.rs`, replace the inline `semantic_state` match with `MediaSemanticState::from_audiobookshelf` over the episode's progress, converting seconds with `seconds_to_ticks`; a missing progress entry stays `Ordinary`. Update the podcast row tests in `crates/mbv-components/src/podcast_content/tests/` that pin played-wins. Verify: `cargo nextest run -p mbv-components podcast`.
- [ ] 2.2 In `crates/mbv-components/src/feeds_content.rs`, replace the inline `semantic_state` `if` with `MediaSemanticState::from_feed_entry`. Update the feeds row tests that pin played-wins. Verify: `cargo nextest run -p mbv-components feeds`.
- [ ] 2.3 Confirm with `rg` that no file under `crates/mbv-components/src` builds `MediaSemanticState::Played` or `MediaSemanticState::Active` from provider facts (the painters' matches in `mbv-render` stay). Verify: the `rg -n "MediaSemanticState::(Played|Active \{)" crates/mbv-components/src` output holds no row-builder site.

## 3. GUI Library projections use the rule

- [ ] 3.1 Change the row state fields of the GUI Library projections (`GuiHomeRow` and the Podcasts, Feeds, Shows, and entry-list rows under `src/app/gui_library/`) to `MediaSemanticState`, and build them with `from_queue_item`, `from_emby`, `from_feed_entry`, or `from_audiobookshelf` (design D2). Delete `GuiHomeRowState`, `GuiHomeRowState::from_progress`, and `media_row_state`, and remove `GuiHomeRowState` from the exports in `src/app/gui_library.rs`, `src/app.rs`, and `src/lib.rs`. Verify: `cargo check -p mbv`.
- [ ] 3.2 In `crates/mbv-gui/src/bridge/frame/library/` (home, entry_list, shows), implement `From<&MediaSemanticState> for GuiHomeRowStateFrame` with `NowPlaying` mapping to `Active` with its progress, and delete the `From<&GuiHomeRowState>` impl. The wire shape does not change. Verify: `cargo check -p mbv-gui` and `cargo nextest run -p mbv-gui`.
- [ ] 3.3 Update `src/app/tests/gui_library_{home,podcasts,feeds,shows}.rs` to assert `MediaSemanticState` values, moving each played-wins assertion to the new outcome. Add no new tests: the rule's contract is owned by task 1.3. Verify: `cargo nextest run -p mbv gui_library`.
- [ ] 3.4 Add the amendment section to `docs/adr/0036-gui-library-panel-owns-its-interaction-state.md` per design D6 and the rules in `docs/adr/README.md` (no status, no change path, no line numbers; point at `openspec/specs/playback-resume/spec.md`). Verify: the section exists and names no file line or PR.

## 4. One row-date helper

- [ ] 4.1 Add `row_date(secs: Option<u64>) -> Option<String>` to `crates/mbv-ui-model/src/ui_util.rs` (design D3) with one test for the contract "an unformattable timestamp yields no date" in `crates/mbv-ui-model/tests/`. Verify: `cargo nextest run -p mbv-ui-model`.
- [ ] 4.2 Replace every `fmt_publish_date_short` date-gutter chain in `crates/mbv-components/src` (podcast, feeds, Emby library, TV episode rows, Home flat) and `src/app/gui_library` (podcasts, feeds, home, shows, emby projections) with `row_date`. Hero dates (`fmt_publish_date`) do not change. Verify: `rg -n "map\(fmt_publish_date_short\)|map\(mbv_ui_model::ui_util::fmt_publish_date_short\)" src crates` returns no row-builder site, and `cargo nextest run -p mbv-components -p mbv` passes.

## 5. Feed store keeps played rows at position zero

- [x] 5.1 In `crates/mbv-feed/src/entry_state.rs`, make `FeedEntryStore::put` store position zero when `played` is true and update the `set_played` doc comment (design D4). Update the existing `set_played` test to the contract "Mark Played stores position zero; Mark Unplayed keeps the position" (spec `feed-queue-item`, cite #1005). Verify: `cargo nextest run -p mbv-feed`.
- [x] 5.2 Make `FeedEntryStore::load` set position zero on every played row with a positive position, save once when it changed a row, and log the count (design D4). Add one test for "an old played row with a position loads corrected and the file is rewritten" and one for "a clean file is not rewritten", using the crate's existing state-dir test isolation. Verify: `cargo nextest run -p mbv-feed`.

## 6. A Feeds mark updates the loaded rows

- [x] 6.1 After `set_feed_entries_played` (`src/app/dispatch/context_menu/actions.rs`) writes the store, apply each entry's stored state to the matching loaded entries in `feed_tab.entries` and `feed_tab.all_entries`, matched by `feed_id` and `guid` (design D5). Add one App test for the contract "a Feeds Mark Played shows played state with position zero in the loaded rows without a refetch" (spec `feed-queue-item`, cite #1005) beside the existing Feeds context-menu tests. Verify: `cargo nextest run -p mbv feeds`.

## 7. Integration checks

- [ ] 7.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`, and fix every failure. Verify: all three exit with status 0.
- [ ] 7.2 Run `openspec validate row-state-follows-playback-resume --strict`. Verify: it reports the change as valid.

## Workflow follow-up

- Run `make check-code-file-lines` just before the push.
- Sync the delta specs into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.
