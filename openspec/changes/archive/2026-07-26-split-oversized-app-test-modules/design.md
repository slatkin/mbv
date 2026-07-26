## Context

Issue #366 extracted 241 tests from `src/app/tests.rs` into ten sibling modules, but grouped by broad subsystem coherence rather than enforcing the 800-line ceiling. Four resulting files remain oversized: `tests_library_position.rs` (1,460 lines), `tests_feed_podcast.rs` (1,360), `tests_queue_mutation.rs` (857), and `tests_session_connect.rs` (841). The production splits in #367 and #368 are complete, and these four files have not changed since #366, so no production refactor remains as a prerequisite.

The current issue text is only a starting hypothesis. The feed/podcast inventory is now 14 feed tests and 7 podcast tests, and putting all feed tests in one replacement would produce an approximately 1,000-line file. Library position and session connection likewise contain several domain-distinct groups. All affected modules are direct children of `crate::app`, import production internals through `use super::*`, and import stable shared fixtures through `use crate::app::tests::*`.

## Goals / Non-Goals

**Goals:**

- Place all 108 tests currently registered by the four source modules into focused sibling modules that each remain below 800 lines.
- Align test ownership with existing domain and production seams, especially Library position versus Panel focus, Sessions-panel connection versus Route-owned transport, and queue mutation versus reorder/slot reconciliation.
- Make the implementation mechanically reviewable as a pure move with test inventory parity before and after.
- Produce a concrete lane plan that another agent can execute without rediscovering boundaries.

**Non-Goals:**

- Changing production logic, signatures, visibility, behavior, or public APIs.
- Renaming tests, rewriting assertions or setup, deduplicating helpers, or reorganizing `src/app/tests.rs` fixtures.
- Resolving #375's directory-wide naming policy; this change uses the established `tests_<concern>.rs` convention only.
- Splitting other oversized test modules or implementing #374, #378, or unrelated cleanup.
- Introducing nested test directories, new dependencies, ADRs, or domain terminology.

## Decisions

### Treat #369 as a standalone structural change

The completed #367 and #368 work satisfies #369's original ordering constraint, and current references show the four modules are coupled only through their declarations in `src/app/mod.rs`. Keeping this change standalone preserves its test-only risk profile and avoids mixing mechanical moves with production refactors.

Alternative considered: fold #369 into a broader module cleanup with #374, #375, or #378. Rejected because those issues have different acceptance criteria and production or naming consequences, while #369 needs no production changes.

### Use direct `crate::app` sibling modules

Every destination remains a `src/app/tests_<concern>.rs` file declared with the existing `#[cfg(test)]` and `#[path = "..."]` pattern in `src/app/mod.rs`. This preserves `use super::*` access and the `crate::app::tests::*` fixture path without visibility widening.

Alternative considered: introduce `src/app/tests/mod.rs` and nested files. Rejected because it changes module ancestry and import behavior for no benefit to this move-only change.

### Split on the following fixed concern boundaries

Library-position source:

- `tests_library_position.rs`: position snapshot/model conversion, ordinary persistence, refresh/rescan reset, and home-navigation non-persistence (12 tests).
- `tests_library_position_restore.rs`: runtime activation, asynchronous restore acceptance, fallback persistence, and stale-result rejection (10 tests).
- `tests_panel_focus.rs`: persisted Panel focus and queue-focus cursor initialization (6 tests).
- `queue_restore_uses_saved_cursor_when_last_played_is_missing` moves to `tests_daemon_bootstrap.rs` (1 test).

Feed/podcast source:

- `tests_feed_group_nav.rs`: feed eligibility/state preservation, folder selection, cache use, navigation, cursor clamping, and refresh targeting (10 tests).
- `tests_feed_group_loading.rs`: pagination gating, group aggregation/filtering, and refreshed-event state reconciliation (4 tests).
- `tests_podcast.rs`: podcast detection and podcast context-menu behavior (7 tests).

Queue-mutation source:

- `tests_queue_mutation.rs`: enqueue, clear, remove, context-menu targeting, and local/remote scope isolation (12 tests).
- `tests_queue_reorder.rs`: move, undo, Queue slot identity, and remote queue reconciliation (16 tests).

