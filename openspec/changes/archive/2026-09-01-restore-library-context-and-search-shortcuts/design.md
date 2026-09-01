## Context

See `proposal.md` — Why. This change extends `restore-library-ctrl-shortcuts`
(#633) with the two library-panel chords that change did not port to the
Music and TV workspace components: `.` (context menu) and `/` (search).

Relevant existing seams (all Service-generic already):

- `Model::open_context_menu_for(item: EmbyItem)` — `shell_browser.rs:59`
  routes `BrowserContextMenu { item }` straight to it;
  `shell_messages.rs:87` routes `MusicTrackContextMenu` to it for a focused
  track. It anchors on the selected library item; no browser coupling.
- `Model::open_inline_search()` — `shell_inline_search.rs:131` keys off
  `self.app.tab == TabSelection::EmbyLibrary(index)` for *any* Emby library,
  and already special-cases `recursive_album_search_enabled(index)` for
  Music. `BrowserComponent` emits `ShellRequest::OpenInlineSearch` for `/`
  (`browser.rs:315`); the variant and its `shell_messages.rs:171` dispatch
  arm exist.
- `Model::render_inline_search_component` (`shell_inline_search.rs:256`) and
  `inline_search_area()` (`:48`) already resolve a rect for TV
  (`tv_wide_right_area`), narrow Music/TV/browser (`left_area`), and wide
  Music (`wide_music_browser_area`). The overlay paints its own list; the
  underlying workspace must not underpaint while it is active.
- Both components already expose `selected_item() -> Option<EmbyItem>`
  (`music_workspace.rs`, `tv_workspace.rs`), the target resolver the
  `EmbyLibrary*` arms from #633 use.

## Goals / Non-Goals

- Goal: `.` and `/` behave on Music (album level) and TV exactly as on the
  generic Emby browser, with no new router policy and no legacy endpoint.
- Non-goal: any change to how the context menu or inline search *renders* —
  the rects and paints already exist. This change only adds key arms, one
  request variant, and its routing.
- Non-goal: `.`/`/` on Audiobookshelf, Feeds, Queue (legacy never bound
  them there).

## Decisions

### D1: One `EmbyLibraryContextMenu { item }` variant, not per-surface

`BrowserContextMenu`, `HomeContextMenu`, and `MusicTrackContextMenu` are
already distinct variants, but all three tails call `open_context_menu_for`
(or `open_context_menu` for Home's dual cursor). The Music album and TV
series/episode cases both resolve a single `EmbyItem` from the component and
want the identical tail, so one shared variant carrying the resolved item is
enough — matching how #633 chose `EmbyLibrary*` over `Music*`/`Tv*` for the
`Ctrl+*` set. Route it beside the `EmbyLibrary*` arms (`shell_library.rs` if
#633 split it out, else `shell_browser.rs`).

Alternative rejected: `MusicAlbumContextMenu` + `TvContextMenu`. Two variants,
two dispatch arms, identical bodies — no benefit.

### D2: `/` reuses `OpenInlineSearch` verbatim

The component emits the same unit variant `BrowserComponent` emits. No item
payload — `open_inline_search` reads the tab and library itself. The Music
arm sits behind `track_cursor.is_none()` (a focused track is a track-level
context; legacy `/` was an album-list-level key). The TV arm has no such
guard.

Alternative rejected: a new `EmbyLibrarySearch` variant — `open_inline_search`
is already tab-driven and Service-generic, so a distinct variant would just
fan back into the same call.

### D3: Precedence in the Music component

Order in `handle_key`, all under the existing `self.context.focused` early
return:
1. focused-track arms (`track_cursor.is_some()`): existing
   `MusicTrackContextMenu` for `.`; **no** `/` arm here (unclaimed).
2. album-level arms (`track_cursor.is_none()`): new `.` →
   `EmbyLibraryContextMenu { item: selected_item()? }`; new `/` →
   `OpenInlineSearch`.
Place the new album-level arms next to the `EmbyLibrary*` `Ctrl+*` arms #633
added, before the `[`/`]` group-pill arms.

## Risks / Trade-offs

- **Underpaint bleed**: while inline search is active over a Music/TV
  workspace, the workspace's own `view` could paint the ordinary list under
  the overlay. → `draw_frame` calls `project_inline_search_active()` before
  the component views and sets `self.app.inline_search_active`; verify the
  Music and TV workspace `view` paths honor that flag the way the legacy
  `render_list` does. If they do not, add the one guard — this is the only
  integration point that is not already wired. Cover it with a render test.
- **TV target ambiguity**: `.` on a focused episode vs. the series. #633's
  `EmbyLibrary*` arms already resolved this by keeping the series-list
  selection authoritative (`selected_item()`); reuse that exactly so `.` and
  `Ctrl+P` agree.
- **`c` vs `.`**: none — `.` is not a `key_policy` binding, so `Active`
  dispatch delivers it to the focused component with no contention.

## Migration Plan

Pure additive. No data model, no protocol, no rollback concern. Ships behind
no flag. If reverted, Music/TV simply return to dropping `.`/`/` at
`_ => None`.

## Open Questions

None that block specs or tasks. The underpaint guard (Risks) is a
verify-then-maybe-one-line step, not a design fork.

### Unit 1 confirmation: legacy semantics and current branch

In the pre-migration tree (6f4b44bd), `.` on both a Music library row and a
TV library row entered the generic library context-menu path:
`src/app/input_lib_keys.rs:142` called `open_context_menu()` for the selected
library item. `/` on both library kinds entered the generic library-search path
at `src/app/input_lib_keys.rs:292`; for Music, that handler first used the
recursive album-search short-circuit (`open_recursive_album_search`), while TV
used the ordinary library search initialization.

On this branch, `MusicWorkspaceComponent::handle_key` has only the existing
focused-track `.` arm (`src/app/components/music_workspace.rs:278`), and no
`/` arm: album-level `.` and `/` therefore fall through to `_ => None`.
`TvWorkspaceComponent::handle_key` has neither `.` nor `/` arms, so both fall
through to `_ => None`.
