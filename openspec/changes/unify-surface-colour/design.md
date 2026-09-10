## Context

`proposal.md` carries the audit. What the approach has to fit:

- **The theme is already the only place colours are *defined*.** Raw primitives are private to
  `render/theme/primitives.rs`, roles are public there, and every production site names a role or a
  resolver. That is why the "change once" assurance looked true.
- **What is scattered is the *mapping* from a rendered region to a colour.** Six mechanisms decide it
  today: the shared resolvers (`resolve_surface_focus`, and on the abandoned branch the column/inset
  ones), direct role names (30 production sites), duplicate roles carrying one level's value
  (`SURFACE_ACCENT_SOFT`, `SURFACE_PLAYBACK`), one role carrying six meanings (`SURFACE_BACKDROP`),
  per-screen colour bits (`episode_focused` / `track_active` / `chapter_focused`), and geometry
  owners the screens do not control (the shell's column backdrops, the playback strip).
- **Value aliasing is what makes the scattered mapping undetectable.** `#3c4841` is a focused column
  surface and the strip's fill; `#48584e` is the focused panel and a retired duplicate role;
  `#333c43` is the panel resting colour, the now-playing strip's resting half and the
  `SURFACE_RESTING` role. A rename or retarget therefore moves some surfaces, skips others, and
  leaves every test green, because tests assert roles and a role's value moves with its definition.
- **The picture is the acceptance surface.** Both defects the user found by eye this week
  (the strip's focused colour, the sub-panels that never lit up) were invisible to the test suite.

## Goals / Non-Goals

**Goals:**

- One edit changes one level's appearance everywhere that level is painted, and nowhere else.
- No production screen chooses a colour, a role, or a resolver arm.
- The picture is pinned by tests that name surfaces, not roles: a wrong or missing painter fails.

**Non-Goals:**

- Repainting anything: every surface keeps the colour it has today unless a task says otherwise.
- Replacing the role constants: they stay the values. This change is about the mapping.
- Layout, breakpoints, painter ownership, mouse geometry, or per-screen content.

## Decisions

### D1 — One focus state, computed once

`FocusState { column: Column { Left, Right }, sub: Option<SubSurface> }` is computed once per frame in
the shell (from `PanelFocus`, the visible panel mode, and the active workspace's cursor) and handed to
screens as a fact. `FrameChromeGeometry` stops carrying ad-hoc bits and carries the value; a screen
receives it with its render context.

Today each screen re-derives its own colour bit from its own sub-mode; that derivation is what made
"change the focused panel look" screen-by-screen work.

_Alternative:_ keep per-screen bits and centralise only the colours. Rejected — the bit is the
decision; centralising the colour alone is what the abandoned change did.

### D2 — One surface table: `(Surface, &FocusState) -> SurfaceColors`

The theme gains a closed `Surface` identity per rendered region — column surfaces, panels, inset
boxes, the dialog frame, the recess rows — and one function mapping it plus the focus state to a
`SurfaceColors { fill, border }`. One row per surface, each row carrying its nesting level. Roles
stay as the values, so a level's appearance is still changed by editing one role, and the table
proves which surfaces that reaches.

The levels are, shallowest first: **column/pane** — the surface a panel sits on, including the column
gutters the shell paints and the hero pane fill; **panel** — a focusable panel body (queue panel,
library panel, a screen's episode/track/chapter box, a hero content box, a dialog's list); **recess**
— a non-focusable inset inside a panel (the now-playing panel's own rows and status pills, the
visualizer background); **chrome band** — non-focusable structural chrome that never follows panel
focus (the tab bar, the status bar, a column's header/status rows, the pill row, the now-playing
panel's status pills band); **dialog** — an overlay frame and its dim backdrop, which never follow a
panel. A selected row is not a level of its own: it is a hole in its panel through which the
containing column surface shows, so it takes the column/pane level wherever the row sits.

A surface's level is what an edit to that level reaches, so two surfaces at the same level share
their appearance and shall not be given separate ones. The chrome band level was added after task
2.1's inventory showed bands (tab bar, status bar, queue header/status rows, sidebar chrome) that none
of the original four levels described.

A `Surface` identity is **structural, not screen-shaped**: it names a position in the layout — a
column gutter, a panel body, a pane container, a pane's content box, a recess, a chrome band, a popup
frame — so the same position in a different screen or provider is the same identity and the same
table row. What a surface holds (a series list, a track list, chapters) is content, never a surface
variant: `TvEpisodeBox` / `MusicTrackBox` / `HeroContentBox` were the wrong shape, because each would
force enum churn for a new screen while the three are one position resolving to one appearance. A
screen may not invent an identity; a genuinely different appearance is a level difference or a named
variant, not a new row.

Names SHALL use the domain vocabulary in `CONTEXT.md`: `Panel` is Library's or Queue's, so a sidebar
has a body and bands rather than panels; the overlay term is **popup**, not dialog; the pane inset is
the **Main content box**; and the inline-hero avoid-list ("separate detail block", "stacked hero")
rules out naming a selected row's detail a block of its own.

_Alternative:_ a registry keyed by strings/ids so screens can add surfaces. Rejected — the closed
enum is what makes "every surface is in the table" checkable.

### D3 — Screens name surfaces, never roles or resolver arms

A production paint site calls the table with its `Surface` and the focus state it was given. It does
not name `SURFACE_*`, does not call a resolver, and does not branch on a focus bit to choose a
colour. Branches on focus stay only for behaviour (selection gating, cursor, hit geometry).

### D4 — Two guardrails, both failing the build

- An ast-grep rule fails on surface role names or `resolve_surface_*` calls in production screens
  outside `src/app/render/theme/` (tests excluded).
- A conformance test enumerates every `Surface` × breakpoint × focus state from the same table the
  production code uses and asserts the rendered rect's fill equals the table's value. The table is
  therefore the inventory: a surface with no painter, or a painter the table does not know, fails.

_Alternative:_ a written inventory only. Rejected — a document cannot fail a build, and the abandoned
change's task-1.1 classification (a grep-derived table) is exactly how the four defects were missed.

### D5 — The row above the pill bar belongs to the library column (the one behaviour kept)

`chrome_geometry.player_area` is four rows; only the first three are the playback panel's content.
The fourth is the library column's first content row and must render the library column surface, not
the panel's recess. `QueueOnly` keeps the row as the panel's own (its "On Now" title lives there);
the mini-view discriminator is `narrow_player`, i.e. `effective_panel_mode() == PanelMode::QueueOnly`.
This is the only behaviour salvaged from `focus-aware-column-surface`.

The ownership is **geographic, not incidental**: the row is the library column's first content row,
not "the row the panel happens to leave free". A layout that no longer paints the playback panel in
the wide layouts therefore changes nothing about this row — it is already the column's — and the
panel's own three content rows above it are that layout's business, not this change's. The buffer
test asserts the row's own fill, so the invariant survives the panel being removed.

_Note for the migration in section 4_: the row's focused arm resolves to today's `SURFACE_FOCUSED`
while the resting arm is `SURFACE_BACKDROP` (`#2d353b`), deliberately **not** the panel resting role
(`SURFACE_RESTING`, `#333c43`) that the neighbouring queue arm uses. Substituting the panel resolver
here would silently repaint the resting column; the table's column entry is the fix, and until then
the site names its decision explicitly.

_Accepted consequence (reviewed, section 1)_: the backdrop rect spans the whole right column, so this
also makes the column's entire gutter follow focus, not only the row above the pill bar — in wide
`LibraryOnly`, where focus is always the library, the column gutter is permanently `#3c4841`. That is
the column-surface behaviour itself and was accepted as a wider visible delta than the row alone; the
buffer tests pin it at wide `LibraryOnly`, wide `Both` library-focused and wide `Both` queue-focused.

## Risks / Trade-offs

- **[A big mechanical migration can drift the picture]** → every migrated site's expected value is
  today's value; the conformance test pins the picture surface-by-surface, and the change lands in
  screen-sized units so any drift is one screen wide and reviewable.
- **[The closed `Surface` enum can lag a new region]** → the ast-grep rule fails on any role name in a
  screen, so a new region cannot choose a colour at all; it must add a `Surface` row.
- **[Value aliasing can survive inside the table]** → two surfaces may still share a value; that is
  fine (a value is not a level). The table states each surface's level, so a level's change is one row
  group, and the audit of shared role values is a row in the tasks.

## Migration Plan

Sections in `tasks.md` land in order, each as its own reviewable unit: the kept behaviour, the
inventory, the focus state, the table plus per-screen migration, the guardrails, then the gates. No
persistence, protocol, or config. Rollback is a revert.

## Open Questions

- Whether `FocusState` should also carry the mouse-hover surface once ADR 0024's hit ownership is
  revisited. Deferrable: hover is not a colour input today.

## Surface inventory (task 2.1)

Every production paint site that writes a surface colour (a background fill) or a surface border,
classified against D2's four levels. This is the migration checklist for section 4 and the
coverage record for section 5's guardrails.

The interactive-ownership ledger (`docs/architecture/interactive-surface-ledger.md`) already
answers *who owns and paints* each surface per breakpoint, and is cited here rather than
duplicated: every row below sits on a ledger row's painter. The ledger does **not** answer
*surface identity, nesting level, or colour decision* — that is this table's job.

### Coverage note

Primary enumeration (verbatim):

```text
rg -n "SURFACE_FOCUSED|SURFACE_RESTING|SURFACE_ACCENT_SOFT|SURFACE_BACKDROP|SURFACE_PLAYBACK|SURFACE_ITEM_FOCUSED|SURFACE_CHROME|SURFACE_STATUS_PILL|SURFACE_ARTWORK_PLACEHOLDER|resolve_surface_focus" src/app --type rust
```

222 hits at `23ddf05a`. Second pass, for fills and borders the primary command misses:

```text
rg -n "bg\(palette::" src/app --type rust
rg -n "\.borders\(|BorderType" src/app --type rust
rg -n "\.set_bg\(|set_style\(" src/app --type rust
rg -n "list_selected_row_bg|library_column_surface" src/app --type rust
rg -n "render_widget" src/app --type rust -A 6 | rg "bg\(palette"
```

The second pass is what found the sites that paint a surface without naming a surface role
(`BORDER_UNFOCUSED` used as a fill, `PILL_ROW_BG`/`PILL_SELECTED_BG`, `ACCENT_ACTIVE`,
`SURFACE_SIDEBAR`, and the one raw `Color::Rgb` in a production painter). Where a site is a
call into a shared painter (a modal caller passing its frame's `bg`, a screen calling
`selected_detail_shell`), the row names the **owner painter's** write site and lists the
callers in the `rect source` cell.

**Excluded, with the reason:**

- `src/app/render/theme/**` — `theme/mod.rs` holds the role definitions and the resolver under
  construction; `theme/primitives.rs` has no hits. This module is the target, not a site.
- `src/app/palette.rs` and `src/app/render/mod.rs` — the two re-export modules; they name roles
  in `use` lists only.
- `#[cfg(test)]`-gated code: every `*tests*.rs` / `tests_*.rs` file, plus the inline test modules
  that made the primary grep noisy — `media_list.rs:46,109,146,176,264,328,357,369`
  (`mod wide_row_regression_tests`), `wide_hero.rs:343` (`mod wide_hero_hero_pane_tests`),
  `visualizer.rs:182`, `card.rs:670`, `playback.rs:241,296`, and the test-only painter
  `home_video.rs:72-74` (`render_home_video_item` is `#[cfg(test)]`-gated at `home_video.rs:57`).
- `src/local_daemon.rs` — 152 lines, no paint call of any kind (`render_widget`, `Style::`,
  `palette::`, `bg(` all absent).

### Sites that paint no colour

Named here because the primary grep returns them and they are deliberately **not** inventory rows:

- `render_selected_block_borders`' `Framed` arm (`widgets.rs:173,179`) — foreground glyphs only;
  its `FocusedRail` arm does carry a surface bg and is a row.
- `render_panel_row` (`chrome.rs:350`), `render_placeholder` (`widgets.rs:501`),
  `render_count_label` (`widgets.rs:266`), `render_sidebar_scrollbar` (`chrome.rs:320`),
  `render_tabs`' overflow glyphs (`chrome_tabs.rs:63`), `shell_draw.rs:23` — foreground text/glyph
  styling only, no fill or border.
- `detail.rs:408` — a prose comment naming a role; no rendered value.
- `home.rs:379`, `media_list/wide_row.rs:30-31`, `widgets.rs:20-31`, `list_rows.rs:331` — doc
  comments naming roles; no rendered value.
- `render_visualizer`'s per-sample points (`visualizer.rs:44`) — a foreground glyph on the
  background its caller resolves; the background itself is row 35.
- The arrangement functions (`wide_hero_split`, `wide_hero_slots`, `pill_bar_areas`,
  `wide_library_panes`, `compute_frame_layout`) — geometry only.
- Media-indicator chips (`indicators.rs:110-120,255-275`) — a fill, but driven by per-item
  indicator colours, not by a surface role; out of scope until D2 names an indicator surface.

### Inventory

`level` is D2's nesting level the surface *is*, not the role it names today. One row per site;
sites are grouped by surface identity in reading order (column/pane, then panel, then recess, then
dialog, then the punch-through rows). The final column is the proposed closed-enum name; the table
after the inventory is that name set, deduplicated.

| file:line | surface identity | level | owner painter (fn) | rect source | focus input | role(s)/resolver named today | proposed `Surface` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `render/components/chrome.rs:53` | queue column gutter | column/pane | `render_legacy_backdrops` | `FrameChromeGeometry.left_area` minus the 1-col boundary col (`arrangements/chrome.rs`) | `queue_focused` (`PanelFocus::Queue`) | `resolve_surface_focus(queue_focused)` | `QueueColumn` |
| `components/queue_boundary.rs:121` | queue column gutter (1-col drag strip) | column/pane | `QueueBoundaryComponent::view` | `layout.main.queue_boundary_area` synced by `Model::sync_queue_boundary` (`shell_library.rs:194`) | `PanelFocus::Queue` | `resolve_surface_focus(self.focused)` | `QueueColumn` |
| `render/components/chrome.rs:61` (decision `chrome.rs:29/31`) | library column gutter | column/pane | `render_legacy_backdrops` → `library_column_surface` | `FrameChromeGeometry.right_full_area` | `right_focused` (`right_visible && PanelFocus::Library`) | `SURFACE_FOCUSED` / `SURFACE_BACKDROP` in the transitional helper row 4.2 owns | `LibraryColumn` |
| `components/wide_hero_boundary.rs:121` | wide hero split gap gutter | column/pane | `WideHeroBoundaryComponent::view` | `Model::wide_hero_boundary_geometry()` (`shell_library.rs`) → the component's synced area | none | `SURFACE_BACKDROP` | `WideSplitGutter` |
| `render/arrangements/wide_hero.rs:302-306` | hero pane fill | column/pane | `wide_hero_hero_pane` | `wide_hero_presentation().hero` (the arrangement's own split) | `LeftPaneFocus::ReadOnly` / `Workspace(held)` | `SURFACE_RESTING` / `resolve_surface_focus(held)` | `HeroPane` |
| `render/components/widgets.rs:151` | selected block background (generic punch-through) | column/pane | `render_selected_block_background` | caller's rect | caller's `bg` argument | caller-supplied | `SelectedRow` |
| `render/components/list_rows.rs:341,354,383` | selected media row/cell (legacy list rows) | column/pane | `build_list_row_spans` / `item_cell_spans` | list cell rect | none | `list_selected_row_bg()` → `SURFACE_BACKDROP` | `SelectedRow` |
| `render/components/list_rows.rs:469` | selected marker gutter | column/pane | `draw_column_selection_markers` | `content_area` + selected row | none | `list_selected_row_bg()` | `SelectedRow` |
| `render/components/media_list/wide.rs:202-203` → `wide_row.rs:199` | selected media row (WideMediaList) | column/pane | `selected_row_surface_color` → `wide_media_row` | retained row paint area | `focused` + `SelectedRowSurface` policy | `list_selected_row_bg()` / `resolve_surface_focus(focused)` | `SelectedRow` |
| `render/components/media_list/wide.rs:268` | selected grid cell (GridMediaList) | column/pane | `render_grid_media_list_component` | cell rect | `policy.focused()` | `list_selected_row_bg()` | `SelectedRow` |
| `render/arrangements/wide_hero.rs:415-419` | library panel body (wide-hero rail) | panel | `wide_hero_browser_border` | `wide_hero_browser_pane(..).list_panel` | `focused` bit | `resolve_surface_focus` | `LibraryPanel` |
| `render/arrangements/wide_hero.rs:422` (bg at `widgets.rs:187`) | library panel frame rows (▔/▁) | panel (border of a panel) | `wide_hero_browser_border` → `render_selected_block_borders` | `list_panel` | `focused` | `resolve_surface_focus` (as border fg/bg) | `LibraryPanel` |
| `components/browser/paint.rs:108-110` | library panel body (wide Movies/home-video rail) | panel | `BrowserComponent::render_wide_movies` | `list_panel` | `self.focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/tv_wide.rs:312` | library panel body (wide TV series rail) | panel | `render_wide_tv_with_ctx` | `list_panel` | `right_focused` (pane bit) | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/music_wide.rs:581` | library panel body (wide Music album rail) | panel | `render_wide_music_group_with_ctx` | `list_panel` | `right_focused` (pane bit) | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/audiobookshelf_book.rs:197` | library panel body (ABS book rail) | panel | `render_audiobookshelf_book_content` | `list_panel` | `rail_focused` (sub-focus bit) | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/audiobookshelf_podcast.rs:264` | library panel body (ABS podcast show rail) | panel | `render_audiobookshelf_podcast_content` | `list_panel` | `focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/feeds.rs:211` | library panel body (wide Feeds rail) | panel | `render_feeds_content` | `list_panel` | `focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/home.rs:370-372` | library panel body (Home wide list panel) | panel | `render_home_content` | `green_panel_full` (the screen's own two-column split) | `focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/widgets.rs:237-241` | queue panel body | panel | `render_queue_panel_frame` | `queue_geometry.panel_area` ← `queue_panel_geometry(left_content)` (`shell_draw.rs`) | `queue_focused` | `SURFACE_ACCENT_SOFT` / `SURFACE_BACKDROP` | `QueuePanel` |
| `render/components/tv_wide.rs:515` | TV episode box | panel | `render_tv_series_selection` | `wide_hero_hero_content_box` slot placement | `episode_focused` (sub-focus bit) | `SURFACE_ACCENT_SOFT` | `TvEpisodeBox` |
| `render/arrangements/wide_hero.rs:468` (variant chosen in `music_wide.rs`) | Music track box | panel | `wide_hero_hero_content_box_with_surface` | caller's `track_area` | `left_focused` (`track_active`) | `SURFACE_ACCENT_SOFT` | `MusicTrackBox` |
| `render/arrangements/wide_hero.rs:467` | wide-hero content box (generic inset) | panel — D2 names "a hero content box" a panel, but it renders as a plain inset | `wide_hero_hero_content_box` | caller's area | none | `SURFACE_BACKDROP` | `HeroContentBox` |
| `render/components/hero.rs:192-196` | inline selected-detail block | panel | `selected_detail_shell` | caller `hero_area`; callers `audiobookshelf_book.rs:296`, `audiobookshelf_podcast.rs:336`, `feeds.rs:268`, `home.rs:477`, `list_narrow.rs:219`, `music_wide.rs:427` | `focused` | `resolve_surface_focus` | `SelectedDetailBlock` |
| `shell_playback.rs:48-50` | now-playing (playback) panel body | panel | `render_playback_component` → `render_player_panel` | `FrameChromeGeometry.player_area` | `PanelFocus::Queue` | `SURFACE_FOCUSED` / `SURFACE_PLAYBACK` | `PlaybackPanel` |
| `components/playback.rs:55` | PlaybackComponent projection panel bg | panel | `PlaybackComponent` (projection value) | n/a — value, not a rect | `PanelFocus::Queue` | `SURFACE_PLAYBACK` | `PlaybackPanel` |
| `shell_draw.rs:286` | queue-only wide playback panel body | panel | `render_main` | local `panel_area` from `left_content` + card | none (`QueueOnly`) | `SURFACE_CHROME` | `PlaybackPanel` |
| `shell_draw.rs:297,316` | queue-only playback panel context bg | panel | `render_main` → `render_player_panel` | local `panel_area` | none | `SURFACE_CHROME` | `PlaybackPanel` |
| `render/components/chrome.rs:188-191` | sidebar panel body (help/settings/search/playlists/sessions) | panel | `render_panel_shell_at` | `panel_shell_rect(sidebar)` | `style` variant (not focus) | `SURFACE_RESTING` / `SURFACE_SIDEBAR` | `SidebarPanel` |
| `render/components/chrome_player.rs:52,65,93,109,201,410` | now-playing panel content rows (seekbar/title/blank) | recess | `render_player_panel` / `render_seekbar` / `render_title_row` | `ctx.area` rows | via `ctx.panel_bg` | `ctx.panel_bg` | `PlaybackRecess` |
| `render/components/chrome_player.rs:126,162` | now-playing bottom row ("On Now" row) | recess — narrow `QueueOnly` only; in the wide layouts this row is the library column's and is row 3 above | `render_player_panel` | `ctx.area` row +3 | `narrow_player` decides panel-vs-column, not a focus bit | `SURFACE_BACKDROP` | `PlaybackBottomRow` |
| `render/components/chrome_player.rs:243` (applied `:277,283,301`) | now-playing status pill | recess | `render_title_row` | title row's right segment | none | `SURFACE_BACKDROP` | `PlaybackStatusPill` |
| `render/components/chrome_status.rs:305` | status bar body | recess — a chrome band with no D2 level (see Findings) | `render_status_bar` | `FrameChromeGeometry.status_area` | none | `SURFACE_CHROME` | `StatusBar` |
| `render/components/chrome_status.rs:28,63,66,76,128,136,139,149,161,166,169,173,178,180,192,197,200,218,223,229,469,481,487` | status bar pills/chips | recess | `render_status_bar` pill builders | within `status_area` | none | `SURFACE_STATUS_PILL` | `StatusPill` |
| `render/components/queue.rs:166` | queue panel title row | recess | `render_queue_title_content` | QueueComponent's title row | none | `SURFACE_CHROME` | `QueuePanelChrome` |
| `render/components/queue.rs:175,197` | queue scope pill (local) | recess | `render_queue_title_content` | `local_area` | none | `SURFACE_CHROME` | `QueuePanelChrome` |
| `render/components/queue.rs:219,231` | queue scope target row (remote) | recess | `render_queue_title_content` | `target_area` | none | `SURFACE_CHROME` | `QueuePanelChrome` |
| `render/components/queue.rs:297` | queue panel status strip | recess | `render_queue_status` | `queue_geometry.pill_row` | none | `SURFACE_CHROME` | `QueuePanelChrome` |
| `render/components/card.rs:245-249` (fill `visualizer.rs:16,20,44`) | queue card visualizer background | recess | `App::render_visualizer` (decision in `render_card_visualizer`) | `card_reserved_rect` | `PanelFocus::Queue` | `resolve_surface_focus` | `QueueCardVisualizer` |
| `render/components/artwork_placeholder.rs:9` | artwork placeholder | recess | `render_artwork_placeholder` | caller's artwork slot | none | `SURFACE_ARTWORK_PLACEHOLDER` | `ArtworkPlaceholder` |
| `render/components/card.rs:111` | artwork loading placeholder (queue card) | recess | `render_card_image` | `card_reserved_rect` | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/album_art.rs:183` | artwork loading placeholder (inline album art) | recess | `render_inline_art_cell` | `img_rect` | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/detail_series_view.rs:125` | artwork loading placeholder (series hero) | recess | series detail painter | `result.img_rect` | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/home_hero_emby.rs:121,272,286` | artwork loading placeholder (Emby hero) | recess | Emby hero painter | image area | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/widgets.rs:391,403,489` | pill selector row background | recess | `render_pill_bar` | `pills_area` | none | `PILL_ROW_BG` | `PillRow` |
| `render/components/hero.rs:666,669` | pill selector row background (hero shell) | recess | hero pill row painter | pill row | none | `PILL_ROW_BG` | `PillRow` |
| `render/components/widgets.rs:254,256` (applied `:446-468`) | pill chip (selected/unselected) | recess | `selector_pill_style` / `render_pill_bar` | pill rect | `selected` | `PILL_SELECTED_BG` / `PILL_BG` | `PillChip` |
| `render/components/search_sidebar.rs:161` | search sidebar selected chip | recess | search sidebar painter | chip rect | `selected` | `PILL_SELECTED_BG` | `PillChip` |
| `render/components/context_menu.rs:38` | context menu selected row | recess | context menu painter | row rect | `selected` | `ACCENT_ACTIVE` | `ContextMenuRow` |
| `render/components/chrome.rs:219,221,246,248,255,280,282,299,311` | sidebar panel header/footer rows | recess | `render_panel_shell_at` | header/footer rects | `style` variant | `SURFACE_CHROME` / `SURFACE_ITEM_FOCUSED` / `SURFACE_RESTING` | `SidebarChrome` |
| `render/components/chrome_tabs.rs:43` | tab bar background | recess — chrome band, no D2 level | `render_tabs` | `FrameChromeGeometry.tab_bar_area` | none | `SURFACE_CHROME` | `TabBar` |
| `render/components/chrome_tabs.rs:131` | tab bar inactive tab glyph | recess | `render_tabs` | tab row cell | none | `Color::Rgb(73, 81, 86)` raw (see Findings) | `TabBar` |
| `render/components/modal_frame.rs:46` | dialog frame body | dialog | `render_modal_frame_inner` | centered rect from `f.area()` | none | caller's `bg` | `DialogFrame` |
| `render/components/confirm_modal.rs:29` | dialog frame body | dialog | `render_confirm_modal_content` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/daemon_lost_modal.rs:29` | dialog frame body | dialog | `render_daemon_lost_modal_content` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/feeds_manage.rs:60` | dialog frame body (feed list) | dialog | `render_feeds_manage_list` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/feeds_manage.rs:171` | dialog frame body (feed form) | dialog | `render_feeds_manage_form` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/library_routes.rs:156` | dialog frame body | dialog | `render_library_routes_content` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/multiselect.rs:53` | dialog frame body | dialog | `render_multiselect_content` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/playlists.rs:41` | dialog frame body (save/rename) | dialog | `render_save_playlist_content` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/remote_reanchor.rs:31` | dialog frame body | dialog | `render_remote_reanchor_popup_content` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/selection_modal.rs:65` | dialog frame body | dialog | `render_selection_modal_content` | modal rect | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/selection_modal.rs:96` | dialog list spacer | dialog | `render_selection_modal_content` | inner rect + filter height | none | `SURFACE_FOCUSED` | `DialogFrame` |
| `render/components/backdrop.rs:dim_backdrop` | modal dim backdrop | dialog | `dim_backdrop` | `f.area()` | none | shades every existing cell (no role) | `DimBackdrop` |

### Proposed `Surface` name set

The closed enum D2 needs, one name per distinct identity in the table above. Level is the enum's
only style input: every name at one level resolves to that level's `SurfaceColors`.

| proposed `Surface` | level | distinct identities covered |
| --- | --- | --- |
| `QueueColumn` | column/pane | queue column gutter, queue boundary strip |
| `LibraryColumn` | column/pane | library column gutter |
| `WideSplitGutter` | column/pane | wide hero split gap |
| `HeroPane` | column/pane | wide hero pane fill |
| `SelectedRow` | column/pane | selected row/cell/marker punch-through, generic selected block |
| `LibraryPanel` | panel | wide-hero rail body + frame, Home two-column list panel |
| `QueuePanel` | panel | queue panel body |
| `TvEpisodeBox` | panel | TV episode box |
| `MusicTrackBox` | panel | Music track box |
| `HeroContentBox` | panel | generic wide-hero content inset |
| `SelectedDetailBlock` | panel | inline selected-detail block (six screens) |
| `PlaybackPanel` | panel | now-playing panel body (right column and queue-only) |
| `SidebarPanel` | panel | sidebar panel body (help/settings/search/playlists/sessions) |
| `PlaybackRecess` | recess | now-playing panel content rows |
| `PlaybackBottomRow` | recess | narrow "On Now" row |
| `PlaybackStatusPill` | recess | now-playing status pill |
| `StatusBar` | recess | status bar body |
| `StatusPill` | recess | status bar pills/chips |
| `QueuePanelChrome` | recess | queue title row, scope pills, scope target, status strip |
| `QueueCardVisualizer` | recess | queue card visualizer background |
| `ArtworkPlaceholder` | recess | artwork placeholder |
| `ArtworkLoadingPlaceholder` | recess | artwork loading placeholder (five painters) |
| `PillRow` | recess | pill selector row background |
| `PillChip` | recess | selected/unselected pill chip, context-menu row |
| `ContextMenuRow` | recess | context menu selected row |
| `SidebarChrome` | recess | sidebar panel header/footer rows |
| `TabBar` | recess | tab bar background and its inactive-tab glyph |
| `DialogFrame` | dialog | modal/dialog frame body and its list spacer |
| `DimBackdrop` | dialog | modal dim backdrop |

### The six mechanisms, site by site

**1. Shared resolvers.** `resolve_surface_focus(focused)` (theme/mod.rs:80) is called at 15
production call sites in 14 production files: `browser/paint.rs:108`, `queue_boundary.rs:121`,
`wide_hero.rs:303,415`, `audiobookshelf_book.rs:197`, `audiobookshelf_podcast.rs:264`,
`card.rs:245`, `chrome.rs:53`, `feeds.rs:211`, `hero.rs:192`, `home.rs:370`,
`media_list/wide.rs:203`, `music_wide.rs:581`, `tv_wide.rs:312`, `widgets.rs:187`.
(`media_list/wide_row.rs:31` and `home_video.rs:72` also match the grep but are a doc comment and a
`#[cfg(test)]`-gated painter respectively, so neither is a production call site.)
`list_selected_row_bg()` (theme/mod.rs:95) is a second
resolver with 7 production call sites (`list_rows.rs:341,354,383,469`, `media_list/wide.rs:202,268`,
`media_list/grid` via `wide.rs:268`); it is a hard-coded alias for `SURFACE_BACKDROP`, so a
retarget of that role moves every selected row in the app. `chrome.rs`'s private
`library_column_surface` (chrome.rs:27) is a third, transitional resolver: it is the one place the
column level is named, and row 4.2 owns it.

**2. Direct role names.** 73 production code lines name a `SURFACE_*` role directly, across 23
production files (command and count in task 2.2's reconciliation). They are every remaining row of
the inventory whose `role(s)` cell names a role rather than a resolver: the modal frame's ten
callers, the `SURFACE_BACKDROP` rows, the `SURFACE_CHROME`/`SURFACE_ITEM_FOCUSED`/`SURFACE_SIDEBAR`
sidebar and queue-chrome rows, and the `SURFACE_ACCENT_SOFT` panel rows.

**3. Duplicate roles carrying one level.**

- `SURFACE_ACCENT_SOFT` (`#48584e`) is the panel level's focus fill at three production sites:
  `widgets.rs:237` (queue panel), `tv_wide.rs:515` (TV episode box), `wide_hero.rs:468` (Music
  track box). `SURFACE_FOCUSED` (`#3c4841`) is the same level's focus fill at every other panel
  site, so one level has two values and a role edit reaches one group or the other, never both.
  `SURFACE_ACCENT_SOFT` has zero `resolve_surface_focus` call sites.
- `SURFACE_PLAYBACK` (`#333c43`) is the now-playing panel/recess value while `SURFACE_RESTING`
  (`#333c43`) is the resting panel and resting column value — one value spanning three levels, so
  a retarget of either moves surfaces at levels the other does not own.

**4. The meanings of `SURFACE_BACKDROP` (`#2d353b`).** Nine distinct rendered meanings at eight
direct production paint sites plus the `list_selected_row_bg` alias: (a) library column gutter
(`chrome.rs:31`); (b) wide hero split gap (`wide_hero_boundary.rs:121`); (c) wide Music browser
containing panel (`music_wide.rs:549`); (d) wide-hero content box (`wide_hero.rs:467`); (e)
unfocused queue panel (`widgets.rs:239`); (f) narrow "On Now" row and its title (`chrome_player.rs:126,162`);
(g) now-playing status pill (`chrome_player.rs:243`); (h) pill-bar spacer / library column first
content row (`home.rs:402`); (i) selected-row punch-through via `list_selected_row_bg()`
(`theme/mod.rs:96`). The audit's visualizer-background meaning is **not** a production
`SURFACE_BACKDROP` site — the only such call is `visualizer.rs:182`, a test; production resolves
focus at `card.rs:245`.

**5. Per-screen colour bits.** The sites whose colour input is a screen-local bit rather than a
shell fact:

- `episode_focused` — `tv_wide.rs:515` (TV episode box panel), and `tv_wide.rs:262`'s
  `LeftPaneFocus::Workspace(ctx.focused && ctx.episode_cursor.is_some())` (hero pane).
- `track_active` — `music_wide.rs`'s `left_focused = ctx.focused && track_active`, feeding both
  `wide_hero_hero_pane` and the track-box variant (`wide_hero.rs:468`).
- `chapter_focused` — `audiobookshelf_book.rs`'s `focused && interaction.chapter_focused`, feeding
  the hero pane; `rail_focused = focused && !chapter_focused` feeds the rail panel.
- `episode_focused` (podcast) — `audiobookshelf_podcast.rs`'s `LeftPaneFocus::Workspace(focused && interaction.episode_focused)`.
- The pane bit `LeftPaneFocus` (`wide_hero.rs:288`) is the pane-level analogue of
  `FocusState.column`, not a sub-surface bit; D1 must keep a pane fact of this shape.
- `narrow_player` — `chrome_player.rs:126` uses `narrow_player` (i.e. `PanelFocus` + visible panel
  mode) to decide *whether* the panel paints the bottom row at all. That is a mode discriminator
  standing in for rect ownership (see Findings).

**6. Colour decided by geometry the screens do not own.**

- The shell's column backdrops: `chrome.rs:53` (left) and `chrome.rs:61` (right), painted by
  `render_legacy_backdrops` from `FrameChromeGeometry`, consumed by no screen.
- The queue boundary strip: `queue_boundary.rs:121`, painted by a mounted component from a shell
  rect, not by the queue panel that visually contains it.
- The wide split gap: `wide_hero_boundary.rs:121`, painted by `WideHeroBoundaryComponent` from
  shell-derived geometry.
- The now-playing strip: `shell_playback.rs:48-50` decides the panel bg and `render_player_panel`
  paints three rects the screen never builds (`FrameChromeGeometry.player_area`).
- The row above the pill bar: in the wide layouts the library column owns it (`chrome.rs:61`, row
  1.3's outcome); only `QueueOnly` (`narrow_player`) paints it as the panel's recess. Two owners,
  one row, discriminated by a mode flag.

### Findings

No fixes here; these are what section 4 and the section 6 report must resolve or record.

1. **Multi-owner identity — library/rail panel body.** `wide_hero_browser_border`
   (`wide_hero.rs:415-419`) fills `list_panel` *and* six screens fill the same rect immediately
   before calling it (`browser/paint.rs:110`, `tv_wide.rs:312`, `music_wide.rs:581`,
   `audiobookshelf_book.rs:197`, `audiobookshelf_podcast.rs:264`, `feeds.rs:211`). Two painters
   write one surface; the values agree today only because both resolve the same focus bit.
2. **Multi-owner identity — TV episode box.** `wide_hero_hero_content_box` paints it
   `SURFACE_BACKDROP` (recess) and `render_tv_series_selection` repaints it `SURFACE_ACCENT_SOFT`
   (panel) when focused. The surface's level depends on which painter ran last, not on a declared
   level.
3. **Multi-owner row — the row above the pill bar.** Painted by `chrome.rs:61` in wide layouts and
   by `chrome_player.rs:126` under `narrow_player`. Rect ownership is expressed as a mode flag, so
   a new layout that paints the panel can silently double-paint or drop the row. Row 1.3's fix is
   the geographic one; the flag still lives in `render_player_panel`.
4. **Screens choosing colour.** The ten modal callers pass `SURFACE_FOCUSED` into
   `render_modal_frame`; `tv_wide.rs:515` picks `SURFACE_ACCENT_SOFT`; `music_wide.rs` picks the
   `WideHeroContentBoxSurface` variant; `home.rs:402` picks `SURFACE_BACKDROP`; `card.rs:245` picks
   the visualizer bg. D3 forbids all of these.
5. **Two roles for one visible level.** `SURFACE_ACCENT_SOFT` vs `SURFACE_FOCUSED` (panel);
   `SURFACE_STATUS_PILL` aliases `SURFACE_CHROME`; `SURFACE_ARTWORK_PLACEHOLDER` aliases
   `SURFACE_BACKDROP` (`#2d353b`); `BORDER_UNFOCUSED` is a border role used as a fill at six call
   sites across four artwork-loading painters (`card.rs:111`, `album_art.rs:183`,
   `detail_series_view.rs:125`, `home_hero_emby.rs:121,272,286`); `SURFACE_RESTING` = `SURFACE_PLAYBACK` (one value, two levels);
   `SURFACE_FOCUSED` = `TEXT_ACCENT_MUTED` (`BG_GREEN`), a surface role aliasing a text role.
6. **Raw colour in a production painter.** `chrome_tabs.rs:131` writes
   `Style::default().fg(Color::Rgb(73, 81, 86))` for the tab bar's inactive glyph column. Every
   other production colour goes through a role; this one is a hue chosen in a screen-adjacent
   painter, exactly the bypass `theme/primitives.rs` privacy is meant to prevent.
7. **D2's four levels do not cover the chrome bands.** The tab bar (`chrome_tabs.rs:43`), the
   status bar (`chrome_status.rs:305`), the queue panel's title/status rows (`queue.rs:166,297`)
   and the sidebar header/footer (`chrome.rs:219-311`) are surfaces but are neither a column, a
   focusable panel, an inset recess, nor a dialog. They are assigned `recess` above for want of a
   fifth level; their real role is `SURFACE_CHROME`/`SURFACE_ITEM_FOCUSED`, which the recess level
   must not take over.

## Audit reconciliation (task 2.2)

Each number `proposal.md` asserts, re-measured against the current tree (`23ddf05a`). Where the
number differs, `proposal.md` is corrected in place; the command and the retrieved number are
recorded here so the correction is reproducible.

| claim in `proposal.md` | command run | measured at `23ddf05a` | disposition |
| --- | --- | --- | --- |
| "≈18 files via the shared lever" | production `resolve_surface_focus(` call sites after removing comment-only lines, `theme/`, the two re-export modules and `#[cfg(test)]` ranges | **15 call sites in 14 files** (raw `rg -l` gives 16: `media_list/wide_row.rs` is a doc comment, `home_video.rs:72` is `#[cfg(test)]`-gated) | CORRECTED in `proposal.md` to 14 files |
| "30 direct role-name production sites" | 73 production code lines matching `SURFACE_[A-Z_]+` after removing comment-only lines, `theme/`, the two re-export modules and `#[cfg(test)]` ranges (brace-matched) | **73 lines in 23 files** | CORRECTED in `proposal.md` to 73 lines in 23 files |
| "3 `SURFACE_ACCENT_SOFT` production sites" | `rg -n "SURFACE_ACCENT_SOFT" src/app --type rust -g '!**/theme/**' -g '!src/app/palette.rs' -g '!src/app/render/mod.rs' -g '!**/tests*' -g '!**/*_tests.rs'` | **3** — `widgets.rs:237`, `tv_wide.rs:515`, `wide_hero.rs:468` | CONFIRMED (correct in `proposal.md`) |
| "`SURFACE_RESTING` = `SURFACE_PLAYBACK` = `#333c43`" | `theme/mod.rs:17-18` + `theme/primitives.rs:24` (`PLAYBACK_PANEL_BG = Rgb(51,60,67)`) | **both `#333c43`** | CONFIRMED |
| "`SURFACE_FOCUSED` = `TEXT_ACCENT_MUTED`" | `theme/mod.rs:16` and `:49` both `primitives::BG_GREEN` (`Rgb(60,72,65)`) | **equal (`#3c4841`)** | CONFIRMED |
| "`SURFACE_BACKDROP` … aliased `SURFACE_ARTWORK_PLACEHOLDER`" | `theme/mod.rs:14` (`LIBRARY_SIDE_BG`) and `:23` (`ARTWORK_PLACEHOLDER`), both `Rgb(45,53,59)` | **equal (`#2d353b`)** | CONFIRMED |
| "one role, six meanings" (`SURFACE_BACKDROP`) | inventory §4 above, per-site | **nine meanings** (eight direct paint sites + the `list_selected_row_bg` alias); the audit's "visualizer background" is a test-only call (`visualizer.rs:182`) | CORRECTED in `proposal.md` to nine, with the visualizer meaning removed |
| "0 lever call sites" for `SURFACE_ACCENT_SOFT` | `rg -n "SURFACE_ACCENT_SOFT" … ` shows no `resolve_surface_focus`/`list_selected_row_bg` call site resolves to it | **confirmed — no resolver emits it** | CONFIRMED |

No other number in `proposal.md` (the `#3c4841`, `#48584e`, `#333c43`, `#2d353b` values, the
"two roles / one value", "one role / six meanings" and "surface role aliasing a text role" claims)
contradicts the tree. The `§ Impact` list stays as written.
