# Scoping: task 5.3a (teardown — Library/browse cluster)

Written after 5.2 landed and the 4.10 playback defect was repaired (38/45).
Read before sending an agent at 5.3a.

## Split it in two

5.3a as written is one task deleting six fields across ~20 files, with four
large test modules to rewrite. That is the size that produced this change's
original three-session stall. Send two sequential agents:

* **5.3a-i — Search cluster.** `LibraryTab.search` and the `select()` split.
* **5.3a-ii — Selection-mode cluster.** `album_track_focus`, `series_selection`,
  `series_season_cursor`.

They share almost no files outside `lib_cursor_actions.rs` and
`input_mouse.rs`, so the second agent inherits a small merge surface. Do
5.3a-i first: the `select()` extraction it performs is the shared body that
5.3a-ii's activation paths also need.

## Field-by-field consumer counts

Non-test files first; test modules are the rewrite burden the task names.

`search` — 11 lib_cursor_actions, 8 input_lib_keys, 7 actions_navigation,
6 library_search_actions, 6 render/components/list, 5 library_load_actions
(+ 11 actions_tests, 4 list_late_tests)

`album_track_focus` — 7 input_mouse, 6 render/components/album,
4 render/screens/album_cursor, 4 input_browse_dispatch, 4 actions_navigation
(+ 24 input_music_track_navigation_tests, 8 input_music_track_focus_tests,
7 input_music_track_scope_tests)

`series_selection` — 8 lib_cursor_actions, 6 input_browse_dispatch,
2 input_mouse, 1 shell_tv_workspace, 1 render/components/tv_wide
(+ 7 input_series_music_selection_modal_tests, 2 input_library_scope_routing_tests)

`series_season_cursor` — 6 lib_cursor_actions, 1 each in shell_tv_workspace,
tv_wide, detail_series_view, library_load_actions, input_mouse, input_lib_keys

## Two fields the task lists that must NOT be deleted

The task text says to delete "`nav_stack`/`library`/`library_total` cursors".
Read precisely — two of those are not interaction state:

**`library_total`** is the library's TRUE unfiltered `TotalRecordCount`,
captured from the first unfiltered fetch and used to gate the letter pill row
and per-letter header grouping (see `LIBRARY_PILL_THRESHOLD`). It is API
metadata, not a cursor. It stays on the shell. Its 13 references in
`actions_tests_letter.rs` are load-path tests, not interaction tests, and stay
as they are.

**`nav_stack`** is a `Vec<BrowseLevel>`, and `BrowseLevel` mixes two kinds of
state:

    interaction (component-owned):  cursor, scroll
    fetch/query  (shell-owned):     parent_id, title, items, total_count,
                                    item_types, unplayed_only, sort_by,
                                    sort_order, loading, all_items,
                                    letter_filter, music_grouping

Only `cursor` and `scroll` move. `letter_filter` looks like interaction state
but changes what gets fetched, so it stays shell-owned. Deleting `nav_stack`
itself would take the whole load path with it — that is not this task.

## The `select()` extraction (5.3a-i)

The task requires splitting `select(lib_idx)` into resolve-item plus
`select_item(lib_idx, item)` so that plain-list Enter and the component's
activation `Msg` share one body. Do this BEFORE deleting `search`, so the
deletion has a single activation path to retarget rather than two.

## Scope boundary

Neither half touches Feeds (5.3b), overlays (5.3c), or the framework (5.3d).
`LegacyInput` and `AppLayout` removal is 5.3d.

Note for whoever writes 5.4: `KEY_POLICY` and `KeyPolicyGate::sub_clause()` are
still inert — see the note appended to task 5.4 in `tasks.md`.
