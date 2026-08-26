# Code Context — Scout 5.3d.18: Refine TV workspace into exact typed contract

Read-only inspection of the TV workspace (wide Emby tvshows) component and its
shell/render/legacy keyboard coupling. Deliverable = refined
`openspec/handoffs/scout-tv-workspace.md` with exact typed keyboard surface,
writer seams, geometry contracts, and bounded implementation rows in the
`scout-emby-browser.md` shape.

## Files Retrieved
1. `src/app/components/tv_workspace.rs` (1-380) — `TvWorkspaceComponent`
   fields (38-45), `set_content` mirror-pin (73-132), `handle_key` (166-193),
   `handle_mouse` (204-268), `resolve_hit` (274-312), module tests (333-413).
2. `src/app/shell_tv_workspace.rs` (1-83) — `tv_workspace_component_id` gate
   (9-22), `sync_tv_workspace` (24-66), `render_tv_workspace_component` (69-82).
3. `src/app/components/msg.rs` — `LegacyTerminalEvent` (46-52),
   `ShellRequest` (194), `TvScroll` (405), `TvClick` (415), `TvHit` (649),
   `TvHitRegion` (669). Mouse already typed; keyboard is not.
4. `src/app/render/components/tv_wide.rs` (1-234) — `TvWideRenderCtx` (19-61),
   `render_wide_tv_with_ctx` (102-234), geometry publishing (110-227).
5. `src/app/shell.rs` — `TvScroll`/`TvClick` arms (986-1010),
   `sync_tv_workspace()` per-frame caller (1072), `render_tv_workspace_component` (1126).
6. `src/app/input_lib_keys.rs` — `handle_enqueue_selected_key` (75-79),
   `handle_lib_key` (93-240, the legacy TV key effects).
7. `src/app/input_browse_dispatch.rs` — `activate_selected_series` (163-164),
   `enter_series_selection` gate (317-322).
8. `src/app/lib_cursor_actions.rs` — `current_library_columns==1` in wide TV
   (77-79), `enter_series_selection`→`fetch_series_detail` (317-322).
9. `src/app/mouse_gestures.rs` — `handle_mouse_single/double/right_click_tv`
   (209-251), `handle_mouse_scroll_browse` (78).
10. `src/app/layout.rs` — geometry fields (67,81,133-138), `is_wide_tv_active`
    (197).

## Key Code

### Typed keyboard surface — component handles locally then forwards ALL keys raw
`tv_workspace.rs:166-193`:
```rust
fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
    match key.code {
        Key::Left | Key::Char('h') => self.pane = Pane::Series,
        Key::Right | Key::Char('l') => self.pane = Pane::Episodes,
        Key::Enter if self.pane == Pane::Series => { self.episode_cursor = Some(0); self.pane = Pane::Episodes; }
        Key::Esc | Key::Backspace if self.episode_cursor.is_some() => { self.episode_cursor = None; self.pane = Pane::Series; }
        Key::Up | Key::Char('k') if self.pane == Pane::Episodes => self.move_episode(-1),
        Key::Down | Key::Char('j') if self.pane == Pane::Episodes => self.move_episode(1),
        Key::Char('[') if self.pane == Pane::Episodes => self.move_season(-1),
        Key::Char(']') if self.pane == Pane::Episodes => self.move_season(1),
        Key::Up | Key::Char('k') => self.cursor = move_cursor(self.cursor, -1, self.context.list.item_count()),
        Key::Down | Key::Char('j') => self.cursor = move_cursor(self.cursor, 1, self.context.list.item_count()),
        Key::Char('[') | Key::Char(']') => {}
        _ => {}
    }
    Some(Msg::Legacy(LegacyTerminalEvent::Key(to_crossterm_key_event(key))))  // <-- every key forwarded raw
}
```
Series-list Up/Down is a **dual-write**: component `self.cursor` AND legacy
`handle_lib_key`→`move_lib_cursor_rows` both mutate the same logical cursor.

