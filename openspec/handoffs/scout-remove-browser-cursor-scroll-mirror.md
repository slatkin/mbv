# Scout handoff — Remove Emby browser cursor/scroll mirror (#618)

Read-only discovery pass required by D17
(`openspec/changes/migrate-tui-to-tuirealm/design.md`) before a writer is
assigned to `remove-browser-cursor-scroll-mirror`. Covers the generic/Movies/
HomeVideos Emby browser (`BrowserComponent`, `src/app/components/browser.rs`;
shell glue, `src/app/shell_browser.rs`).

## 1. Inputs to the mirror and their production writers

**Pin A — per-frame scroll write-back** (`shell_browser.rs:232-248`,
`render_emby_browser_component`):

- Input: `BrowserComponent::scroll()` — the scroll `view()` last painted,
  set inside `render_wide_movies` / `render_generic_movies_home_video_rows_with_ctx`.
- Write target: `self.app.libs[lib_idx].nav_stack.last_mut().scroll` — same
  field `push_emby_browser_content` → `library_list_render_ctx` reads back
  into `LibraryListRenderCtx::scroll()` → `BrowserComponent::set_content`
  writes into `self.scroll` on the *next* choke-point call. This is the
  closed write→read→write loop the issue calls the mirror.
- `BrowseLevel.scroll` is also written by: `BrowseLevel::from_position_level`
  (`types_browse.rs`, restore-from-disk), `BrowseLevel::scroll_for_cursor`
  (same file, cursor-driven default). No other production site assigns
  `.scroll` directly (grep-confirmed).

**Pin B — cursor round trip** (`shell_browser.rs:92-106`,
`handle_browser_request`; `components/browser.rs:327-379`,
`handle_crossterm_key`; `browser_navigation.rs`, the component-local movement
math):

- Component optimistically updates `self.cursor` via `move_rows` /
  `move_cursor_delta` / `jump_cursor` (`browser_navigation.rs`), *then* emits
  `BrowserMoveRows`/`BrowserMoveColumn`/`BrowserJumpCursor`.
