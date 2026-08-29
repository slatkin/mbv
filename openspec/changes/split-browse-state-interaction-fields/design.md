## Context

Three structs each carry content and interaction state together:

```
        BrowseLevel                    what actually owns it
        ───────────                    ─────────────────────
        items, total_count             shell   (fetched)
        loading, all_items             shell   (fetch lifecycle)
        sort_by, sort_order            shell   (query parameters)
        item_types, unplayed_only      shell   (query parameters)
        letter_filter, music_grouping  shell   (query parameters)
        parent_id, title               shell   (navigation identity)
        ───────────────────────────────────────────────────────
        cursor                         BOTH — see D1
        scroll                         BOTH — see D1
```

Because the component holds a clone of the whole struct, the bottom two rows
are reachable from both sides, and every projection must hand-patch them.

## Decisions

### D1 — `cursor` is two different facts wearing one name

`BrowseLevel.cursor` means two things depending on whether its level is the
visible one:

| | Live cursor | Resting position |
|---|---|---|
| Which level | The visible one | A level below the top of `nav_stack`, or a non-active library |
| Who changes it | The user, continuously | The shell, at a navigation event |
| Who needs to read it | The painter and local input | `save_default_library_position`, restore-on-entry, `go_back`'s parent re-anchor |
| Persisted | No | Yes (`LibraryPosition`) |

Conflating them is why "is this read a mirror?" cannot be answered
mechanically today — `actions_navigation.rs:244` reading `parent.cursor` after
a pop is legitimate resting-position access, while `actions.rs:139` reading
`lvl.items.get(lvl.cursor)` on the visible level is a mirror read. Both are
spelled identically.

Splitting them means the type tells you which you have, and the mirror read
stops compiling.

*Rejected:* keeping one field and adding a rule/comment about which uses are
sanctioned. That is what the tree does now, and it is why an ast-grep rule and
several warning comments exist.

### D2 — Three outcomes per reader, decided by inventory, not by guesswork

Every reader of a removed field resolves to exactly one of:

1. **Takes the value as a parameter.** The caller already knows the resolved
   item or index. This is the pattern `remove-tv-workspace-cursor-mirror`
   established with `activate_selected_series_item` and
   `remove-browser-cursor-scroll-mirror` used for `apply_lib_cursor_index`'s
   argument. Expected to be the large majority.
2. **Reads the resting position.** Persistence, restore, `go_back`'s parent
   re-anchor. Unchanged in behaviour; changed in spelling.
3. **Reads the component.** Only where the shell genuinely needs the live
   value at an event, via the existing sanctioned downcast accessors.

A reader that fits none of the three is a finding: stop and report it rather
than inventing a fourth path. #611's own history is the argument for this rule
— two of its four slices were sized wrongly because a field's reachability was
assumed rather than traced.

### D3 — Inventory is type-aware, not grep