Session-connect source:

- `tests_daemon_bootstrap.rs`: the eight local-daemon bootstrap/adoption tests plus the queue-restore test moved from library position (9 tests).
- `tests_session_connect.rs`: Sessions-panel endpoint/direct-connect behavior (7 tests).
- `tests_auto_reconnect.rs`: the persisted route/direct-session reconnect policy matrix (6 tests).
- `tests_library_route.rs`: lazy route connection and applying a Library route for playback (8 tests).
- Existing `tests_lifecycle.rs`: `extrapolated_remote_position` runtime-state behavior (1 moved test, in addition to its existing lifecycle tests).

These names intentionally follow the existing test-prefix convention. Small focused modules such as panel focus and auto-reconnect are accepted because they represent explicit, independent domain concepts; combining them would recreate catch-all modules.

#### Exact move manifest

The implementation uses function names, not recorded line numbers, as stable locators:

- `tests_library_position.rs`: `library_position_snapshot_captures_path_focus_and_feed_group`, `browse_level_restore_prefers_item_id_and_clamps_index_fallback`, `restore_library_position_keeps_saved_path_when_levels_exist`, `restore_library_position_clamps_stale_missing_item_to_nearest_fallback`, `restore_library_position_stops_at_deepest_valid_parent`, `applying_library_position_clears_non_position_ui_state`, `save_default_library_position_persists_focused_item`, `move_lib_cursor_persists_default_library_position`, `saving_visible_library_position_keeps_hidden_library_state_entries`, `refresh_lib_clears_saved_position_for_active_library`, `trigger_lib_rescan_clears_only_active_scope`, `power_home_navigation_does_not_persist_library_position_state`.
- `tests_library_position_restore.rs`: `ensure_lib_loaded_for_uses_saved_position_loading_state_without_root_flash`, `activating_saved_power_position_initializes_feed_home_video_state`, `ensure_lib_loaded_for_visible_power_library_accepts_restore_from_queue_focus`, `restoring_library_position_does_not_eagerly_prefetch_all_items`, `restoring_pre_pill_feature_position_captures_library_total_and_shows_pills`, `library_tab_next_activates_saved_placeholder`, `library_tab_next_from_queue_focus_accepts_restore_result`, `restored_default_library_fallback_rewrites_state_file_after_success`, `stale_restore_is_ignored_after_saved_position_is_cleared`, `stale_restore_is_ignored_when_scope_is_no_longer_active`.
- `tests_panel_focus.rs`: `build_restores_panel_focus_from_prefs_for_both_values`, `save_prefs_persists_panel_focus_for_both_values`, `entering_power_queue_focus_selects_now_playing_item`, `entering_power_queue_focus_preserves_valid_queue_cursor_without_now_playing`, `entering_power_queue_focus_defaults_invalid_queue_cursor_to_first_item`, `building_from_panel_focus_prefs_does_not_mutate_saved_library_positions`.
- `tests_feed_group_nav.rs`: `feed_home_video_group_view_requires_homevideos_and_feed_config`, `feed_home_video_group_view_stays_enabled_with_cached_groups`, `fetch_home_preserves_feed_home_video_state`, `select_feed_folder_group_pushes_video_level_for_selected_folder`, `select_feed_folder_group_zero_pushes_all_videos_level`, `select_feed_folder_group_uses_client_side_all_items_cache`, `select_feed_folder_group_updates_feed_state_when_detail_level_exists`, `go_back_keeps_feed_home_video_group_view_intact`, `ensure_feed_home_video_group_level_clamps_stale_cursor_to_available_groups`, `refresh_lib_targets_power_feed_selection`.
- `tests_feed_group_loading.rs`: `feed_home_video_root_does_not_auto_push_before_folder_pagination_completes`, `feed_home_video_root_filters_groups_from_all_video_paths`, `refreshed_does_not_overwrite_feed_root_with_video_items`, `refreshed_restores_feed_loading_state_when_feed_state_is_missing`.
- `tests_podcast.rs`: `podcast_library_detects_collection_type`, `podcast_library_detects_name_when_collection_type_missing`, `podcast_folder_context_menu_uses_play_labels_and_item_state`, `podcast_folder_context_menu_shows_mark_played_when_unplayed_items_remain`, `power_view_podcast_context_menu_uses_left_pane_library_context`, `power_view_podcast_context_menu_offers_mark_all_played_for_selected_show`, `power_view_podcast_context_menu_mark_all_played_uses_all_pill_selection`.
- `tests_queue_mutation.rs`: `ctrl_a_enqueues_from_home_view`, `ctrl_a_appends_to_direct_remote_queue`, `ctrl_a_rejects_v2_direct_remote_append_without_replace_queue`, `rejected_v2_direct_remote_append_preserves_remote_undo_slot_identity`, `clearing_local_queue_in_direct_remote_mode_leaves_remote_queue_intact`, `clearing_remote_queue_in_direct_remote_mode_leaves_local_queue_metadata_intact`, `removing_from_local_queue_in_direct_remote_mode_does_not_touch_remote_queue`, `removing_from_remote_queue_in_direct_remote_mode_does_not_touch_local_queue`, `clearing_remote_queue_does_not_prompt_to_save_local_playlist`, `removing_from_inactive_remote_queue_is_rejected`, `context_menu_remove_targets_displayed_remote_queue`, `stale_context_menu_remove_remote_queue_index_is_ignored`.
- `tests_queue_reorder.rs`: `move_queue_item_up_swaps_items_and_cursor_follows`, `move_queue_item_down_swaps_items_and_cursor_follows`, `move_queue_item_up_is_noop_at_start_of_queue`, `move_queue_item_down_is_noop_at_end_of_queue`, `undo_reverses_a_move_and_cursor_follows_back`, `undo_of_move_does_not_disturb_prior_removal_undo_history`, `undo_of_move_is_refused_if_the_moved_item_is_no_longer_at_to`, `undo_of_move_is_refused_when_duplicate_id_masks_changed_queue`, `resolve_slot_at_maps_index_to_slot_and_rejects_out_of_range`, `queue_edit_preserves_updated_item_fields_after_shadow_model_was_built`, `move_queue_item_for_remote_scope_sends_move_command_and_preserves_local_queue`, `move_queue_item_for_inactive_remote_scope_is_rejected`, `remote_queue_update_reconciles_remote_queue_without_touching_local_queue`, `remote_queue_update_after_move_keeps_cursor_on_moved_item`, `remote_queue_update_after_move_tracks_duplicate_item_by_position`, `moving_now_playing_item_keeps_cursor_on_it`.
- `tests_daemon_bootstrap.rs`: `queue_restore_uses_saved_cursor_when_last_played_is_missing`, `local_daemon_bootstrap_adopts_saved_local_queue_and_source`, `failed_local_daemon_adoption_routes_through_remote_disconnected`, `remote_app_starts_on_local_queue_when_remote_queue_is_empty`, `remote_app_starts_on_remote_queue_when_remote_queue_has_items`, `local_daemon_bootstrap_carries_saved_positions_for_enrichment`, `local_daemon_bootstrap_has_no_positions_without_saved_state`, `local_daemon_bootstrap_uses_restore_cursor_and_carries_last_played_state`, `local_daemon_bootstrap_prefers_existing_daemon_queue_state`.
- `tests_session_connect.rs`: `session_direct_endpoint_prefers_advertised_tcp_port`, `session_direct_endpoint_rejects_non_mbv_without_local_fallback`, `session_direct_endpoint_falls_back_to_local_socket_for_same_host_session`, `f3_direct_upgrade_with_empty_device_name_remains_disconnectable`, `connect_to_session_preserves_direct_upgrade_failure_status_after_fallback`, `connect_to_session_tears_down_an_active_library_route_via_restore_local_mode`, `connect_to_session_is_a_no_op_teardown_when_no_library_route_is_active`.
- `tests_auto_reconnect.rs`: `try_auto_reconnect_restores_a_persisted_library_route`, `try_auto_reconnect_falls_back_to_local_when_route_no_longer_configured`, `try_auto_reconnect_restores_a_persisted_direct_session`, `try_auto_reconnect_falls_back_to_local_when_device_not_found`, `try_auto_reconnect_is_a_no_op_when_disabled`, `try_auto_reconnect_is_a_no_op_when_nothing_was_persisted`.
- `tests_library_route.rs`: `try_daemon_route_connect_returns_remote_player_on_successful_connect`, `try_daemon_route_connect_returns_a_ready_to_display_warning_without_flashing_on_failure`, `app_construction_never_attempts_a_daemon_route_connect`, `apply_route_for_playback_swaps_to_routed_daemon_on_success`, `apply_route_for_playback_falls_back_to_local_with_warning_on_connect_failure`, `apply_route_for_playback_is_noop_when_item_already_matches_active_route`, `apply_route_for_playback_restores_local_when_item_has_no_route`, `apply_route_for_playback_restores_local_via_restore_local_mode_when_swap_to_a_different_route_fails`.
- Existing `tests_lifecycle.rs`: `remote_position_extrapolation_does_not_round_up_partial_seconds`.