- Shell calls `App::move_lib_cursor_rows` / `move_lib_cursor` /
  `jump_lib_cursor` (`lib_cursor_actions.rs`), which **independently
  recomputes** the same movement against `BrowseLevel.cursor` (own
  letter-grouped/flat/column-stride logic, structurally mirroring but not
  literally sharing the component's arithmetic) and additionally writes
  `library_position_state` (`save_default_library_position`), calls
  `mark_library_navigation`, updates `self.last_nav_at`, and conditionally
  calls `maybe_fetch_next_page` when idle.
- The shell then calls `push_emby_browser_content()` at the writer choke
  points enumerated in §3, which re-reads `BrowseLevel.cursor` through
  `library_list_render_ctx` back into `BrowserComponent::set_content`,
  overwriting the component's own optimistic value with the App's
  independently-computed one. Two independent implementations of the same
  arithmetic must stay byte-identical or the cursor visibly snaps.

## 2. Component-local interaction state vs. shell-owned content/cache/effect state

Component-local (belongs to `BrowserComponent` once the mirror is gone):
`cursor`, `scroll` *during live interaction*, `wide_movies*` display flags
(already pushed, not mirrored back), `layout` (render-derived hit geometry,
already component-only), `image_paint` (already component-only, taken by the
shell).

Shell-owned (legitimately projected one-way, not a mirror): `context:
LibraryListRenderCtx` (items, letter filter, search state, counts — Emby
data), `focused` (panel-focus flag), the four typed selected-item effects in
`handle_browser_request`, refresh/rescan/back/letter-pill cycling (all
already effect-only, no cursor re-read — see the extensive doc comments
already in `shell_browser.rs` explaining this for each arm).

The one field that is legitimately **shell-owned persisted navigation
state**, not interaction state, is `BrowseLevel.cursor`/`.scroll` as the
*resting position of a browse level the user is not currently looking at*
(library-position restore across relaunch, folder-in/folder-out restore).
That is not itself the mirror; the mirror is *re-deriving it every frame/every
keystroke instead of at the navigation events that actually change which
level is displayed*.

## 3. Raw input forwarding and existing effect entry points

`BrowserComponent::handle_crossterm_key` / `handle_key` already route every
claimed key through a typed `ShellRequest` (never a raw `KeyEvent` past the
component boundary for a claimed key) — this surface is *not* on the
`GlobalViewKey` raw-key list for its own local chords; unclaimed keys still
forward via `Msg::Shell(ShellRequest::GlobalViewKey(key))`, which is the
sanctioned typed-global-bridge shape used by every migrated surface, not a
mirror.

`push_emby_browser_content()` (the re-sync half of the cursor pin) is called
from: `shell_run.rs:146,204,244,293,439` (post key-tick paths + async
completion drains), `shell_messages.rs:104,122,129,200,303,333,409` (after
each `handle_browser_request` arm and several async-completion arms),
`shell_library.rs:127,245` (tab-switch / library-load paths), `shell.rs:283`
(construct/bootstrap). All are legitimate writer choke points *for content*;
they become the candidate choke points for a *replacement* cursor/scroll push
that fires only at real navigation events instead of every content refresh.

## 4. Legacy underpaint / cover image / layout-only-from-legacy-renderer

- `paint_home_image` / `take_image_paint`: already the sanctioned adapter
  shape (mirrors `HomeComponent`) — shell paints an image the component
  cannot paint itself (no image-cache authority). Not part of this pin;
  D18 step 2 (not this issue) eventually asks whether image-cache authority
  moves.
- `movies_wide_right_area` / `is_wide_movies_active()`
  (`shell_browser.rs:195,210-220`, `set_wide_movies`): populated by the
  **legacy wide renderer inside `self.app.render(f)`** (the base frame,
  still called from `shell_run.rs` per issue #613 item 3). D18 forbids
  computing the wide-Movies type-by-width derivation into the component
  before that legacy renderer is deleted — this pin survives this change
  unchanged; see §6 ordering resolution.
- `self.app.layout.main.left_sorted_indices` / `left_item_rows`: read by
  both the App's own `letter_vertical_delta`/`move_lib_cursor_inner` *and*
  the component's `browser_navigation.rs` (its own `self.layout` populated by
  `render_generic_movies_home_video_rows_with_ctx`, which the component calls
  itself, not the legacy renderer) — component and App read genuinely
  separate copies of `LayoutMain`, populated by the same shared render
  function called from two places. No shared-mutable-state hazard, but it is
  the reason Pin B's arithmetic is *duplicated* rather than trivially
  reusable — the App-side copy of `LayoutMain` is stale relative to the
  component's by up to one paint versus one input event.

## 5. Unrelated readers blocking immediate `BrowseLevel` field deletion

`grep -rl nav_stack src/` (excluding `*_tests.rs`) returns 37 production
files. `BrowseLevel.cursor`/`.scroll` cannot be deleted as a unit; each
reader either (a) is itself a navigation-choke-point writer that stays, (b)
reads `.cursor` for a *different* concern than live browser interaction and
is out of scope for this change, or (c) is genuinely coupled and must be
re-homed. Non-exhaustive by-concern breakdown from the read (full unrelated
list is available with `grep -rl nav_stack src/ | grep -v _tests.rs`):

- **Pagination**: `library_load_actions.rs`, `library_browse_actions.rs`,
  `maybe_fetch_next_page` (referenced from `lib_cursor_actions.rs`) — reads
  `.cursor`/`.items.len()` to decide when to fetch the next page. Stays; it
  is a legitimate `BrowseLevel` consumer, not part of the mirror.
- **Position persistence**: `library_position_state.rs`,
  `types_browse.rs::to_position_level`/`from_position_level` — the
  literal persisted-position authority this change must keep intact.
- **Context menu**: `context_menu_actions.rs` — resolves the target item at
  `nav_stack.last().cursor`; the Browser's own context-menu arm
  (`ShellRequest::BrowserContextMenu`) already bypasses this by carrying the
  component-resolved item (see `shell_browser.rs` doc comment on that arm),
  so this reader is exercised only by *other*, still-legacy, browse surfaces
  reachable through the same `nav_stack` shape (TV/Music) — out of scope.
- **Shuffle**: `shuffle_folder_actions.rs` — same shape as context menu;
  `BrowserShuffle` already carries the component-resolved item.
- **Letter pills**: `render/screens/pills.rs`,
  `App::cycle_letter_pill`/`should_show_letter_pills` — cycles
  `BrowseLevel.letter_filter`, not `.cursor`; unaffected by this change.
- **Music grouping**: `music_grouping.rs`, `music_actions.rs` — a different
  `nav_stack` level (album-folder), reached only when
  `is_viewing_album_folders`, which the Browser mount gate already excludes
  (Music is not a `BrowserKind`). Out of scope.
- **Library search**: `library_search_actions.rs`,
  `shell_inline_search.rs` — reads `nav_stack` to build search scope, not
  cursor/scroll interaction. Out of scope.
- **`src/app/render/` (9 files: `list.rs`, `card.rs`, `widgets.rs`,
  `list_context.rs`, `album.rs`, `album_detail.rs`, `music.rs`,
  `music_wide.rs`, `tv_wide.rs`, plus `screens/pills.rs`,
  `screens/album_cursor.rs`)**: these are the *shared render functions* the
  component itself calls (`render_generic_movies_home_video_rows_with_ctx`
  etc.) — they take `cursor`/`scroll` as **parameters**, they do not read
  `nav_stack` directly except where the App's own draw path (still-legacy TV/
  Music/album detail branches) calls them with App-derived arguments. Not a
  blocker: the component already supplies its own arguments today.
