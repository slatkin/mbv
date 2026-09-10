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

- Repainting anything: every surface keeps the colour it has today unless a task says otherwise. The
  one intended exception is the wide library's own sub-panels, which stop following the cursor: with
  the column focused, the rail and the pane's content box both render the focused appearance (the
  "selection moves inside a focused pane" scenario). That follows from removing the screens' colour
  choice — the cursor was never a colour input once the column supplies it.
- Replacing the role constants: they stay the values. This change is about the mapping.
- Layout, breakpoints, painter ownership, mouse geometry, or per-screen content.

## Decisions

### D1 — One focus state, computed once

`FocusState { column: Column { Left, Right }, right_visible: bool }` is computed once per frame in the
shell and handed to screens as a fact: which column holds panel focus, and whether the right (library)
column is on screen at all. `FrameChromeGeometry` stops carrying ad-hoc bits and carries this value; a
screen receives it with its render context.

The state deliberately carries **no** sub-surface or cursor position. A surface's colour never depends
on where the cursor sits inside its column — the delta spec's "the selection moves inside a focused
pane" scenario states that, and the only screen-level use for a sub-surface would be to re-introduce
the per-screen colour bits this change removes. Passing it in would be dead data that invites exactly
that regression.

What replaces it, field by field, is in the task 2.1 inventory: the pairs of screen-owned bits
(`episode_focused` / `track_active` / `chapter_focused`, and the pane's `LeftPaneFocus`) currently
stand in for "the library column is focused", and the shell's `queue_focused` / `right_focused` stand
in for the column pair. The per-screen bits survive for behaviour — selection gating, cursor
ownership, hit geometry — and are removed from the colour path only.

Today each screen re-derives its own colour bit from its own sub-mode; that derivation is what made
"change the focused panel look" screen-by-screen work.

_Alternative:_ keep per-screen bits and centralise only the colours. Rejected — the bit is the
decision; centralising the colour alone is what the abandoned change did.

### D2 — One surface table: `(Surface, &FocusState) -> SurfaceColors`

The theme gains a closed `Surface` identity per rendered region — column surfaces, panels, inset
boxes, the popup frame, the recess rows — and one function mapping it plus the focus state to a
`SurfaceColors { fill, border }`. One row per surface, each row carrying its nesting level. Roles
stay as the values, so a level's appearance is still changed by editing one role, and the table
proves which surfaces that reaches.

The levels are, shallowest first: **column/pane** — the surface a content body sits on, including
the column gutters the shell paints and the hero pane fill; **content body** — a focusable content
region that holds content: a panel's body (Library/Queue), a screen's main content box, a sidebar's
body, a popup's list. The level name deliberately avoids the reserved word `Panel`, which is
legitimate only in surface *names* like `LibraryPanel`/`QueuePanel` — those really are panels;
**recess** — a non-focusable inset inside a content body (the now-playing panel's own rows and
status pills, the visualizer background); **chrome band** — non-focusable structural chrome that
never follows panel focus (the tab bar, the status bar, a column's header/status rows, the pill row and the spacer band
below it, a sidebar's header/footer); **popup** — an overlay frame and its dim backdrop, which never
follow a panel. A selected row is not a level of its own: it is a hole in its content body through which the
containing column surface shows, so it takes the column/pane level wherever the row sits. The
now-playing panel's own rows and status pills are a recess, not a chrome band: they sit inside a
content body.

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

**The value map (decided, user-chosen).** Every surface keeps the colour it has today, including the
fact that two levels share a value:

| level | focused | resting |
| --- | --- | --- |
| column/pane | `#3c4841` (`SURFACE_FOCUSED`) | `#2d353b` (`SURFACE_BACKDROP`) |
| content body | `#3c4841` (`SURFACE_FOCUSED`) | `#333c43` (`SURFACE_RESTING`) |
| content body, soft variant | `#48584e` (`SURFACE_ACCENT_SOFT`, retired as a name) | `#333c43` |
| recess, chrome band, popup | as each surface paints today (`SURFACE_CHROME`, `SURFACE_BACKDROP`, `PILL_ROW_BG`, the popup frame role) | |

The **soft content body** is the table's one declared variant: the lighter green that today's
`SURFACE_ACCENT_SOFT` carries, for the content surfaces that read as content inside a container — the
queue panel, the TV episode box and the Music track box today, and the pane content boxes that start
following focus in this change (TV's overview box, the inline-hero content box). It is a property of
the surface **row**, so a screen still only names its surface and never a variant.

_Rejected alternatives, recorded:_ making the two greens two *levels* (column `#3c4841`, every content
body `#48584e`) would repaint every focused panel body including the now-playing band, which the user
had already rejected by asking for that band's original colour; normalising to one green
(`#3c4841` everywhere) would take the lighter green away from the three surfaces that carry it today.
Both are visible repaints for no gain the user asked for, so the default-plus-variant map wins.

**Row shape: level-driven focused appearance, declared resting deviations.** Row 4.1 raised this and
the answer is part of the design, because a level-driven table cannot both stay level-driven and leave
today's resting values alone: the inventory proves that surfaces sharing a level do not rest at one
value. So a row declares:

- a **level** (metadata, and the thing an edit is expressed against);
- a **focused appearance** — the level's focused value, or the declared soft content-body variant.
  This is level-driven: one edit to a level's focused appearance reaches every row of that level;
- a **resting appearance** — today's value for that surface, because this change touches no resting
  surface. Where a row's resting value differs from its level's resting default (`#333c43`,
  `SURFACE_RESTING` — the value `CONTEXT.md` pins as the Hero pane's resting fill) the row declares a
  **named deviation with its reason**; the deviations are listed where the table is declared and
  counted in section 6's report, so they are visible and removable one at a time.

_Why not the alternative:_ making the resting side level-driven too would repaint `HeroPane`,
`QueueColumn` and `QueuePanel` (and contradict `CONTEXT.md`'s definition of the Hero pane), which is
the repaint the user ruled out; leaving every appearance as a bare per-row value with no level rule
would make the level decorative and re-create today's problem. Level-driven where the change is about
appearance, declared-and-counted where reality differs.

**Selection is its own row, not a third input.** A selected list row and a selected pill are distinct
surfaces (`SelectedRow`, `PillChipSelected`) rather than a `selected` flag threaded through
`surface_colors`, so the signature stays `(Surface, &FocusState)` and the conformance test can
enumerate variants. `SelectedRow` is the hole in its panel through which the containing column
surface shows; a popup's context-menu row is the one row at that level with a different value and is
declared as a named variant rather than silently.

**A popup whose body paints no fill.** The context menu clears its rect to the terminal default and
draws foreground-only rows, so it has no body fill to name: the table's popup frame covers the popups
that do paint one, and the context menu's only table row is its selected row. Recorded because
`Color::Reset` is the terminal default rather than a chosen colour — the site names no role — and
because filling it with the popup frame would repaint a surface the user did not ask to change. A
later change that gives the context menu a frame names `PopupFrame` like every other popup.

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
- **[One load-sensitive pre-existing flake]** → a parallel full-suite run aborted once with SIGABRT in
  `app::tests_home_latest::home_play_and_enqueue_leave_feeds_tab_state_untouched`; twenty isolated runs
  of that test pass, its paths are untouched by this change, and at the time the table had no
  production callers at all. Recorded rather than fixed here, and section 6 re-runs the suite.
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
classified against D2's five levels. This is the migration checklist for section 4 and the
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
sites are grouped by surface identity in reading order (the column/pane rows — including the
selected-row punch-through — then content body, then recess, then chrome band, then popup). The final
column is the proposed closed-enum name, a **sketch**: the canonical set is declared in the theme by
row 4.1 and pinned by the 5.2 conformance test. The table after the inventory is that name set,
deduplicated.

| file:line | surface identity | level | owner painter (fn) | rect source | focus input | role(s)/resolver named today | proposed `Surface` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `render/components/chrome.rs:53` | queue column gutter | column/pane | `render_legacy_backdrops` | `FrameChromeGeometry.left_area` minus the 1-col boundary col (`arrangements/chrome.rs`) | `queue_focused` (`PanelFocus::Queue`) | `resolve_surface_focus(queue_focused)` | `QueueColumn` |
| `components/queue_boundary.rs:121` | queue column gutter (1-col drag strip) | column/pane | `QueueBoundaryComponent::view` | `layout.main.queue_boundary_area` synced by `Model::sync_queue_boundary` (`shell_library.rs:194`) | `PanelFocus::Queue` | `resolve_surface_focus(self.focused)` | `QueueColumn` |
| `render/components/chrome.rs:61` (decision `chrome.rs:29/31`) | library column gutter | column/pane | `render_legacy_backdrops` → `library_column_surface` | `FrameChromeGeometry.right_full_area` | `right_focused` (`right_visible && PanelFocus::Library`) | `SURFACE_FOCUSED` / `SURFACE_BACKDROP` in the transitional helper row 4.2 owns | `LibraryColumn` |
| `render/components/music_wide.rs:549` | wide Music browser pane container (the column surface the rail panel sits on) | column/pane | `render_wide_music_group_with_ctx` | `panes.browser_panel` from `MusicWideRenderCtx::publish_geometry` (`wide_library_panes`) | none | `SURFACE_BACKDROP` | `LibraryColumn` |
| `components/wide_hero_boundary.rs:121` | wide hero split gap gutter | column/pane | `WideHeroBoundaryComponent::view` | `Model::wide_hero_boundary_geometry()` (`shell_library.rs`) → the component's synced area | none | `SURFACE_BACKDROP` | `WideSplitGutter` |
| `render/arrangements/wide_hero.rs:302-306` | hero pane fill | column/pane | `wide_hero_hero_pane` | `wide_hero_presentation().hero` (the arrangement's own split) | `LeftPaneFocus::ReadOnly` / `Workspace(held)` | `SURFACE_RESTING` / `resolve_surface_focus(held)` | `HeroPane` |
| `render/components/widgets.rs:151` | selected block background (generic punch-through) | column/pane | `render_selected_block_background` | caller's rect | caller's `bg` argument | caller-supplied | `SelectedRow` |
| `render/components/list_rows.rs:341,354,383` | selected media row/cell (legacy list rows) | column/pane | `build_list_row_spans` / `item_cell_spans` | list cell rect | none | `list_selected_row_bg()` → `SURFACE_BACKDROP` | `SelectedRow` |
| `render/components/list_rows.rs:469` | selected marker gutter | column/pane | `draw_column_selection_markers` | `content_area` + selected row | none | `list_selected_row_bg()` | `SelectedRow` |
| `render/components/media_list/wide.rs:202-203` → `wide_row.rs:199` | selected media row (WideMediaList) | column/pane | `selected_row_surface_color` → `wide_media_row` | retained row paint area | `focused` + `SelectedRowSurface` policy | `list_selected_row_bg()` / `resolve_surface_focus(focused)` | `SelectedRow` |
| `render/components/media_list/wide.rs:268` | selected grid cell (GridMediaList) | column/pane | `render_grid_media_list_component` | cell rect | `policy.focused()` | `list_selected_row_bg()` | `SelectedRow` |
| `render/components/context_menu.rs:38` | context menu selected row | column/pane | context menu painter | row rect | `selected` | `ACCENT_ACTIVE` | `SelectedRow` |
| `render/arrangements/wide_hero.rs:415-419` | library panel body (wide-hero rail) | content body | `wide_hero_browser_border` | `wide_hero_browser_pane(..).list_panel` | `focused` bit | `resolve_surface_focus` | `LibraryPanel` |
| `render/arrangements/wide_hero.rs:422` (bg at `widgets.rs:187`) | library panel frame rows (▔/▁) | content body (border rows of a content body) | `wide_hero_browser_border` → `render_selected_block_borders` | `list_panel` | `focused` | `resolve_surface_focus` (as border fg/bg) | `LibraryPanel` |
| `components/browser/paint.rs:108-110` | library panel body (wide Movies/home-video rail) | content body | `BrowserComponent::render_wide_movies` | `list_panel` | `self.focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/tv_wide.rs:312` | library panel body (wide TV series rail) | content body | `render_wide_tv_with_ctx` | `list_panel` | `right_focused` (pane bit) | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/music_wide.rs:581` | library panel body (wide Music album rail) | content body | `render_wide_music_group_with_ctx` | `list_panel` | `right_focused` (pane bit) | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/audiobookshelf_book.rs:197` | library panel body (ABS book rail) | content body | `render_audiobookshelf_book_content` | `list_panel` | `rail_focused` (sub-focus bit) | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/audiobookshelf_podcast.rs:264` | library panel body (ABS podcast show rail) | content body | `render_audiobookshelf_podcast_content` | `list_panel` | `focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/feeds.rs:211` | library panel body (wide Feeds rail) | content body | `render_feeds_content` | `list_panel` | `focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/home.rs:370-372` | library panel body (Home wide list panel) | content body | `render_home_content` | `green_panel_full` (the screen's own two-column split) | `focused` | `resolve_surface_focus` | `LibraryPanel` |
| `render/components/widgets.rs:237-241` | queue panel body | content body | `render_queue_panel_frame` | `queue_geometry.panel_area` ← `queue_panel_geometry(left_content)` (`shell_draw.rs`) | `queue_focused` | `SURFACE_ACCENT_SOFT` / `SURFACE_BACKDROP` | `QueuePanel` |
| `render/components/tv_wide.rs:515` | pane content box (TV episode listing) | content body | `render_tv_series_selection` | `wide_hero_hero_content_box` slot placement | `episode_focused` (sub-focus bit) | `SURFACE_ACCENT_SOFT` | `MainContentBox` |
| `render/arrangements/wide_hero.rs:468` (variant chosen in `music_wide.rs`) | pane content box (Music track listing) | content body | `wide_hero_hero_content_box_with_surface` | caller's `track_area` | `left_focused` (`track_active`) | `SURFACE_ACCENT_SOFT` | `MainContentBox` |
| `render/arrangements/wide_hero.rs:467` | pane content box (generic pane inset) | content body | `wide_hero_hero_content_box` | caller's area | none | `SURFACE_BACKDROP` | `MainContentBox` |
| `render/components/hero.rs:192-196` | selected row's inline detail (inline hero) | content body | `selected_detail_shell` | caller `hero_area`; callers `audiobookshelf_book.rs:296`, `audiobookshelf_podcast.rs:336`, `feeds.rs:268`, `home.rs:477`, `list_narrow.rs:219`, `music_wide.rs:427` | `focused` | `resolve_surface_focus` | `InlineHero` |
| `shell_playback.rs:48-50` | now-playing (playback) panel body | content body | `render_playback_component` → `render_player_panel` | `FrameChromeGeometry.player_area` | `PanelFocus::Queue` | `SURFACE_FOCUSED` / `SURFACE_PLAYBACK` | `PlaybackPanel` |
| `components/playback.rs:55` | PlaybackComponent projection panel bg | content body | `PlaybackComponent` (projection value) | n/a — value, not a rect | `PanelFocus::Queue` | `SURFACE_PLAYBACK` | `PlaybackPanel` |
| `shell_draw.rs:286` | queue-only wide playback panel body | content body | `render_main` | local `panel_area` from `left_content` + card | none (`QueueOnly`) | `SURFACE_CHROME` | `PlaybackPanel` |
| `shell_draw.rs:297,316` | queue-only playback panel context bg | content body | `render_main` → `render_player_panel` | local `panel_area` | none | `SURFACE_CHROME` | `PlaybackPanel` |
| `render/components/chrome.rs:188-191` | sidebar body (help/settings/search/playlists/sessions) | content body | `render_panel_shell_at` | `panel_shell_rect(sidebar)` | `style` variant (not focus) | `SURFACE_RESTING` / `SURFACE_SIDEBAR` | `SidebarBody` |
| `render/components/chrome_player.rs:52,65,93,109,201,410` | now-playing panel content rows (seekbar/title/blank) | recess | `render_player_panel` / `render_seekbar` / `render_title_row` | `ctx.area` rows | via `ctx.panel_bg` | `ctx.panel_bg` | `PlaybackRecess` |
| `render/components/chrome_player.rs:126,162` | now-playing bottom row ("On Now" row) | recess — Mini (`narrow_player`) only; in Wide/Normal the row is inside `LibraryColumn`'s backdrop rect and carries no separate surface (finding 3) | `render_player_panel` | `ctx.area` row +3 | `narrow_player` (mode split, not a focus bit) | `SURFACE_BACKDROP` | `PlaybackBottomRow` |
| `render/components/chrome_player.rs:243` (applied `:277,283,301`) | now-playing status pill | recess | `render_title_row` | title row's right segment | none | `SURFACE_BACKDROP` | `PlaybackStatusPill` |
| `render/components/chrome_status.rs:305` | status bar body | chrome band | `render_status_bar` | `FrameChromeGeometry.status_area` | none | `SURFACE_CHROME` | `StatusBar` |
| `render/components/chrome_status.rs:28,63,66,76,128,136,139,149,161,166,169,173,178,180,192,197,200,218,223,229,469,481,487` | status bar pills/chips | chrome band | `render_status_bar` pill builders | within `status_area` | none | `SURFACE_STATUS_PILL` | `StatusBarPill` |
| `render/components/queue.rs:166` | queue panel title row | chrome band | `render_queue_title_content` | QueueComponent's title row | none | `SURFACE_CHROME` | `QueuePanelBand` |
| `render/components/queue.rs:175,197` | queue scope pill (local) | chrome band | `render_queue_title_content` | `local_area` | none | `SURFACE_CHROME` | `QueuePanelBand` |
| `render/components/queue.rs:219,231` | queue scope target row (remote) | chrome band | `render_queue_title_content` | `target_area` | none | `SURFACE_CHROME` | `QueuePanelBand` |
| `render/components/queue.rs:297` | queue panel status strip | chrome band | `render_queue_status` | `queue_geometry.pill_row` | none | `SURFACE_CHROME` | `QueuePanelBand` |
| `render/components/card.rs:245-249` (fill `visualizer.rs:16,20,44`) | queue card visualizer background | recess | `App::render_visualizer` (decision in `render_card_visualizer`) | `card_reserved_rect` | `PanelFocus::Queue` | `resolve_surface_focus` | `QueueCardVisualizer` |
| `render/components/artwork_placeholder.rs:9` | artwork placeholder | recess | `render_artwork_placeholder` | caller's artwork slot | none | `SURFACE_ARTWORK_PLACEHOLDER` | `ArtworkPlaceholder` |
| `render/components/card.rs:111` | artwork loading placeholder (queue card) | recess | `render_card_image` | `card_reserved_rect` | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/album_art.rs:183` | artwork loading placeholder (inline album art) | recess | `render_inline_art_cell` | `img_rect` | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/detail_series_view.rs:125` | artwork loading placeholder (series hero) | recess | series detail painter | `result.img_rect` | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/home_hero_emby.rs:121,272,286` | artwork loading placeholder (Emby hero) | recess | Emby hero painter | image area | none | `BORDER_UNFOCUSED` (as a fill) | `ArtworkLoadingPlaceholder` |
| `render/components/widgets.rs:391,403,489` | pill selector row background | chrome band | `render_pill_bar` | `pills_area` | none | `PILL_ROW_BG` | `PillRow` |
| `render/components/hero.rs:666,669` | pill selector row background (hero shell) | chrome band | hero pill row painter | pill row | none | `PILL_ROW_BG` | `PillRow` |
| `render/components/home.rs:402-405` | Home pill-bar spacer band (the row below the pill bar) | chrome band | `render_home_content` | `spacer_area` from `wide_hero::pill_bar_areas(area)` (wide) / the narrow pill areas | none | `SURFACE_BACKDROP` (the comment claims a wide-vs-single-column split the code does not implement — finding 8) | `PillRowGap` |
| `render/components/widgets.rs:254,256` (applied `:446-468`) | pill chip (selected/unselected) | chrome band | `selector_pill_style` / `render_pill_bar` | pill rect | `selected` | `PILL_SELECTED_BG` / `PILL_BG` | `PillChip` |
| `render/components/search_sidebar.rs:161` | search sidebar selected chip | chrome band | search sidebar painter | chip rect | `selected` | `PILL_SELECTED_BG` | `PillChip` |
| `render/components/chrome.rs:219,221,246,248,255,280,282,299,311` | sidebar header/footer rows | chrome band | `render_panel_shell_at` | header/footer rects | `style` variant | `SURFACE_CHROME` / `SURFACE_ITEM_FOCUSED` / `SURFACE_RESTING` | `SidebarBand` |
| `render/components/chrome_tabs.rs:43` | tab bar background | chrome band | `render_tabs` | `FrameChromeGeometry.tab_bar_area` | none | `SURFACE_CHROME` | `TabBar` |
| `render/components/chrome_tabs.rs:131` | tab bar inactive tab glyph | chrome band | `render_tabs` | tab row cell | none | `Color::Rgb(73, 81, 86)` raw (see Findings) | `TabBar` |
| `render/components/modal_frame.rs:46` | popup frame body | popup | `render_modal_frame_inner` | centred rect from `f.area()` | none | caller's `bg` | `PopupFrame` |
| `render/components/confirm_modal.rs:29` | popup frame body | popup | `render_confirm_modal_content` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/daemon_lost_modal.rs:29` | popup frame body | popup | `render_daemon_lost_modal_content` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/feeds_manage.rs:60` | popup frame body (feed list) | popup | `render_feeds_manage_list` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/feeds_manage.rs:171` | popup frame body (feed form) | popup | `render_feeds_manage_form` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/library_routes.rs:156` | popup frame body | popup | `render_library_routes_content` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/multiselect.rs:53` | popup frame body | popup | `render_multiselect_content` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/playlists.rs:41` | popup frame body (save/rename) | popup | `render_save_playlist_content` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/remote_reanchor.rs:31` | popup frame body | popup | `render_remote_reanchor_popup_content` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/selection_modal.rs:65` | popup frame body | popup | `render_selection_modal_content` | modal rect | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/selection_modal.rs:96` | popup list spacer | popup | `render_selection_modal_content` | inner rect + filter height | none | `SURFACE_FOCUSED` | `PopupFrame` |
| `render/components/backdrop.rs:dim_backdrop` | popup dim backdrop | popup | `dim_backdrop` | `f.area()` | none | shades every existing cell (no role) | `PopupDimBackdrop` |

### Proposed `Surface` name set

_Superseded._ The canonical identity set is the `Surface` enum in `src/app/render/theme/surface.rs` (row
4.1), and the mapping from each inventory identity to a declared variant is the next section. This
sketch is kept only as the history of how the names were proposed — do not read a name here as
binding, and do not add a variant from it without the mapping entry.

**Proposed sketch — the canonical set is declared in the theme by row 4.1 and pinned by the 5.2
conformance test.** The closed enum D2 needs, one name per distinct *structural* identity in the
table above: the same layout position in a different screen or provider is the same name. Level is
the enum's only style input: every name at one level resolves to that level's `SurfaceColors`.

| proposed `Surface` | level | distinct structural identities covered |
| --- | --- | --- |
| `QueueColumn` | column/pane | queue column gutter, queue boundary strip |
| `LibraryColumn` | column/pane | library column gutter, wide Music browser pane container |
| `WideSplitGutter` | column/pane | wide hero split gap |
| `HeroPane` | column/pane | wide hero pane fill |
| `SelectedRow` | column/pane | selected row/cell/marker punch-through, generic selected block, context menu selected row |
| `LibraryPanel` | content body | wide-hero rail body + frame, Home two-column list panel |
| `QueuePanel` | content body | queue panel body |
| `MainContentBox` | content body | a pane's content box (TV episode listing, Music track listing, generic pane inset) |
| `InlineHero` | content body | a selected row's inline detail (six screens) |
| `PlaybackPanel` | content body | now-playing panel body (right column and queue-only) |
| `SidebarBody` | content body | sidebar body (help/settings/search/playlists/sessions) |
| `PlaybackRecess` | recess | now-playing panel content rows |
| `PlaybackBottomRow` | recess | Mini-only "On Now" row |
| `PlaybackStatusPill` | recess | now-playing panel's status pill |
| `QueueCardVisualizer` | recess | queue card visualizer background |
| `ArtworkPlaceholder` | recess | artwork placeholder |
| `ArtworkLoadingPlaceholder` | recess | artwork loading placeholder (four painters) |
| `StatusBar` | chrome band | status bar body |
| `StatusBarPill` | chrome band | status bar pills/chips |
| `QueuePanelBand` | chrome band | queue title row, scope pills, scope target, status strip |
| `PillRow` | chrome band | pill selector row background |
| `PillChip` | chrome band | selected/unselected pill chip, search sidebar selected chip |
| `PillRowGap` | chrome band | Home pill-bar spacer band |
| `SidebarBand` | chrome band | sidebar header/footer rows |
| `TabBar` | chrome band | tab bar background and its inactive-tab glyph |
| `PopupFrame` | popup | popup frame body and its list spacer |
| `PopupDimBackdrop` | popup | popup dim backdrop |

A screen or provider name in this enum is a defect: `TvEpisodeBox`, `MusicTrackBox` and
`HeroContentBox` were three names for one position (a pane's content box), and `SelectedDetailBlock`
named a screen's detail rather than the position it occupies. Likewise `SidebarBody` carries the
sidebar's focusable body but not the name `Panel`, which `CONTEXT.md` reserves for Library/Queue.

### Identity → `Surface` mapping (task 4.1)

The canonical set is now declared in `src/app/render/theme/surface.rs` (the `Surface` enum and
`ALL`), `surface_table.rs` (the rows, `Surface::level`, `RESTING_DEVIATIONS`) and
`surface_resolve.rs` (the resolver) — 34 variants, `Surface::ALL`; this table places every identity
in the task 2.1 inventory against it, so "no identity unplaced" is checkable by a reader. Sites are
the inventory's `file:line`; where the inventory groups a shared painter, every listed caller is
covered.

| inventory identity (sites) | declared `Surface` |
| --- | --- |
| queue column gutter, 1-col drag strip (`chrome.rs:53`, `queue_boundary.rs:121`) | `QueueColumn` |
| library column gutter (`chrome.rs:61`), wide Music browser pane container (`music_wide.rs:549`) | `LibraryColumn` |
| wide hero split gap gutter (`wide_hero_boundary.rs:121`) | `WideSplitGutter` |
| hero pane fill (`wide_hero.rs:302-306`) | `HeroPane` |
| selected block background (`widgets.rs:151`), legacy list row/cell + marker (`list_rows.rs:341,354,383,469`), Wide/Grid list row and cell (`media_list/wide.rs:202,268`) | `SelectedRow` |
| queue list's selected row (`queue.rs:36`) | `SelectedRowOnQueueColumn` |
| TV episode-list and Music track-list selected rows (`tv_wide.rs:576`, `music_wide.rs:538`) | `SelectedRowOnLibraryPane` |
| context menu selected row (`context_menu.rs:38`) | `ContextMenuSelectedRow` |
| wide-hero rail body + frame (`wide_hero.rs:415-422`), Movies/home-video rail (`browser/paint.rs:108-110`), TV series rail (`tv_wide.rs:312`), Music album rail (`music_wide.rs:581`), ABS book rail (`audiobookshelf_book.rs:197`), ABS podcast show rail (`audiobookshelf_podcast.rs:264`), Feeds rail (`feeds.rs:211`), Home wide list panel (`home.rs:370-372`) | `LibraryPanel` |
| queue panel body (`widgets.rs:237-241`) | `QueuePanel` |
| pane content box: TV episode listing (`tv_wide.rs:515`), Music track listing (`wide_hero.rs:468`), generic pane inset (`wide_hero.rs:467`), TV overview box and inline-hero content box (row 4.6) | `MainContentBox` |
| selected row's inline detail (`hero.rs:192-196`) | `InlineHero` |
| now-playing panel body (`shell_playback.rs:48-50`), projection panel bg (`playback.rs:55`), Queue-only wide body + context bg (`shell_draw.rs:286,297,316`) | `PlaybackPanel` |
| sidebar body (`chrome.rs:188-191`) | `SidebarBody` (expanded), `NonHeroSidebarBody` (non-hero shell) |
| now-playing panel content rows (`chrome_player.rs:52,65,93,109,201,410`) | `PlaybackRecess` |
| now-playing bottom "On Now" row (`chrome_player.rs:126,162`) | `PlaybackBottomRow` |
| now-playing status pill (`chrome_player.rs:243`) | `PlaybackStatusPill` |
| status bar body (`chrome_status.rs:305`) | `StatusBar` |
| status bar pills/chips (`chrome_status.rs:28-487`) | `StatusBarPill` |
| queue title row, scope pills, scope target, status strip (`queue.rs:166,175,197,219,231,297`) | `QueuePanelBand` |
| queue's selected scope pill (`queue.rs:258-287`) | `QueueScopePillSelected` |
| queue card visualizer background (`card.rs:245-249`) | `QueueCardVisualizer` |
| artwork placeholder (`artwork_placeholder.rs:9`) | `ArtworkPlaceholder` |
| artwork loading placeholder (`card.rs:111`, `album_art.rs:183`, `detail_series_view.rs:125`, `home_hero_emby.rs:121,272,286`) | `ArtworkLoadingPlaceholder` |
| pill selector row background (`widgets.rs:391,403,489`, `hero.rs:666,669`) | `PillRow` |
| pill chip (`widgets.rs:254,256`) | `PillChip` (unselected), `PillChipSelected` (selected) |
| search sidebar selected chip (`search_sidebar.rs:161`) | `PillChipSelected` |
| Home pill-bar spacer band (`home.rs:402-405`) | `PillRowGap` |
| sidebar header/footer rows (`chrome.rs:219-311`) | `SidebarBand` (expanded), `NonHeroSidebarBand` (non-hero shell) |
| tab bar background (`chrome_tabs.rs:43`) | `TabBar` |
| tab bar inactive glyph (`chrome_tabs.rs:131`) | `TabBar` (foreground glyph; row 4.4 makes it a role) |
| popup frame body + list spacer (`modal_frame.rs:46` and its ten callers, `selection_modal.rs:96`) | `PopupFrame` |
| popup dim backdrop (`backdrop.rs:dim_backdrop`) | `PopupDimBackdrop` |

Splits against the sketch (each a second appearance the sketch collapsed; the theme resolves one
colour per row, so both values need a row):

- `SelectedRow` → `SelectedRow`, `SelectedRowOnQueueColumn`, `SelectedRowOnLibraryPane`: the name
  states which containing surface's value shows through the hole (that is what determines the value
  and it stays true if the row is reparented) — the library rails punch through to the app backdrop
  (`list_selected_row_bg()`), while the queue list shows the queue column's fill and the TV/Music
  panes show the library pane's fill (`resolve_surface_focus`).
- `SelectedRow` → `ContextMenuSelectedRow`: `ACCENT_ACTIVE` is not the column/pane level's value.
- `SidebarBody`/`SidebarBand` → `NonHeroSidebarBody`/`NonHeroSidebarBand`: the non-hero shell
  paints `SURFACE_SIDEBAR`/`SURFACE_ITEM_FOCUSED`, not the expanded panel's
  `SURFACE_RESTING`/`SURFACE_CHROME`.
- `PillChip` → `PillChip` + `PillChipSelected`: selection is its own row, not a third input to
  `surface_colors`.

Two placements the inventory deliberately left open are resolved as follows. `QueueCardVisualizer`
is declared at `content body` (row 4.1 review: it resolves the content-body pair, so a recess
restyle must not repaint it and a content-body edit must reach it) and resolves that pair with the
queue column's focus, because the queue card is the queue's now-playing content and not an inset
inside a panel: `card.rs:245` paints `resolve_surface_focus(queue_column_focused())`, i.e.
`SURFACE_FOCUSED` when the queue column holds focus and `SURFACE_RESTING` otherwise — exactly the
content-body default, so its picture is unchanged and it needs no deviation. `ContextMenuSelectedRow`
keeps today's picture as a declared named variant: the column/pane level's normal values are
`SURFACE_FOCUSED` `#3c4841` / `SURFACE_RESTING` `#333c43`, and `context_menu.rs:38` paints
`ACCENT_ACTIVE` `#a7c080`, so the second appearance is declared rather than retired.

The table's one mode-driven row is the playback strip: with no right column on screen, Queue-only
paints `PlaybackPanel` and `PlaybackRecess` as the chrome band (`shell_draw.rs:286,297,316`), read
from `FocusState::right_visible()`. Resting-side deviations (rows whose resting value is not their
level's default) are `LibraryColumn`, `WideSplitGutter`, `SelectedRow`, `ContextMenuSelectedRow`,
`QueuePanel`, `MainContentBox` and `NonHeroSidebarBody` — declared with reasons in
`RESTING_DEVIATIONS`; `HeroPane` is the level default, per `CONTEXT.md`.

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
sidebar and queue-chrome rows, and the `SURFACE_ACCENT_SOFT` content-body rows.

**3. Duplicate roles carrying one level.**

- `SURFACE_ACCENT_SOFT` (`#48584e`) is the content body level's focus fill at three production sites:
  `widgets.rs:237` (queue panel), `tv_wide.rs:515` (pane content box), `wide_hero.rs:468` (pane
  content box). `SURFACE_FOCUSED` (`#3c4841`) is the same level's focus fill at every other
  content-body site, so one level has two values and a role edit reaches one group or the other,
  never both. `SURFACE_ACCENT_SOFT` has zero `resolve_surface_focus` call sites.
- `SURFACE_PLAYBACK` (`#333c43`) is the now-playing panel/recess value while `SURFACE_RESTING`
  (`#333c43`) is the resting content-body and column value — one value spanning three levels, so
  a retarget of either moves surfaces at levels the other does not own.

**4. The meanings of `SURFACE_BACKDROP` (`#2d353b`).** Nine distinct rendered meanings at eight
direct production paint sites plus the `list_selected_row_bg` alias: (a) library column gutter
(`chrome.rs:31`); (b) wide hero split gap (`wide_hero_boundary.rs:121`); (c) wide Music browser
containing panel (`music_wide.rs:549`); (d) pane content box (`wide_hero.rs:467`); (e)
unfocused queue panel (`widgets.rs:239`); (f) narrow "On Now" row and its title (`chrome_player.rs:126,162`);
(g) now-playing status pill (`chrome_player.rs:243`); (h) pill-bar spacer / library column first
content row (`home.rs:402`); (i) selected-row punch-through via `list_selected_row_bg()`
(`theme/mod.rs:96`). The audit's visualizer-background meaning is **not** a production
`SURFACE_BACKDROP` site — the only such call is `visualizer.rs:182`, a test; production resolves
focus at `card.rs:245`.

**5. Per-screen colour bits.** The sites whose colour input is a screen-local bit rather than a
shell fact:

- `episode_focused` — `tv_wide.rs:515` (pane content box), and `tv_wide.rs:262`'s
  `LeftPaneFocus::Workspace(ctx.focused && ctx.episode_cursor.is_some())` (hero pane).
- `track_active` — `music_wide.rs`'s `left_focused = ctx.focused && track_active`, feeding both
  `wide_hero_hero_pane` and the pane-content-box variant (`wide_hero.rs:468`).
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
2. **Multi-owner identity — a pane's content box.** `wide_hero_hero_content_box` paints the
   `MainContentBox` `SURFACE_BACKDROP` and `render_tv_series_selection` repaints the same rect
   `SURFACE_ACCENT_SOFT` when focused. The surface's level depends on which painter ran last, not on
   a declared level.
3. **The row above the pill bar is a mode split, not a second owner.** In Wide and Normal the row is
   inside `LibraryColumn`'s backdrop rect (`chrome.rs:61`): there is no separate surface — row 1.3
   made the ownership geographic. Only in Mini does `render_player_panel` paint it as the playback
   panel's own bottom band (`chrome_player.rs:126`), so `PlaybackBottomRow` is a Mini-only identity
   and the wide case carries no separate surface. The residual risk is that the discriminator is the
   `narrow_player` mode flag rather than the rect, so a new layout that paints the panel must keep
   applying it.
4. **Screens choosing colour.** The ten modal callers pass `SURFACE_FOCUSED` into
   `render_modal_frame`; `tv_wide.rs:515` picks `SURFACE_ACCENT_SOFT`; `music_wide.rs` picks the
   `WideHeroContentBoxSurface` variant; `home.rs:402` picks `SURFACE_BACKDROP`; `card.rs:245` picks
   the visualizer bg. D3 forbids all of these.
5. **Two roles for one visible level.** `SURFACE_ACCENT_SOFT` vs `SURFACE_FOCUSED` (content body);
   `SURFACE_STATUS_PILL` aliases `SURFACE_CHROME`; `SURFACE_ARTWORK_PLACEHOLDER` aliases
   `SURFACE_BACKDROP` (`#2d353b`); `BORDER_UNFOCUSED` is a border role used as a fill at six call
   sites across four artwork-loading painters (`card.rs:111`, `album_art.rs:183`,
   `detail_series_view.rs:125`, `home_hero_emby.rs:121,272,286`); `SURFACE_RESTING` = `SURFACE_PLAYBACK` (one value, two levels);
   `SURFACE_FOCUSED` = `TEXT_ACCENT_MUTED` (`BG_GREEN`), a surface role aliasing a text role. The
   context menu's selected row (`context_menu.rs:38`) is now under `SelectedRow` at column/pane yet
   paints `ACCENT_ACTIVE` — a second appearance for that level; row 4.3 retires it or 4.1 gives it a
   named variant.
6. **Raw colour in a production painter.** `chrome_tabs.rs:131` writes
   `Style::default().fg(Color::Rgb(73, 81, 86))` for the tab bar's inactive glyph column. Every
   other production colour goes through a role; this one is a hue chosen in a screen-adjacent
   painter, exactly the bypass `theme/primitives.rs` privacy is meant to prevent.
7. **The chrome band level has no single appearance yet.** D2 says two surfaces at one level share
   their appearance, but today's chrome bands resolve to six different roles — `SURFACE_CHROME`
   (tab bar, status bar, queue title/status rows, sidebar header), `SURFACE_ITEM_FOCUSED` and
   `SURFACE_RESTING` (sidebar header/footer), `SURFACE_STATUS_PILL` (status pills), `PILL_ROW_BG`
   and `PILL_SELECTED_BG`/`PILL_BG` (pill row/chip), plus the raw `Color::Rgb` at
   `chrome_tabs.rs:131`. The level's `SurfaceColors` must either collapse these into one appearance
   or the level model needs named variants.
8. **Stale comment on the Home pill-bar spacer.** `home.rs:398-401` says "The wide layout uses the
   list panel surface; the single-column layout inherits the ordinary library panel surface", but
   `panel_bg` is `SURFACE_BACKDROP` unconditionally at `home.rs:402`. A migrator following the
   comment would give the wide case a panel colour the code never paints.

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

## Migration report (task 6.2)

**What the change did.** Colour decisions now have one home and one input. The shell derives one
`FocusState` per frame (the active column plus whether the right column is visible; deliberately no
cursor, because selection never changes an appearance). The theme declares one closed `Surface` set —
34 identities, five levels, and a row per surface naming its level and its focused and resting
appearance — and resolves `surface_colors(surface, &FocusState)`. Every production painter names a
surface; none names a role and none calls a resolver. Two ast-grep rules fail the build if that
changes: no surface role name or resolver in a painter, and no background filled straight from a theme
role.

**Aliases retired** (each proven byte-identical by a colour-literal multiset comparison per changed
file): `SURFACE_ACCENT_SOFT` (its value survives as the table's soft content-body variant),
`SURFACE_PLAYBACK`, `SURFACE_STATUS_PILL`, `SURFACE_ARTWORK_PLACEHOLDER`, the `BORDER_UNFOCUSED` fill
at its six artwork-loading sites, and the `SURFACE_FOCUSED` = `TEXT_ACCENT_MUTED` alias, so a text
colour can no longer move a surface appearance. Three primitives shared between a surface role and a
non-surface role (`BG_GREEN_SOFT`/`SCROLLBAR`, `FOAM`/`PILL_SELECTOR_SELECTED_BG`,
`AQUA`/`PLAYBACK_THROBBER_FG`) were split, same bytes, two names.

**Coverage.** The task 2.1 inventory classified 66 production paint sites; every one now names a
surface, and an independent grep finds no production role name or resolver call anywhere outside
`src/app/render/theme/`. The conformance test (row 5.2) enumerates all 34 surfaces across the
breakpoints: 15 pinned by it directly, 17 by named existing tests, and two — `WideSplitGutter` and
`QueueCardVisualizer` — pinned nowhere at buffer level, because both are painted by boundary
components from their own rects; they are covered only by those components' unit tests.

**What a level edit now reaches.** Editing one focused appearance in the table changes every surface
of that level, and a screen cannot skip it: naming a role is a build failure. Seven surfaces declare a
**resting deviation** with its reason (the library column's backdrop, the queue panel's and the
content box's recess, the non-hero sidebar pair, and `QueueCardVisualizer`), listed where the table is
declared; each is removable one at a time.

**What moved on screen.** Exactly two things, both authorised: the row above the pill bar is now the
library column's rather than the playback panel's (section 1, the behaviour kept from
`focus-aware-column-surface`), and the pane content boxes follow their pane, so TV's overview box and
the inline-hero box take the soft fill while focused instead of staying dark (row 4.6). Everything
else is byte-identical, including the whole existing suite: no test expectation's value moved.

**Residuals, recorded rather than hidden:** the two surfaces pinned nowhere at buffer level (above);
`ACCENT` remains a mixed surface/foreground role; row 4.5's single-painter property rests on the
deletions plus the 5.1 rules rather than a buffer assertion, because two identical fills are
indistinguishable in a buffer; and one load-sensitive SIGABRT flake fires occasionally in a Home test
under parallel full-suite runs and passes in isolation.

**Reviews.** Every unit ran a bounded Standards+Spec review round. Those rounds caught: a vacuous
`debug_assert` and a bool-taking resolver that would have accepted a cursor bit (row 4.1/unit A); a
selected-row colour parameter hardcoded to the focused fill, latent rather than live, now pinned on
both halves (unit C); and — before the rule existed — that an ast-grep rule listing only surface names
would never have caught the queue's aqua scope pill, which is why row 5.1 additionally bans
role-painted backgrounds.
