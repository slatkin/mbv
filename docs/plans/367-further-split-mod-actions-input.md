# Plan — Issue #367: Further split `mod.rs` / `actions.rs` / `input.rs`

Status: REVISED after critic pass + user rulings — ready for executor handoff
Base commit surveyed: `07e8246` (HEAD; confirmed nothing drifted since the issue was filed)
Author: planner survey pass, revised per critic findings and explicit user decisions (see §11)

---

## 0. Survey results (confirm/refute the issue's hypotheses)

Current line counts (verified via `wc -l`):

| File | Issue said (post-#365) | Actual now |
|---|---:|---:|
| `src/app/actions.rs` | 3,444 | **3,444** |
| `src/app/mod.rs` | 3,333 | **3,373** (grew +40 since filing) |
| `src/app/input.rs` | 1,823 | **1,823** |

Git history confirms all #365 work is in (`22c3a6d` actions lane A, `ac24a80`/`d6418df` mod lanes D1/D2, `b7c4352` input lane B, `73a9096` render lane C). No production code in these three files has changed since; only later commits touched styling/tests.

Method-count hypotheses:
- **`actions.rs`**: two `impl App` blocks. `impl App[0]` (lines 67–91, 3 accessors) + `impl App[1]` (lines 536–3432, ~95 methods) + three `impl *PlaybackTarget` blocks (94–535, ~440 lines of playback-target dispatch) + 2 free fns. The issue's "149 remaining methods" is stale/high; today it is ~95 methods in the main block. `select` / `play_item` / `play_items_routed` / `select_home` / `go_back` are indeed the central dispatch hub and are called from `input.rs`, `action.rs`, `input_mouse.rs`, and five `render/*` files — **confirmed keep**.
- **`mod.rs`**: one `impl App` block (1467–3291, ~48 methods incl. `build`/`new`/`new_remote`). Critically, **~1,465 lines *before* `impl App` are pure type definitions** (structs, enums, their small impls, the 235-line `App` struct itself, statics, signal handlers). `run()` (2224–2534) + `drain_*` + `handle_session_event` + `teardown` are the #365 "keep-as-is" run loop. The issue framed mod.rs purely as a method problem; **the bigger lever is the type-definition region.**
- **`input.rs`**: one `impl App` block (23–1806, ~44 methods). Keyboard dispatch is table-driven: `handle_key` (83–90) walks `super::input_resolver::CONTEXT_STACK`, an ordered list of `fn(&mut App, KeyEvent)` pointers (`App::handle_key_settings`, etc.). **Consequence: every CONTEXT_STACK handler is already `pub(super)`** (verified) — moving them needs **no** visibility bump. The issue's suggested clustering (settings/sessions/playlists vs library/power vs playback/queue) is sound.

### Scope ruling: this issue's ceiling applies to the 3 named files only
`src/app/library_browse_actions.rs` (897 lines) and ~8 other production files (`render/list.rs` 1811, `render/album.rs` 1740, `action.rs` 1219, `input_resolver.rs` 910, `input_mouse.rs` 844, etc.) are already over 800 lines but are **out of scope for #367**. The user confirmed separate GitHub issues already track those files. **This plan does not touch `library_browse_actions.rs` or any other file outside `mod.rs`/`actions.rs`/`input.rs`.** (The originally-proposed "Lane L5" is dropped — see §11.)

### Established convention (match this exactly)
Production sibling files use a **plain `mod name;`** declaration in `mod.rs` (NOT `#[path]` — that is only used for the test files). Each sibling opens:
```rust
use super::{App, /* types it needs */};
use mbv_core::...;

impl App {
    pub(super) fn moved_method(&mut self, ...) -> ... { ... }
}
```
Cross-file callers rely on `pub(super)`; the rule (per #365) is **bump the moved item's visibility, never the call site.**

---

## 1. Guardrails

**Must have**
- Every resulting file lands **well under 800 lines** — target 200–500. If a natural cluster would land at 600–800, split it further into smaller cohesive siblings, **except** the mandated `actions.rs` dispatch hub (see §3), which the user has explicitly accepted at a ~600–640-line floor as a single cohesive file.
- Behavior-preserving move refactor: cut a method/type from its origin, paste verbatim into the new sibling, add `mod` decl + `use` header, bump visibility as needed. **Move by symbol name, not by line span** — some listed spans nest a sub-range assigned to a different sibling file (e.g. `browse_level_actions.rs`'s span contains `snap_grouped_album_cursor_to_display_order`, which belongs in `lib_cursor_actions.rs`); follow the method/type name assignments in each table, treat line spans as locator hints only.
- For **L3 only** (moving struct type definitions, not just impl methods): this is move **+ systematic field-visibility widening**, not a pure move — see the per-type classification in §5. This is expected and allowed to appear as visibility-only hunks in the diff (not a "logic change").
- One independently reviewable diff per lane; independent review before merge; no push/merge until reviewed.
- `cargo fmt --all -- --check` + `cargo build` + `cargo clippy --all-targets -- -D warnings` + the relevant `src/app/tests_*.rs` / `input_*_tests.rs` / `input_resolver.rs` tests pass per lane.

**Must NOT**
- No logic changes, no signature changes, no "while I'm here" cleanups (Karpathy surgical rule).
- Never edit a call site to satisfy visibility — widen the moved item instead.
- No new dependencies.
- Do not move `select` / `select_home` / `play_item` / `play_items_routed` / `go_back` out of `actions.rs` (central dispatch, keep — user confirmed accepting the resulting ~600–640-line residual rather than splitting the hub further).
- Do not move `run()` or its thin `drain_*` helpers out of `mod.rs`.
- Do not move the `App`/`AppInit` struct out of `mod.rs` (user-confirmed ruling — see §11 Q2: moving it would require widening all 142 private `App` fields to `pub(super)` for zero size benefit, since mod.rs already lands well under 800 without moving it).

---

## 2. Lane structure

Three lanes (L5 from the original draft is dropped — `library_browse_actions.rs` is out of #367's scope, see §0). Same-file work stays in one lane to avoid intra-file merge hell; the mod.rs lane is chunked into two **sequential** sub-lanes.

| Lane | Owns (edits) | New sibling files | Parallelizable? |
|---|---|---|---|
| **L1 — actions.rs** | `actions.rs` | 12–14 new files | yes |
| **L2 — input.rs** | `input.rs` | 6 new files | yes |
| **L3 — mod.rs type defs** | `mod.rs` (defs region) | ~9 new files | must land **before** L4 |
| **L4 — mod.rs impl methods** | `mod.rs` (impl region) | ~7 new files | after L3 merges/rebases |

**Shared-file coordination:** every lane appends `mod xxx;` lines to the declaration block at the very top of `mod.rs`. This is the only cross-lane contention for L1/L2 and it is a trivial union merge (each lane adds its own lines; resolve additively). L3 and L4 both rewrite `mod.rs` heavily → they are **sequential**: L3 merges first, L4 rebases onto it. L1/L2 (which only add `mod` lines to mod.rs) should be integrated and their mod-decl additions reconciled when L4 finalizes the declaration block.

Recommended execution order: **L1, L2 in parallel; L3 then L4 sequentially; final integrator reconciles the `mod` declaration block.**

Each executor works in an **isolated git worktree** (per AGENTS.md `worktrees` skill).

---

## 3. Lane L1 — `actions.rs` (3,444 → accepted floor ~600–640)

Keep in `actions.rs`: the free fns `enqueue_action_context` and `queue_restore_cursor`, plus the `#[test]` fn `enqueue_action_context_names_action_item_and_thin_client_bypass` and the `#[path = "actions_tests.rs"] mod tests;` declaration at the tail — don't lose these in the move; `impl App[0]` accessors (`playback_target`/`playback_display_target`/`playback_indicator_target`), the dispatch hub (`select`, `select_home`, `play_item`, `play_items_routed`, `go_back`, `do_enqueue_folder`, `activate_album_folder_row`), and small library accessors (`remote_audio_indexes`, `remote_subtitle_indexes`, `lib_page_size`, `queue_page_size`, `current_home_item`, `current_lib_item`, `spawn_global_search`).

**Residual size (user-confirmed, revised from the original ~450 estimate):** the mandated dispatch hub alone is ~430 lines (`select` 128 + `go_back` 69 + `select_home` 62 + `play_item` 48 + `play_items_routed` 44 + `do_enqueue_folder` 48 + `activate_album_folder_row` 31), plus kept accessors (~130), import header + free fns (~66), test tail (~12) → **`actions.rs` residual lands at ~600–640 lines.** The user has explicitly accepted this as the floor for this cohesive dispatch concern rather than splitting it further — do not split the hub into additional files.

New sibling files:

| New file | Methods moved (line spans in actions.rs) | Est. lines |
|---|---|---:|
| `playback_target.rs` | `impl PlaybackTarget` (94–186) + `effective_playback_state`/`displayed_queue_playback_state` (1111–1152) | ~140 |
| `playback_target_local.rs` | `impl LocalPlaybackTarget` (187–357) | ~170 |
| `playback_target_remote.rs` | `impl RemotePlaybackTarget` (358–535) | ~180 |
| `audio_subtitle_actions.rs` | `toggle_mute`, `session_toggle_mute`, `cycle_audio`, `push_subtitle_prefs`, `cycle_subtitle_mode`, `next_subtitle_entry`, `toggle_sub`, `cycle_sub` (897–999) | ~105 |
| `notify_actions.rs` | `notify_system`, `notify_with_actions`, `ring_terminal_bell`×2, `trigger_lib_rescan`, `flash_status`, `flash_status_high`, `enqueue_route_conflict` (1001–1109) | ~110 |
| `artist_header_actions.rs` | `power_artist_header_action_lib_idx` … `play_selected_artist_header` (1320–1461) | ~142 |
| `lib_cursor_actions.rs` | `move_lib_cursor`, `jump_lib_cursor` (578–715) + `is_viewing_album_folders`, `is_viewing_season_grid`, `enter_series_selection`, `series_selection_episodes`, `switch_series_selection_season`, `is_home_video_view` (785–895), `snap_grouped_album_cursor_to_display_order` (2132–2156), `recursive_album_display_item` (3411–3431) | ~300 |
| `browse_level_actions.rs` | `update_current_browse_level`, `normalize_current_browse_level_items`, `handle_loaded_level`, `maybe_auto_push_power_tv_season_level` (2095–2227) | ~130 |
| `lib_event_actions.rs` | `handle_lib_loaded`, `maybe_capture_library_total_and_apply_default_pill`, `handle_lib_page_appended`, `handle_lib_refreshed`, `handle_restored_library_position`, `handle_lib_event` (2229–2609) | ~380 |
| `shuffle_folder_actions.rs` | `shuffle_play`, `play_folder`, `is_tvshows_library`, `active_lib_is_tvshows`, `shuffle_folder` (1873–1999) | ~127 |
| `power_home_actions.rs` | `power_home_current_item` … `power_home_enqueue` (2814–3003) | ~190 |
| `power_cw_library_tab_actions.rs` | `power_cw_move_cursor`, `power_cw_play`, `power_cw_enqueue`, `power_cw_toggle_watched` (2767–2810) + `library_tab_count`, `set_library_tab`, `library_tab_next`, `library_tab_prev` (2720–2765) | ~90 |
| `session_command_actions.rs` | `spawn_sessions_load`, `session_jump_track`, `remote_seek_ticks`, `clear_playback_overlays`, `do_session_command` (2001–2093) | ~93 |
| `consume_quit_actions.rs` | `try_quit` (2611–2661), `on_video_consumed`, `on_audio_consumed` (2663–2718) | ~106 |
| `library_load_actions.rs` | `spawn_load_playlists`, `spawn_open_playlist`, `open_playlists_panel`, `load_and_play_playlist` (3005–3083), `rebuild_library_tabs_from_views` (3085–3148), `fetch_home` (3150–3202), `settings_scroll_follow` (3342–3353), `update_lib_search` (3355–3409), `refresh_lib`/`refresh_queue`/`refresh_current_view` (1806–1871) | ~330 |
| `ws_event_actions.rs` | `handle_ws_event` (3204–3340) | ~137 |
| (keep) `queue_actions.rs` (existing 403) | move `enqueue_selected`, `append_item_to_queue_and_sync` (1248–1318) here | +70 → ~473 |

All estimates ≤ ~380. **Visibility:** every moved method that is called from outside its new file → bump to `pub(super)`. Notable cross-file callers to check: `handle_lib_event` (called from `run()` loop in mod.rs), `handle_ws_event` (called from run loop), `on_video_consumed`/`on_audio_consumed` (player-event path), `try_quit` (called from `input.rs` `handle_global_view_key`), the `power_home_*`/`power_cw_*` set (called from `input.rs` key handlers) — these are already effectively cross-file and need `pub(super)`. Verify each with `find_referencing_symbols` before finalizing.

---

## 4. Lane L2 — `input.rs` (1,823 → target ~210)

Keep in `input.rs`: context-menu accessors + podcast mark-id helpers + `tab_count` (24–81), `handle_key` (83–90), `handle_key_global_overlay_open` (100–119), `handle_key_ctrl_l`/`handle_key_f5_refresh`/`handle_key_view_dispatch` (417–442), tab-visibility helpers `visible_tab_range`/`ensure_tab_visible`/`tab_title_widths` (1728–1777), `load_prefs`/`save_prefs` (1779–1805).

New sibling files:

| New file | Methods moved (spans) | Est. lines |
|---|---|---:|
| `input_settings_keys.rs` | `handle_key_settings` (585–689), `handle_key_help` (691–703), `handle_key_sessions` (705–752) | ~166 |
| `input_playlist_keys.rs` | `handle_key_playlists` (754–908), `handle_key_save_modal` (545–582), `handle_key_save_playlist_entry` (92–98), `handle_save_playlist_key` (1621–1726) | ~300 |
| `input_confirm_keys.rs` | `handle_key_confirm_clear_queue`, `handle_key_confirm_rescan`, `handle_key_confirm_skip_intro`, `handle_key_confirm_next_up`, `handle_key_clear_queue_prompt` (290–415) | ~126 |
| `input_home_search_keys.rs` | `handle_key_home_search` (155–263), `handle_key_context_menu` (910–940) | ~140 |
| `input_queue_keys.rs` | `handle_queue_key` (1242–1619), `handle_queue_column_width_key` (1206–1240), `handle_key_queue_column_width` (140–146), `is_queue_column_width_resize_key` (1202–1204) | ~425 |
| `input_lib_power_keys.rs` | `handle_lib_key` (942–1073), `handle_power_cw_key` (1122–1196), `power_cw_page` (1198–1200), `handle_key_power_lib_search` (265–288), `handle_key_power_album_track_mode` (134–138), `active_power_album_track_lib_idx` (121–132), `handle_key_power_sidebar_toggle` (148–153), `handle_lib_search_key` (500–543), `handle_enqueue_selected_key` (490–497), `handle_global_view_key` (452–489), `handle_playback_key` (1079–1120), `adjust_volume` (1075–1077) | ~430 |

**Visibility notes (L2):**
- CONTEXT_STACK handlers (`handle_key_settings`, `handle_key_help`, `handle_key_sessions`, `handle_key_playlists`, `handle_key_save_modal`, `handle_key_save_playlist_entry`, `handle_key_home_search`, `handle_key_power_lib_search`, `handle_key_confirm_*`, `handle_key_clear_queue_prompt`, `handle_key_context_menu`, `handle_playback_key`, `handle_key_power_album_track_mode`, `handle_key_queue_column_width`) are **already `pub(super)`** → no bump needed; `input_resolver.rs` references them via `App::…` and will keep working.
- Currently-private handlers that gain a cross-file caller after the split → **bump to `pub(super)`**: `handle_queue_key` (called by `handle_key_view_dispatch`, which stays in input.rs), `handle_lib_key` / `handle_power_cw_key` / `handle_global_view_key` (called by `handle_queue_key`, which lands in a *different* file — `input_queue_keys.rs` vs `input_lib_power_keys.rs`), and `handle_save_playlist_key` (called from `handle_key_playlists`). Keeping `handle_lib_key`/`handle_power_cw_key`/`handle_global_view_key`/`handle_enqueue_selected_key`/`handle_lib_search_key` together in one file (`input_lib_power_keys.rs`) minimizes bumps to the boundary crossings only.
- **Shared-helper watch (issue warning):** the context-menu key path calls `open_context_menu` / `open_context_menu`-family helpers that already live in `context_menu_actions.rs` / `input_context_menu.rs` (extracted in #365) — so the mouse/keyboard shared-helper hazard is largely already resolved. Still verify `handle_key_context_menu` and `handle_global_view_key` (`open_context_menu`, `tab_count`, `try_quit`, `set_library_tab`) resolve via `pub(super)` after the move.

---

## 5. Lane L3 — `mod.rs` type definitions (sequential, first)

Extract the ~1,465-line type-definition region into cohesive sibling modules. **This lane is move + systematic field-visibility widening, not a pure move** — a critic pass caught that the original draft's "no field-visibility changes required" claim was wrong and would have broken the build. The mechanism:

- Rust's privacy rule: a plain-private item is visible to its defining module **and that module's descendants**. Today every moved struct is defined in `mod.rs` (`crate::app`), so sibling files (`crate::app::actions`, `crate::app::render::*`, etc.) — all descendants — can read private fields freely.
- The moment a struct moves to a new sibling module (e.g. `crate::app::types_library_tab`), the old siblings are **no longer descendants of the struct's new defining module** and lose field access → compile errors (E0616) everywhere that struct's fields are touched outside its own `impl` block.
- **Enums are exempt**: enum variant fields inherit the enum's own visibility, not per-field privacy, so enums move with zero visibility changes.
- **Structs are not exempt**: verified empirically — `App` has 142 plain-private fields with zero `pub`; same pattern for `LibSearch`, `PlayerTab`, `PlaybackState`, `QueueScopeResolution`, `LibraryTab`, etc. (`LibraryTab.album_track_focus` alone is read from 11 sibling files, `.nav_stack` from 16.)
- **Required fix per moved struct:** mark every field `pub(super)` (equivalent to `pub(in crate::app)`, restoring visibility to all `crate::app` descendants) as part of the move. This is expected, allowed scope for this lane — it will show as visibility-only hunks in the diff, not logic changes.

**Do not move the `App`/`AppInit` struct** — user-confirmed ruling (§11 Q2). It would require widening all 142 `App` fields to `pub(super)`, plus keeping `construct.rs`'s (L4) struct-literal construction of `App { ...142 fields }` working, for zero size benefit — mod.rs already lands well under 800 without moving it (see §7).

| New file | Kind | Items moved (spans) | Field-visibility work needed? | Est. lines |
|---|---|---|---|---:|
| `types_context_menu.rs` | mixed | `ContextAction` (enum), `ContextMenuEntry`/`MultiSelectPopup`/`ContextMenu` (structs), `MultiSelectKind`/`LibraryRouteStage`/`LibraryRoutePopup` (enums) + `impl` (178–274) | yes, for the struct types only | ~96 |
| `types_browse.rs` | mixed | `LibSearch`, `AlbumPathPart`, `AlbumSearchEntry`, `AlbumIndexState`, `SeriesDetail`, `BrowseLevel` (structs) + `impl` (275–396), `restore_library_position` fn (397–445) | yes | ~170 |
| `types_feed.rs` | mixed | `FeedHomeVideoGroup`, `FeedHomeVideoState` (structs) + `impl`, `SavePlaylistStage` (enum), `SavePlaylistDialog` (struct) (446–492) | yes | ~46 |
| `types_events.rs` | enums only | `LibEvent`, `SessionEvent` (496–597) | **no** — clean move | ~101 |
| `types_player_tab.rs` | struct | `PlayerTab` + `impl` (598–752), `same_queue_occurrence` fn (753–756) | yes | ~158 |
| `types_playback.rs` | mixed | `PlaybackTarget` enum + `LocalPlaybackTarget`/`RemotePlaybackTarget` (structs, 89–108), `PlaybackState`/`QueueScopeResolution`/`RemoteSlotState` (structs), `QueueScope`/`HomePane` (enums) (816–913), `PendingQueueAction` (enum, 1344–1353), `ArtistHeaderSelection` (enum), `SuspendedLocalSession` (struct, 990–1001) | yes, for the struct types | ~140 |
| `types_library_tab.rs` | struct | `LibraryTab` + `impl` (914–989) | yes (widely-read fields: `album_track_focus` in 11 files, `nav_stack` in 16 — verify each) | ~76 |
| `types_settings.rs` | mixed | `PanelFocus` (struct) + `impl` (1354–1377), `SettingKey` (enum) + `SETTING_SECTIONS` static (1378–1462) | yes, for `PanelFocus` only | ~110 |
| `bootstrap.rs` | struct | `LocalDaemonBootstrap` struct + `bootstrap_local_daemon_queue` fn (757–815) | yes | ~60 |
| `resize.rs` | aliases/fn | resize type aliases (`ResizeRegisterTx`/`ResizeResponseRx`) + `spawn_resize_worker` (1272–1343) | no (no struct fields) | ~72 |

For every "yes" row: after the move, run `cargo build`, fix each E0616 by adding `pub(super)` to the named field (never widen further than `pub(super)`, never touch the call site), then re-build until clean. This is mechanical but real work — budget time for it per-lane, and call it out explicitly in the lane's PR description so reviewers aren't surprised by visibility-only hunks.

---

## 6. Lane L4 — `mod.rs` impl methods (sequential, after L3)

Keep in `mod.rs`: imports, module declarations, statics/mutexes/type aliases (`DirectConnectFn`, `SessionsLoadFn`, override/test-lock mutexes), `PAGE_SIZE`/panel-width consts, `run()` (2224–2534) and its thin `drain_notif_actions`/`drain_search_results`/`drain_session_events` helpers (2536–2614).

New sibling files:

| New file | Methods moved (spans) | Est. lines |
|---|---|---:|
| `construct.rs` | `build` (1716–1886), `new` (1888–1997), `new_remote` (1999–2147), `handle_failed_local_daemon_adoption` (2149–2158), `build_image_picker` (2160–2185) | ~460 |
| `library_position_state.rs` | `save_default_library_position`, `active_library_position_scope_for`, `saved_library_position`, `replace_saved_library_position`, `focus_power_queue_initial_item`, `activate_library_position`, `clear_saved_library_position` (1500–1646) | ~146 |
| `remote_slot_state.rs` | `remote_slot_state`, `has_sessions_panel_connection`, `can_disconnect_remote`, `disconnect_remote`, `sessions_overlay_footer` (1648–1692) | ~45 |
| `player_event.rs` | `handle_player_event` (2922–3290), `sync_volume_from_player` (2894–2920) | ~396 |
| `run_loop_events.rs` | `handle_session_event` (2616–2777), `teardown` (2779–2892) | ~276 |
| `app_state_misc.rs` | `queue_column_width_max_for_terminal`, `normalize_queue_column_width`, `clamp_queue_column_width` (1467–1486), `note_focus_gained`, `note_focus_lost` (1488–1498), `set_panel_focus` (1544–1553), `extrapolated_remote_position` (1694–1696), `ui_config_snapshot` (1698–1714), `wants_terminal_render`, `render_interval` (2187–2222) | ~95 |
| `runtime.rs` | signal handlers `handle_quit_signal`, `install_signal_handlers`, `stdin_has_hup`, `start_quit_watchdog` (110–177) + `init_terminal`, `restore_terminal` (3294–end) | ~110 |

**User-confirmed ruling (§11 Q1):** move `handle_session_event` + `teardown` out of `mod.rs` into `run_loop_events.rs` (and `handle_player_event` into `player_event.rs`), leaving only `run()` + the `drain_*` helpers in `mod.rs`. This supersedes #365's original "keep as-is" scoping — that was a scoping choice for #365, not a technical constraint, and both moved fns are called only from `run()` (stays) so a `pub(super)` bump on each is sufficient (both new files are `crate::app` descendants, so their `App`-field access is unaffected since `App` itself stays in `mod.rs`).

**Visibility (L4):** `build`/`new`/`new_remote` are constructors called from `main`/tests — keep their existing `pub`/`pub(crate)` level. `handle_player_event`, `handle_session_event`, `teardown` are called from `run()` (stays in mod.rs) → `pub(super)`. `library_position_state` methods are called from `actions.rs`/`input.rs` → `pub(super)`.

---

## 7. Line-budget check (does mod.rs actually land under 800?)

mod.rs after L3 + L4 keeps: imports (~28) + module decls (~73, corrected from the original ~55 estimate — ~20 existing prod decls + ~13 existing test decls + ~40 new sibling decls across all lanes) + statics/mutexes/aliases (~55) + consts (~10) + `run()` (~311) + `drain_*` (~78) = **~555**. Comfortably under 800, per the confirmed ruling to move `handle_session_event`/`teardown` out (§11 Q1). Every other new file is ≤ ~460, most ≤ ~300. `actions.rs` lands at the accepted ~600–640 floor (§3). No new file approaches 800. ✅

---

## 9. Per-lane verification (Definition of Done)

For each lane, in its worktree:
1. `cargo build` clean (no warnings introduced).
2. `cargo fmt --all -- --check` passes.
3. `cargo clippy --all-targets -- -D warnings` clean.
4. Targeted tests for the touched area pass: `cargo test -p <crate> app::` plus the specific `tests_*` / `input_*_tests` / `input_resolver` modules relevant to the moved code. Full `app::` test run is what catches any `super::`-path breakage in `#[path]`-included test modules (e.g. `actions_tests.rs`, `input_*_tests.rs`) that reference moved private items — these are `crate::app` descendants so `pub(super)` covers them, but verify via the test run, not by inspection.
5. `wc -l` on every file the lane created/touched → confirm all < 800 (target < 500; `actions.rs` residual accepted at ~600–640 per §3).
6. Diff is a **move**: `git diff` shows deletions in origin file + additions in new files + `mod`/`use` declarations + visibility-keyword changes (`pub(super)`) only. No behavior/logic changes. For L3, expect legitimate field-visibility hunks (§5) — these are in-scope for this lane, not a red flag.
7. Independent review (code-reviewer/critic) before merge; no push/merge until reviewed.
8. If a lane's build won't go green: discard the worktree and restart that lane rather than partially merging.

Integrator step (after all lanes): reconcile the `mod` declaration block in `mod.rs` (union of all lanes' additions), then one final `cargo build` + `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + full `app::` test run.

---

## 10. Prior open questions — all resolved (see §11)

The original draft posed 4 open questions to the critic/user. All are now resolved with explicit rulings; see §11. This section is kept only as a pointer for historical context — do not re-litigate during execution.

---

## 11. Rulings (user-confirmed, binding for execution)

1. **Scope of the 800-line ceiling:** applies **only** to the 3 files named in issue #367 (`mod.rs`, `actions.rs`, `input.rs`). The ~8 other over-ceiling production files (`render/list.rs`, `render/album.rs`, `render/home.rs`, `render/detail.rs`, `render/chrome.rs`, `action.rs`, `input_resolver.rs`, `input_mouse.rs`) and `library_browse_actions.rs` are **out of scope** — already tracked by separate GitHub issues. Original "Lane L5" is dropped from this plan.
2. **`actions.rs` dispatch-hub residual (~600–640 lines):** accepted as the floor. Do not split `select`/`select_home`/`play_item`/`play_items_routed`/`go_back`/`do_enqueue_folder`/`activate_album_folder_row` into further files — keep as one cohesive hub file, comfortably under 800.
3. **Q1 — `handle_session_event`/`teardown`:** move both out of `mod.rs` into `run_loop_events.rs` (with `handle_player_event` in `player_event.rs`), per §6.
4. **Q2 — `App`/`AppInit` struct:** do **not** move it; leave in `mod.rs`. No `app_struct.rs` file. (This also removes the highest-risk item from the original L3 draft.)
5. **Naming scheme** (`*_actions.rs` / `types_*.rs` / `input_*_keys.rs`): approved by the critic as consistent with #365 precedent (`queue_actions.rs`, `library_browse_actions.rs`, `context_menu_actions.rs`, `input_mouse.rs`) — proceed as specified in §3/§4/§5/§6.