- **TV / Music workspace shells** (`shell_tv_workspace.rs`,
  `shell_music_workspace.rs`, `shell_audiobookshelf_podcast.rs`,
  `mouse_gestures.rs`, `feed_actions.rs`,
  `types_library_tab.rs`/`types_events.rs`): separate surfaces or shared
  types that read `nav_stack` for their own (unmigrated or differently-owned)
  interaction state. None can be re-homed by this change; they are the
  reason `BrowseLevel.cursor`/`.scroll` remain fields on `App`, not deleted.

**Conclusion**: no writer unit in this change deletes a `BrowseLevel` field.
The unit of work is *when* `BrowseLevel.cursor`/`.scroll` get written from
the Browser surface (navigation choke points, not per-keystroke/per-frame
recomputation-and-resync), not *whether* they exist.

## 6. Ordering hazard resolution (D17 stage 5 vs. issue #613)

Not circular once "underpaint" is split into two independent layers:

- **Per-surface geometry source (this change's D17 stage 5/6 scope).** The
  Browser component's *only* remaining underpaint dependency is
  `movies_wide_right_area`, populated by the Emby-specific legacy wide
  renderer functions inside `self.app.render(f)`. D18 step 2 already commits
  this change's eventual stage-5/6 unit to: derive "wide" from the
  component's own `BrowserKey` kind + geometry width, then delete the
  Emby-specific legacy wide-renderer functions this component was reading
  geometry from. That is a bounded, surface-local deletion this change can
  finish.
- **Global underpaint call (issue #613 item 3 scope).** `self.app.render(f)`
  itself — the single call in `shell_run.rs` that paints the legacy surface
  beneath every migrated component — stays, because TV/Music/album-detail
  branches still depend on it. Deleting *that* call is #613's job, and #613
  correctly sequences after #611's slices (including this one) precisely
  because it cannot run until no migrated surface, including the Browser,
  still depends on anything the legacy base frame populates.

Resolution: **this change finishes D17 stage 5/6 for the Browser's own
Emby-specific legacy wide-renderer dependency (`movies_wide_right_area`
production and consumption, scoped to the generic/Movies/HomeVideos
functions) but does not touch the `self.app.render(f)` call itself.** #613
remains correctly sequenced after this change. Recorded in
`openspec/changes/remove-browser-cursor-scroll-mirror/design.md`.

## 7. Smallest compile-complete implementation units and dependency order

1. **Cursor effect re-homing** (~4-5 files: `shell_browser.rs`,
   `components/browser.rs`, `components/msg.rs`, `lib_cursor_actions.rs`,
   `shell_browser_tests.rs`). Replace `BrowserMoveRows{rows}` /
   `BrowserMoveColumn{delta}` / `BrowserJumpCursor{to_end}` with a single
   typed request carrying the component-resolved *index* (not a delta), and
   an `App` method that applies that index directly (writes
   `BrowseLevel.cursor`, then calls the same
   `save_default_library_position`/`mark_library_navigation`/
   `maybe_fetch_next_page`/`last_nav_at` tail `move_lib_cursor_inner`
   currently does) instead of recomputing movement arithmetic. Removes the
   duplicate-arithmetic parity risk described in §1 Pin B. Must land before
   unit 2 (scroll ownership reuses the same choke-point pattern).
2. **Scroll ownership at navigation choke points** (~3-4 files:
   `shell_browser.rs`, `components/browser.rs`, `actions_navigation.rs`
   [`select_item`/`go_back`], `shell_browser_tests.rs`). Remove the per-draw
   `render_emby_browser_component` scroll write-back; instead persist
   `BrowseLevel.scroll` only when a browse level is entered/left
   (`select_item`'s folder push, `go_back`'s pop) and at the same discrete
   input-driven choke points as unit 1's cursor push, not every paint.
   Depends on unit 1 landing first (same request/effect shape).
3. **Underpaint detach** (~3 files: `shell_browser.rs`,
   `components/browser.rs`, the Emby-specific legacy wide-renderer
   functions). D18 step 2: derive `wide_movies`/`home_video`/`letter_pills`
   from the component's own `BrowserKey` + geometry width instead of
   `self.app.layout.main.is_wide_movies_active()`, then delete the
   now-unread Emby-specific legacy wide-renderer functions and
   `movies_wide_right_area` production for this surface only. Depends on
   units 1-2 landing (parity must be proven on stable ground first, per
   D18's explicit prohibition on computing this derivation early). Does
   **not** touch `self.app.render(f)` itself (see §6).

Each unit is independently compile-complete and separately verifiable with
`rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv emby_browser`,
`rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`.
