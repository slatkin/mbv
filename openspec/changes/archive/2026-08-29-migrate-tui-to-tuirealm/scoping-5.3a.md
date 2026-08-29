# Scoping: task 5.3a (teardown — Library/browse cluster)

Written after 5.2 landed and the 4.10 playback defect was repaired (38/45).
**5.3a is now closed.** This document is kept for the field-by-field analysis
below, which 5.3d still needs. The planning sections are superseded by
`tasks.md`; read that first.

## How it was actually split — and what it cost

The original plan here was two sequential passes (Search, then a single
"selection-mode cluster" of `album_track_focus` + `series_selection` +
`series_season_cursor`). Two things went wrong with that, both worth keeping:

**The sequencing note was addressed to the reader, not the agent, and an agent
executed it anyway.** The line "Send two sequential agents" was read by the
5.3a-i implementer as its own instruction; it dispatched a subagent at 5.3a-ii
out of order, into the very files the sequencing existed to protect, and the run
had to be discarded. Sequencing belongs in scoping and handoff documents that no
implementer sees; every agent prompt now states only its own in-scope work plus
an explicit "implement this yourself, do not delegate".

**The three selection-mode fields were sized by grep reference count, and that
metric does not predict cost.** Most hits were exhaustive `LibraryTab { .. }`
struct literals — 94 of them, ~30 in test modules — so *any* field deletion cost
the same ~90 compile-forced one-line edits regardless of how entangled the field
actually was. Task `5.3-pre` fixed this by adding `LibraryTab::new`, collapsing
the per-field fan-out (`series_season_cursor` went from 38 files to 7). The
counts in the next section predate that and should be read as reference counts
only. **Size by compile-forced edit sites, not grep hits.**

With the constructor in place the fields separated cleanly, and the real split
was three passes, not two: Search (`008be6c5`..`9ac69d81`), the constructor
(`5d9e77ec`), and series (`9e4bd7c`, `153c9b9`, `758d0a84`).
`album_track_focus` turned out not to belong to this cluster at all and moved to
5.3d — the reasons are recorded on task 5.3a in `tasks.md`.

## The teardown hazard this task discovered

The stage-1 mirror guard (`initialized` + `last_mirrored_*`) means "the shell is
authoritative unless the user has moved." **It inverts to "preserve local state
forever" the moment the shell stops supplying a real value.** Commit `153c9b9`
deleted `series_selection`/`series_season_cursor` and left
`shell_tv_workspace.rs` feeding literal `0, None`, so the guard compared local
state against a constant and never re-mirrored — the season and episode cursors
survived a change of series. `758d0a84` repaired it with an identity field
(`last_series_id`) that resets local state before the guards run.

Every remaining teardown (5.3b, 5.3c, 5.3d) has this hazard. When a field's
shell-side source is deleted, the component's guard against it must be deleted
in the same commit, not left comparing against a placeholder.

It also slipped a weak verification gate: the agent deleted the test asserting
the reset behaviour *because* it had deleted the behaviour. Test counts alone did
not catch it (1213 → 1211 looked like ordinary cleanup). Teardown prompts now
require naming every removed test and why it is obsolete rather than retargeted.

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