`.cursor` appears ~74 times outside tests, but most belong to other structs
(`SelectionModal`, `Feeds`, `SearchSidebar`, and the components' own state).
The authoritative inventory comes from `rtk ast-grep`, matching field access
on the `BrowseLevel` type, not from a text search. #618's scout recorded ~37
non-test `BrowseLevel` readers; task 1 confirms or corrects that figure and
records it before any field moves.

### D4 — Migrate one struct at a time, deepest dependency first

Order: `AudiobookshelfBookBrowseState`, then `AudiobookshelfBrowseState`, then
`BrowseLevel`. The two Audiobookshelf structs are smaller, have a single
component each, and validate the split shape before it meets `BrowseLevel`'s
reader population. Each is independently shippable and independently
verifiable.

*Rejected:* one atomic change across all three. The compiler-forced edit set
for `BrowseLevel` alone is large enough that bundling it with the others
produces a diff no reviewer can hold in their head, and this project's
practice is to split large task groups across sequential agents rather than
run one oversized unit.

### D5 — Deletion, not deprecation

A field is removed in the same task that re-points its last reader. No
transitional accessor is left behind returning the old value; that would
recreate the mirror at one remove and it would survive, because nothing forces
its removal later.

## Risks

- **File-size cap.** Splitting structs and threading parameters will push
  several `src/app/*.rs` files over 800 lines. AGENTS.md requires splitting in
  the same PR. Budget for it; do not gate mid-project units on the cap
  (`rtk make check-code-file-lines` is a pre-PR check, not a per-task one).
- **A reader that needs the live cursor at a point where no component is
  mounted.** Destination components can be unmounted; a shell reader that
  needs the live value then has no source. Expected to surface in
  `save_default_library_position` on tab-switch-away. The resting position is
  the answer, but the ordering (persist before unmount) must be verified, not
  assumed.
- **Silent restore regressions.** Position restore is the behaviour most
  exposed by this change and the least covered by tests. Every struct's task
  group opens with a restore characterization test.
- **Mouse.** `mouse_gestures.rs` reads and writes these fields freely and is
  accepted-broken. Deleting its callees will require deleting or stubbing its
  call sites; that is in scope, repairing its behaviour is not.

## Task 1 — Authoritative reader inventory

Method: `rtk grep` over the files that name `BrowseLevel` / `nav_stack` /
`audiobookshelf_browse` / `audiobookshelf_book_browse`, then read the enclosing
function of every `.cursor` / `.scroll` / `.selected_id` / `.episode_selection`
/ `.chapter_selection` / `.selected_bucket` / `.select(` hit and classify by
D2 outcome. `#[cfg(test)]` sites excluded. ast-grep is syntactic, not
type-aware, so classification is by binding site, not by a type matcher.

### 1.1 — `BrowseLevel::cursor` / `BrowseLevel::scroll`

**Count: 13 non-test read sites, 11 write clusters (~24 distinct sites) in the
`impl App` action layer; the render layer adds 8 read sites and 4 write sites
(subsection 1.1b below).**
#618's scout figure was ~37. The divergence is downward and explained: the two
sibling changes (`split-audiobookshelf-cursor-ownership`,
`remove-music-workspace-cursor-mirror`) plus #615–#618 already deleted the
round-trip readers the scout counted. Not a concerning divergence.

Writes (become component-owned movement, or an explicit resting-position write
at a navigation event):

| Site | Context | Disposition |
|---|---|---|
| `actions_navigation.rs:99` | `select_item` — `lvl.cursor = pos` after resolving a playable item | drop; component owns live cursor (outcome 1 caller already has the item) |
| `actions_navigation.rs:244,278` | `go_back` parent re-anchor `parent.cursor = idx` | **resting-position write** (outcome 2) at the pop event |
| `context_menu_actions.rs:190,339` | clamp after unplayed-only removal | drop; component re-clamps its cursor against the projected content (spec: projection reset) |
| `lib_cursor_actions.rs:218,232,262,297,309` | `move_lib_cursor_inner` / `jump_lib_cursor` / `apply_lib_cursor_index` | movers deleted (task 5.2); `apply_lib_cursor_index` deleted (5.1) |
| `lib_cursor_actions.rs:453` | `snap_grouped_album_cursor_to_display_order` (post-load) | resting-position re-anchor at the Loaded event (outcome 2) |
| `library_position_state.rs:32` | `persist_library_scroll` — `level.scroll = scroll` | **resting-position write** (outcome 2) |
| `mouse_gestures.rs:122,219,231` | mouse cursor writes | delete call sites (task 5.3, D16) |
| `music_actions.rs:56,109` | `cycle_music_group` / `select_music_group` group-level cursor on pop | resting-position write at the group-switch event (outcome 2) |
| `music_actions.rs:196,197` | letter-filter reset `last.cursor = 0; last.scroll = 0` | resting-position write at the query-param event (outcome 2) |
| `music_grouping.rs:304,306,309` | `settle` re-anchor to album index | resting-position re-anchor at the grouping-settled event (outcome 2) |

Reads:

| # | Site | Function | Value | Outcome |
|---|---|---|---|---|
| R1 | `actions.rs:139` | `current_lib_item` — `lvl.items.get(lvl.cursor)` on the visible level | live | **1** — thread the resolved item/index from the component Msg; audit `current_lib_item`'s callers in task 4.3 |
| R2 | `browse_level_actions.rs:85` | `maybe_auto_push_tv_season_level`, from `handle_loaded_level` | cursor just set by level construction | **2** — reads the resting cursor the shell wrote when it built the level; runs synchronously in the Loaded handler, no user movement can interleave |
| R3 | `feed_actions.rs:24` | debug-log formatter | live | none — drop the field from the log line |
| R4 | `lib_cursor_actions.rs:137` | `letter_vertical_delta` | live | deleted with the mover (5.2) |
| R5 | `lib_cursor_actions.rs:206` | `move_lib_cursor_inner` | live | movement resolution moves to the component (5.2) |
| R6 | `library_load_actions.rs:298,304` | `reconcile_libraries` old→new `BrowseLevel` copy on Service refresh | resting | **2** |
| R7 | `library_search_actions.rs:240` | `maybe_fetch_next_page` prefetch threshold `lvl.cursor + PREFETCH_AHEAD` | live | **1** — the component's move Msg already carries the resolved cursor (task 4.3); threshold check takes it as a parameter |
| R8 | `music_actions.rs:51` | `cycle_music_group` reads the group level's cursor before pop | resting (level below top) | **2** |
| R9 | `music_actions.rs:254` | auto-push group view reads root `nav_stack[0].cursor` | resting (root, pre-descent) | **2** |
| R10 | `music_actions.rs:328` | `should_auto_push_music`, from `handle_loaded_level` | cursor just set by construction | **2** — same as R2 |
| R11 | `music_grouping.rs:296` | `settle` reads anchor album id at the grouping-settled event | cursor just set by construction / prior settle | **2** — same as R2 |
| R12 | `types_browse.rs:97,98` | `to_position_level` serialization | resting | **2** |
| R13 | `shuffle_folder_actions.rs:27` | `shuffle_play` (legacy `impl App` path) | live | **1** — the component path (`shuffle_play_selected`) already supplies the target; legacy path re-pointed/removed with the other legacy endpoints |

### 1.1b — Render-layer sites (added 2026-08-29)

The original 1.1 pass scoped only the `impl App` action layer
(`src/app/*.rs`), which is where the compiler forces every re-point — but not
the legacy paint path. `src/app/render/` reads and writes both fields per
frame, the original inventory assigned no D2 outcome to any of them, and
tasks 4.3–4.5 as written would not have compiled (`cargo check` in 4.5 would
surface the render readers after the action-layer re-points). This subsection
closes the gap: every render-layer reader/writer of `BrowseLevel.cursor` /
`BrowseLevel.scroll`, verified against HEAD (`22bad7ad`) by grep for
`.cursor` / `.scroll` / `nav_stack` under `src/app/render/` followed by
reading each hit to confirm the receiver is a `BrowseLevel`. Line numbers may
drift; match on the surrounding code.

Reads:

| # | Site | Function | Value | Outcome |
|---|---|---|---|---|
| R14 | `list_context.rs:14,15` | `library_list_render_ctx` — builds `LibraryListRenderCtx` (items, cursor, scroll, total_count) from `nav_stack.last()` | live | **3** — the shell supplies cursor/scroll from the mounted component: `BrowserComponent::cursor()`/`scroll()` (both already exist) for generic/Movies/home-video, `MusicWorkspaceComponent::album_cursor()` (exists, #620) for grouped Music. `LibraryListRenderCtx` stays a plain-data struct, so every downstream reader of its cursor/scroll re-points transitively with no direct change: `list.rs`, `tv_wide.rs`, `detail_series_view.rs`, `music_wide_browser.rs`, and the renderers in R19/R20 |
| R15 | `music.rs:19` | `music_group_state` — `nav_stack[-2].cursor` for the group-pill highlight | resting (level below top) | **2** — `resting().cursor()`, the same classification the shell's group-level writes already received (`music_actions.rs:56,109,196`, re-pointed in 4.2) |
| R16 | `music_wide.rs:144` | `wide_music_render_ctx` — `selected_album = level.items.get(level.cursor)` on the top album level | live | **3** — `MusicWorkspaceComponent::selected_item()`; 4.4 adds this accessor mirroring `TvWorkspaceComponent::selected_item()`, with a first-mount fallback to the App-derived item like `push_tv_workspace_content` |
| R17 | `music_wide.rs:152` | `wide_music_render_ctx` — `group_cursor` from `nav_stack[-2].cursor` | resting | **2** — same as R15 |
| R18 | `widgets.rs:606` | `selected_album_item` — `lvl.items.get(lvl.cursor)` on the top level | live | **3** — `MusicWorkspaceComponent::selected_item()` (4.4). Callers: `activate_album_folder_row` (`actions_navigation.rs:212`, narrow Enter), `focused_music_track` and the track-fetch trigger in `push_music_workspace_content` (`shell_music_workspace.rs:21,104`) |
| R19 | `detail.rs:124` | `selected_movie_item_with_ctx` — `ctx.items.get(ctx.cursor)` | live | **3** — transitive through `LibraryListRenderCtx`: the ctx is confirmed BrowseLevel-derived (`library_list_render_ctx` → `nav_stack.last()`), so R14's re-point supplies the value from `BrowserComponent::cursor()`; no direct change in `detail.rs` |
| R20 | `detail.rs:152` | `selected_series_item_with_ctx` — `ctx.items.get(ctx.cursor)` | live | **3** — transitive, same as R19 |

Writes:

| Site | Context | Disposition |
|---|---|---|
| `list.rs:585` | `render_list` per-frame viewport write-back `level.scroll = final_offset` | **1** — becomes a `&mut usize` render parameter owned by the narrow legacy render path (the ABS book `browser_offset` / podcast `scroll` pattern from Phases 2–3), not a component write-back. Decision: the mounted painters already record their own scrolls (`BrowserComponent::view` both breakpoints, `MusicWorkspaceComponent` wide), so the write-back's only unconsumed consumer is the narrow legacy surface — a per-frame component read-back into `App` would be exactly the mirror the `sync_*` rule forbids, and the event-driven persist seams (`persist_library_scroll`, `persist_emby_browser_scroll`) are already covered. Narrow TV, whose `TvWorkspaceComponent` is mounted only when wide, is the case that requires the parameter rather than deletion |
| `album_cursor.rs:21` | `move_music_group_display_cursor` — `level.cursor = idx` | **2** — resting-position write at the navigation event via `set_resting_cursor(idx)`; the component already resolved and landed the target in its own `album_cursor`, so no `re_anchor` fires for a mover the component originated (`re_anchor` stays reserved for shell-driven changes, #620) |
| `album_cursor.rs:43` | `jump_music_group_display_cursor` | **2** — same as above |
| `album_cursor.rs:78` | `page_grouped_album_cursor` | **2** — same as above |

Swept and excluded: `album.rs` / `album_inline.rs` / `album_detail.rs` /
`music_wide_browser.rs` take cursor/scroll as plain parameters
(`AlbumRowsCursorCtx`, `ListRenderCtx`) and re-point transitively through R14;
`pills.rs` reads only `letter_filter`; test files (`*_tests.rs`, `tests_*.rs`,
`test_helpers.rs`) build `BrowseLevel` literals and are excluded per the
Task 1 method. No render-layer site fits none of D2's three outcomes.

### 1.2 — Audiobookshelf structs

The sibling change `split-audiobookshelf-cursor-ownership` already moved the
live paths onto the components. Remaining App-struct interaction reads:

`AudiobookshelfBrowseState` — `selected_id`, `episode_selection`, `scroll`,
`episode_filter`:

| Site | Field | Outcome |
|---|---|---|
| `library_position_state.rs:250` (`save_audiobookshelf_position`) | `selected_id`, `cursor()` | **2** — shell keeps a resting `selected_id`, written at the select Msg, read by position save |
| `lib_event_actions.rs:221,262,281,319,371,740` (detail-fetched routing "is this detail for the current selection?") | `selected_id` | **2** — compare against the resting `selected_id`; the fetch was dispatched for that id |
| `audiobookshelf_browse_actions.rs:82,164` (post-`select` readback) | `selected_id` | **1** — resolve id from `cursor` against `state.shows` locally, thread it |
| `audiobookshelf_browse_actions.rs:264` (`selected_show()`) | `selected_id` | folds into the queue-item resolution, outcome **1** |
| `audiobookshelf_browse_actions.rs:185,200` | `episode_selection` | `#[cfg(test)]` only — the test seams now take an explicit `episode_index` |
| `shell_audiobookshelf_podcast.rs:25` | `episode_selection` | already component-owned (`component.episode_selection()`) |
| `audiobookshelf_browse_actions.rs:130,131` | `episode_selection`, `scroll` | refresh reset only — moved onto the component |
| `audiobookshelf_browse_actions.rs:260` (`visible_episodes()` in `selected_audiobookshelf_queue_item`) | `episode_filter` | **1** — the episode index resolves *within the filtered list*, so the shell threads `component.episode_filter()` through `play/enqueue_selected_audiobookshelf_episode` |
| `audiobookshelf_podcast_modal_actions.rs` (`open_podcast_selection_modal`, `select_podcast_selection_modal_filter`) | `episode_filter` | **1**/**3** — `open` reads `component.episode_filter()`; the shell filter-cycle path already owned it; the D14 App mirror write is deleted |
| `lib_event_actions.rs:322,733,738` (detail-fetched / progress modal refresh) | `episode_filter` (was via the App mirror) | the modal now rebuilds via `RefreshSelectionModalAtSelectedFilter`, reading the **`SelectionModal` component's own** filter-pill selection — no App-side filter needed |
| `types_audiobookshelf_browse.rs:108` (`select()` identity-change filter reset) | `episode_filter` | moved to the component's `select_show`; content `select()` exposes `select_changed_identity()` |

`scroll` has **no** non-test reader — it is written by the renderer's inline
flow and read back the next frame; it moves to the component as a
`&mut usize` render parameter (the book-struct pattern from Task 2).
Task 1.2 originally omitted `episode_filter`; it was inventoried during Task
3.2 (rows above). `episode_filter`'s single source of truth is now the
`AudiobookshelfPodcastComponent` (live browsing) and the `SelectionModal`
component's filter pill (the open modal); no `App` field holds it.

`AudiobookshelfBookBrowseState` — `selected_id`, `chapter_selection`, `selected_bucket`:

| Site | Field | Outcome |
|---|---|---|
| `library_position_state.rs:280` (`save_audiobookshelf_book_position`) | `selected_id`, `cursor()` | **2** — resting `selected_id` |
| `lib_event_actions.rs` detail routing | `selected_id` | **2** |
| `audiobookshelf_browse_actions.rs:337` (post-`select` readback) | `selected_id` | **1** — resolve from `cursor` locally, thread |
| `audiobookshelf_browse_actions.rs:412,454` (`activate_audiobookshelf_book_row`, `selected_audiobookshelf_book_queue_item`) | `selected_id` | **1** — thread the resolved book id in the `AudiobookshelfBookIntent` Msg |
| `audiobookshelf_browse_actions.rs:415` (`activate_audiobookshelf_book_row`) | `chapter_selection` | **1** — thread in the `ActivateChapter` Msg (or read the component accessor, outcome 3) |
| `audiobookshelf_browse_actions.rs:359,362` | `chapter_selection` | already param-driven (`set_audiobookshelf_book_chapter_focus(selection)`) |
| `selection_modal_actions.rs:214` | `chapter_selection` write | move onto the interaction struct / component |
| `audiobookshelf_browse_actions.rs:385,386` | `selected_bucket`, `cursor()` | already param-driven (`select_audiobookshelf_book_bucket(bucket_pos)`); component owns `selected_bucket` |

### 1.3 — Flagged readers (fit none of D2's three outcomes)

**None.** R2 / R10 / R11 initially looked like a fourth path (shell reads the
*visible* level's live cursor at a content-loaded / grouping-settled event).
They resolve to outcome 2: `handle_loaded_level` replaces the `BrowseLevel`
wholesale (`*last = level.take()`) and then runs the auto-push / snap / settle
logic synchronously in the same event-handler call, so the cursor those readers
see is the resting value the shell itself just computed in
`BrowseLevel::from_position_level` (or the `0` default from `select_item`), not
a component-owned live value. The resting-position field must therefore be
populated at level-construction time (it is) — task 4.2 must keep that invariant
when it introduces the resting type.

The design-doc Risk "a reader that needs the live cursor where no component is
mounted" is confirmed to land only on the persistence path
(`save_audiobookshelf_position` / `save_default_library_position` on
tab-switch-away), which is outcome 2 by construction. No blocker.