### Writer seams (component writes no App field; App driven by legacy forward / typed mouse)
- `App::move_lib_cursor_rows` ← legacy Up/Down forward (`input_lib_keys.rs:114-117`).
- `App::enter_series_selection`→`fetch_series_detail` writes `App.series_detail_cache`
  (`lib_cursor_actions.rs:317-322`; cache read `shell_tv_workspace.rs:51`).
- `App::go_back` (`actions_navigation.rs:196`).
- `handle_mouse_*_tv` write `libs[idx].nav_stack.last_mut().cursor` + `set_panel_focus` (`mouse_gestures.rs:209-235`).
- `handle_mouse_scroll_browse` (`:78`).
- **No `push_*` seam exists for TV** (cf Emby `push_emby_browser_content`).

### Geometry contracts (two panes; LEFT=Episodes/hero, RIGHT=Series list)
Published in `render_wide_tv_with_ctx` (`tv_wide.rs:102-234`), read by
`resolve_hit` (`tv_workspace.rs:274-312`):
- `layout.tv_wide_area` (`layout.rs:138`) = `app.layout.main.tv_wide_area`.
- `layout.tv_wide_left_area` (137) — Episodes/hero pane (legacy `left_area` zeroed, `tv_wide.rs:118`).
- `layout.tv_wide_right_area` (133) — Series list pane.
- `layout.tv_wide_list_area` (134) — inner series list rect.
- `layout.tv_wide_season_tabs: Vec<(Rect,usize)>` (136) → `TvHit::SeasonTab`.
- `layout.tv_wide_episode_rows: Vec<(Rect,usize)>` (135) → `TvHit::EpisodeRow`.
- `layout.left_row_map: Vec<Option<usize>>` (67) — series-row click resolution.

### Mount/sync adapter
`Model::sync_tv_workspace` (`shell_tv_workspace.rs:24-66`): gate =
`TabSelection::EmbyLibrary` + `collection_type=="tvshows"` + `is_wide_tv_active()`;
builds `TvWideRenderCtx::new(list, selected_series, series_detail, 0, None, focused, show_letter_pills)`
(hard-coded `season_cursor=0`, `episode_cursor=None` — component re-derives via
mirror-pin) and calls `set_content`. Called every frame from `shell.rs:1072`.

## Architecture
`TvWorkspaceComponent` owns local pane/season/episode cursors and hit-tests its
own painted geometry (mouse fully typed). Keyboard is still raw-forwarded:
`handle_key`→`Msg::Legacy(Key)`→shell legacy bridge→`handle_key_with_home_context`→
`CONTEXT_STACK`→`handle_lib_key`. The series-list cursor is therefore double-written
(component + legacy), synchronized by `set_content`'s `last_mirrored_*` pin (D14 mirror).
`series_detail_cache` is App-owned and read by the shell projection each frame.
This is the exact same shape Emby browser was in before `8929248`/`24e645b9`/`6fa217fb`;
TV is one wave behind (raw-key forward, mirror-pin, per-frame sync, legacy underpaint).

## Start Here
Open `src/app/components/tv_workspace.rs:166` (`handle_key`) and
`src/app/shell_tv_workspace.rs:24` (`sync_tv_workspace`). The Emby template
(`scout-emby-browser.md`, commits `8929248`/`24e645b9`/`6fa217fb`) is the exact
conversion path: T1 typed keyboard → T2 drop mirror-pin → T3 push at writers →
T4 underpaint removal → T5 teardown. Plus the new T6 episode play/enqueue gap.

## Supervisor coordination
No decisions needed from supervisor; constraints respected (read-only, did not
edit tasks.md, no production edits). Findings written to
`openspec/handoffs/scout-tv-workspace.md`.

## Residual Risks
- B1: T2 (drop mirror-pin) must not regress cursor sync — App reader must be
  driveable from component cursor first.
- B2: T3 writer enumeration for `push_tv_workspace_content` is large (nav-stack/
  cache/panel-focus/letter/resize), same coverage risk as Emby.
- B3: T6 episode play/enqueue needs new App methods; episode id source =
  `series_detail.episodes[season_id][episode_cursor]`.
- Legacy wide-TV underpaint (`render_list` branch) must remain until component
  owns geometry (T4), else `tv_wide_*` rects the component hit-tests disappear.
