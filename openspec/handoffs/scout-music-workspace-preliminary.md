# Scout 5.3d.19 — Music workspace exact contracts (artifact)

See `openspec/handoffs/scout-music-workspace-preliminary.md` for the authoritative
replacement report. Summary of exact contracts below.

## Raw-key surface (owner: `MusicWorkspaceComponent::handle_key`, music_workspace.rs:142-264)
- Component is the **active** TuiRealm component while a music-group tab is mounted; it
  gets first crack at every key. Unhandled keys → `Msg::Legacy(Key(...))` → legacy
  `App::handle_key_with_home_context` (shell.rs:528).
- Handled keys:
  - `Enter`/`Ctrl+P` with track focused → `MusicTrackActivate`
  - `Enter` (no track, wide) → local `track_cursor=Some(0)` + `Legacy(NoOp)`
  - `Esc`/`Backspace` (track focused) → local `track_cursor=None` + `Legacy(NoOp)`
  - `Up`/`k`/`Down`/`j` (track focused, Library panel) → local `move_track(±1)` + `Legacy(NoOp)`
  - `Ctrl+A` (track focused, Library) → `MusicTrackEnqueue`
  - `.` (track focused, Library) → `MusicTrackContextMenu`
  - `Up`/`k`/`Down`/`j` (album, focused, no track) → `MusicAlbumCursor{Move}` (row stride × album_columns, wrap)
  - `h`/`l` (album, focused, album_columns>1) → `MusicAlbumCursor{Move}` (single step, columns=1, wrap)
  - `Home`/`End` → `MusicAlbumCursor{Jump}` → `album_order.first()/last()`
  - `PageUp`/`PageDown` → `MusicAlbumCursor{Page}` → ±page_rows (clamped, no wrap)
  - Mouse `Down(Left)` → hit-test `wide_music_track_at` / `wide_music_browser_area.left_row_targets`, set `track_cursor`/`album_cursor`; shell ignores `Legacy(Mouse)` (shell.rs:534)
  - all else → `Legacy(Key(to_crossterm_key_event(key)))`

## Projection writers (App fields / methods)
- Album cursor → **only** `libs[lib_idx].nav_stack.last_mut().cursor` via
  `move_music_group_display_cursor` (Move), `jump_music_group_display_cursor` (Jump),
  `page_grouped_album_cursor` (Page) — album_cursor.rs:4/27/48, dispatched shell.rs:571-602.
- Track focus → **never** written to App (local `Option<usize>`); shell reads via
  `Model::focused_music_track` (shell_music_workspace.rs:18-34).
- Track actions resolved at shell: `play_album_track` (actions_navigation.rs:150),
  `enqueue_lib_item` (queue_actions_playlist_mutation.rs:438),
  `open_context_menu_for` (context_menu_actions.rs:607).
- Album tracks = `app.album_tracks_cache: HashMap<album_id, Vec<EmbyItem>>` +
  `album_tracks_loading: HashSet` (app_struct.rs:357-358); populated async by
  `fetch_album_tracks` (images.rs:91). **Legacy branch triggers the fetch** (list.rs:74-80);
  the component does NOT.
- `scroll` written by the renderer (`level.scroll = output.final_scroll`, list.rs:86); the
  component mirrors it back via `set_content` (music_workspace.rs:115).

## Geometry contracts
- `render_wide_music_group_with_ctx` (music_wide.rs:170-271) writes `LayoutMain`:
  `wide_music_area` (music_wide.rs:176), `wide_music_right_area` (200),
  `left_area`/`hero_area` (203/206), `wide_music_art_area` (222),
  `wide_music_track_hitmap`, `wide_music_browser_area`, `left_row_targets`.
- Left layout `wide_music_left_layout` (arrangements/music.rs:7-60) → `WideMusicLeftLayout`
  {hero_area, track_area, art_area, text_area, stack_metadata}.
- `is_wide_music_active()` = `wide_music_right_area` non-zero (layout.rs:185).
- Breakpoint: hero-on-left via `shared_hero_presentation(area)` (list.rs:69); below → narrow,
  `wide_music_area` stays zero, component no-ops.

## Underpaint contracts
- Legacy `App::render_list` wide-music branch (list.rs:66-95) is the **only** producer of
  `wide_music_area`/`wide_music_right_area` before the component view runs. It calls the
  SAME `render_wide_music_group_with_ctx` painter, then the component repaints over it in
  the same frame → legacy is pure underpaint.
- Render seam: `app.render(f)` (legacy branch sets geometry) → `render_music_workspace_component`
  (shell.rs:1127) reads `layout.main.wide_music_area`, calls `application.view` (component
  repaints with local cursor), then `paint_music_image`.
- **Chicken-and-egg (BLOCKER):** component view depends on `wide_music_area` set by the
  legacy branch. Must compute geometry before the component view (U2) before deleting the
  legacy branch (U3).

## Bounded units
U1 mount/idempotent mirror; U2 geometry pre-pass (BLOCKER for U4); U3 delete legacy branch
(after U2); U4 relocate `fetch_album_tracks` trigger (R2); U5 framework teardown + delete
differential legacy test.

## Residual risks
R1 geometry chicken-and-egg (BLOCKER for U4); R2 fetch trigger relocation; R3 Page requires
Library panel focus (matches legacy); R4 one-frame mouse warm-up; R5 h/l single-step vs
arrow row-stride (intended).