### Preserve test content and local ownership verbatim

Tests move with their contiguous attributes, comments, nested helper functions, local imports, statics, lock guards, override installation, and override cleanup. Repeated nested helpers remain repeated. Shared fixtures stay in `src/app/tests.rs`. Module headers may be reduced only where an import is provably unused, such as keeping the crossterm key import in queue mutation but not adding it to queue reorder.

Alternative considered: deduplicate large `App` setup and helper functions while files are being touched. Rejected because it would invalidate the move-only evidence and expand review risk.

### Prove preservation with a normalized inventory

Capture `cargo test --bin mbv -- --list` before the move and after it, normalize only the affected module-name segment, sort both inventories, and require an empty diff. This catches lost, duplicated, or renamed tests while allowing the intended module paths to change. The current baseline is 108 affected tests; the executor must derive the inventory from its own worktree rather than trust that number alone.

Formatting, workspace check, Clippy with warnings denied, targeted tests, and the full workspace test suite provide the remaining evidence. A final diff review must confirm that `src/app/mod.rs` changes only in the test declaration block and no production body or fixture changes appear.

## Risks / Trade-offs

- [Test attributes, comments, or nested setup are lost during extraction] -> Resolve test boundaries by function name, include contiguous attributes/comments, move each complete test item, and inspect the move-only diff.
- [A test is lost, duplicated, or renamed] -> Require a normalized before/after `--list` inventory diff and the expected 29 + 21 + 28 + 30 source count.
- [Global override tests become flaky] -> Keep lock guards, override functions, and resets inside their original test bodies unchanged; do not extract shared override helpers.
- [A destination remains over 800 lines] -> Measure all twelve decomposed modules plus the existing lifecycle destination after formatting; the feed tests are deliberately split into navigation and loading/reconciliation modules to avoid this known failure.
- [The split creates many small modules] -> Prefer explicit domain ownership over retaining mixed files; do not split beyond the twelve decomposed modules, and reuse the existing lifecycle module for the runtime-state outlier.
- [Concurrent cleanup causes a declaration conflict] -> Restrict the integrator edit to the test declaration block near the end of `src/app/mod.rs`; resolve additive changes without touching production declarations.
- [Broad naming cleanup leaks into the change] -> Apply only `tests_<concern>.rs` names and leave #375 independent.

## Migration Plan

1. Capture and preserve the affected test inventory in the implementation worktree.
2. Move each source module's tests in four independent lanes, preserving complete test items and placing the remote-position outlier in the existing lifecycle module.
3. Integrate the destination declarations in `src/app/mod.rs` and remove the obsolete `tests_feed_podcast` declaration/file.
4. Format, verify inventory parity, run targeted and workspace gates, measure destination line counts, and review the diff as move-only.

Rollback is a direct revert of the structural commit because there are no data, API, or behavior migrations.

## Open Questions

None. Destination boundaries and scope exclusions are fixed for implementation.
