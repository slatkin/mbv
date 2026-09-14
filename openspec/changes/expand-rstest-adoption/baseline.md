# rstest adoption baseline

Captured from clean HEAD `34b7561e` before source edits on 2026-09-14.

## Package totals

| Package | `cargo nextest list` test count |
|---|---:|
| `mbv-core` | 513 |
| `mbv` | 1441 |

Commands used: `cargo nextest list -p mbv-core | wc -l` and `cargo nextest list -p mbv | wc -l` (the preceding `cargo nextest list` output contained only test names).

## Selected case candidates for this unit

All ledger names below resolved in the live source before edits; no candidates were deferred.

| File | Family | Old tests / count |
|---|---|---:|
| `crates/mbv-core/src/ws.rs` | `parse_msg(..).is_none()` | `play_empty_item_ids_returns_none`, `playstate_unknown_command_returns_none`, `general_command_unknown_returns_none`, `unknown_message_type_returns_none`, `malformed_json_returns_none`, `missing_message_type_returns_none` / 6 |
| `crates/mbv-core/src/api_tests_parsing.rs` | `playback_label_*` | `playback_label_audio_without_artist_falls_back_to_display_name`, `playback_label_video_uses_display_name` / 2 |
| `crates/mbv-core/src/api_tests_parsing.rs` | `parse_item_{artist,album_artist}*` | `parse_item_artist_from_album_artist_field`, `parse_item_artist_falls_back_to_artists_array`, `parse_item_album_artist_takes_priority_over_artists_array` / 3 |
| `crates/mbv-core/src/api_tests_parsing.rs` | `parse_audio_info_*` | `parse_audio_info_multiple_tracks`, `parse_audio_info_unknown_lang_omitted_from_label`, `parse_audio_info_skips_non_audio_streams` / 3 |
| `crates/mbv-core/src/api_tests_parsing.rs` | `should_resume_*` | `should_resume_zero_position_returns_false`, `should_resume_negative_position_returns_false`, `should_resume_mid_way_returns_true`, `should_resume_under_six_percent_returns_false`, `should_resume_exactly_six_percent_returns_true`, `should_resume_just_below_six_percent_returns_false`, `should_resume_with_unknown_runtime_returns_true` / 7 |
| `crates/mbv-core/src/player_tests_status.rs` | `next_idx_*` | `next_idx_advances_when_room`, `next_idx_none_at_end_of_queue`, `next_idx_none_when_inactive` / 3 |
| `crates/mbv-core/src/player_tests_status.rs` | `previous_idx_*` | `previous_idx_none_at_start`, `previous_idx_steps_back`, `previous_idx_none_when_inactive` / 3 |

**Unit parity:** 6 + (2 + 3 + 3 + 7) + (3 + 3) = 27 old tests selected for conversion.

## Full ledger fixture-consumer mappings

| File | Helper | Same-file consumers |
|---|---|---|
| `crates/mbv-core/src/player_tests_session_feed.rs` | `make_feed_session()` | 6 (`feed_session_*`, `feed_cancel_pending_quit_clears_state`) |
| `crates/mbv-core/src/player_tests_session_feed.rs` | `make_no_session_reporter()` | 2 (`reporter_session_lifecycle`, `reporter_no_session_all_reporting_is_noop`) |
| `crates/mbv-core/src/player_tests_session_feed.rs` | `make_no_session_reporter_with_ids()` | 2 (`reporter_session_lifecycle`, `reporter_with_session_stopped_proceeds_to_client`) |
| `src/app/actions_tests_queue_state.rs` | `make_queue_items(3)` | 5 `queue_restore_cursor_*` consumers |
| `src/app/tests_podcast_playback.rs` | `make_socket_merge_ready_app()` | 4 `socket_progress_*` consumers |
| `src/app/tests_feed_group_loading.rs` | `make_home_video_app()` | 3 `feed_home_video_*` consumers |
| `src/app/tests_queue_reorder.rs` | `make_feed_entry("f1")` | 3 consumers |

## Reconfirmation / deferrals

The selected names were checked against the live source and the baseline nextest lists. No ledger candidate drifted, so there are no deferrals.
