# rstest adoption candidate ledger

Read-only inventory after `add-rstest-for-table-tests`. This ledger is the change boundary: a worker MUST reconfirm the old test names and uniform assertion shape in the live file; candidates that have drifted are deferred and recorded rather than replaced.

## Named `#[case]` families

| File | Families (old tests / count) |
|---|---|
| `crates/mbv-core/src/ws.rs` | `play_empty_item_ids`, `playstate_unknown_command`, `general_command_unknown`, `unknown_message_type`, `malformed_json`, `missing_message_type` / 6 — `parse_msg(..).is_none()` |
| `crates/mbv-core/src/api_tests_parsing.rs` | `playback_label_*` / 2; `parse_item_{artist,album_artist}*` / 3; `parse_audio_info_{multiple_tracks,unknown_lang,skips_non_audio_streams}` / 3; `should_resume_*` / 7 |
| `crates/mbv-core/src/player_tests_status.rs` | `next_idx_*` / 3; `previous_idx_*` / 3 |
| `crates/mbv-core/src/player_tests_session_feed.rs` | `feed_queue_item_*` / 3 |
| `crates/mbv-core/src/playback_queue_tests_feed.rs` | `feed_entry_primary_source_*` / 3; `feed_media_kind_*` / 3 |
| `crates/mbv-core/src/playback_queue_tests_persistence.rs` | `queue_item_deserializes_{tagged,legacy_bare_emby_item,tagged_feed}` / 3 |
| `src/app/components/help.rs` | key-to-`Msg` (`esc`, `f1`–`f4`, `quit`, `unbound`, `ctrl_q`) / 8; scroll (`up`, `down`, `page`, `home`, `saturate`) / 6 |
| `src/app/tests_lifecycle.rs` | `transport_{prev,next}_*` / 5; `render_interval_*` / 2 |
| `src/app/tests_queue_reorder.rs` | `move_queue_item_*` / 4 |
| `src/app/actions_tests_letter.rs` | `should_show_letter_pills_true_for_*_tvshows_total` / 2 |
| `src/app/components/daemon_lost.rs` | `set_content_*` / 2 |
| `src/app/actions_tests_queue.rs` | `remote_seek_*` / 3 |
| `src/app/components/search_sidebar.rs` | `esc_emits_dismiss_search`, `ctrl_key_is_swallowed`, `alt_key_is_swallowed`, `unbound_key_is_swallowed` / 4 |
| `src/app/components/feeds_component_tests.rs` | watched/unwatched filter / 2 |

Every table uses `#[rstest]` plus `#[case::<old_test_name>]`; its case arguments carry every varied input and expected output. `api_tests_parsing.rs` retains its existing `json!` inputs, including where a JSON value is passed as case data.

## Eligible `#[fixture]` helpers

| File | Helper | Same-file consumers |
|---|---|---|
| `crates/mbv-core/src/player_tests_session_feed.rs` | `make_feed_session()` | 6 (`feed_session_*`, `feed_cancel_pending_quit_clears_state`) |
| same | `make_no_session_reporter()` | 2 (`reporter_session_lifecycle`, `reporter_no_session_all_reporting_is_noop`) |
| same | `make_no_session_reporter_with_ids()` | 2 (`reporter_session_lifecycle`, `reporter_with_session_stopped_proceeds_to_client`) |
| `src/app/actions_tests_queue_state.rs` | `make_queue_items(3)` | 5 `queue_restore_cursor_*` consumers |
| `src/app/tests_podcast_playback.rs` | `make_socket_merge_ready_app()` | 4 `socket_progress_*` consumers |
| `src/app/tests_feed_group_loading.rs` | `make_home_video_app()` | 3 `feed_home_video_*` consumers |
| `src/app/tests_queue_reorder.rs` | `make_feed_entry("f1")` | 3 consumers |

Fixtures keep meaningful varying arguments explicit. Cross-file support, one-consumer helpers, and helpers whose argument variations are the scenario remain helpers.

## Deferred categories

Do not convert families with different assertion verbs/sets, materially different setup or match arms, already aggregated multi-assertion coverage, or helper arguments that express the test scenario. In particular, do not introduce a cross-file fixture registry, `#[values]`, `#[files]`, timeout/async rstest features, or a broad helper conversion.
