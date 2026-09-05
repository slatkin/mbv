# Characterization — grouped Music / Audiobookshelf Podcast / Audiobookshelf Book

Tasks 1.1–1.3 evidence for `migrate-music-audiobookshelf-to-canonical-lists`.
Source-verified against the feature branch; corrections to the pre-issue scout
recon are called out inline.

## 1.1 Preconditions

- Feature branch: `feat/migrate-tui-to-tuirealm`.
- **Baseline SHA: `819dbd0c714ffef588cd3ea2679b3aba6c3108f3`** (`819dbd0c`,
  "chore(openspec): archive slice 3.2 migrate-home-feeds-to-canonical-lists").
  Worktree clean at capture time.
- Predecessors already landed on the feature branch:
  - canonical media-list foundation (`introduce-canonical-media-list-foundation`)
    — provides `WideMediaList` / `InlineMediaBrowser` / `ViewportAnchor` in
    `src/app/components/media_list/` and the paint entry points
    `render_wide_media_list` / `render_inline_media_browser` in
    `src/app/render/components/media_list.rs`.
  - #640 Home podcast hero-on-left correction — `shared_hero_presentation`
    (podcast hero artwork-on-top, shared hero placement).
- `migrate-home-feeds-to-canonical-lists` is a sibling slice, already archived
  at `819dbd0c`; it is not a functional dependency.
- This slice is a distinct PR stacked on PR #606's feature branch. No SHA was
  pinned in the plan; this file records the baseline.

## 1.2 Caller inventory

### TV / Movies — canonical source-of-truth precedent to mirror

- `TvWorkspaceComponent` (`src/app/components/tv_workspace/mod.rs`) owns a
  `WideMediaList<String>` (`list`, line 40) and a
  `pending_anchor: Option<ViewportAnchor<String>>` (line 49).
- Breakpoint hand-off is `ViewportAnchor` (`src/app/components/media_list/anchor.rs`):
  `{ selected_target, selected_row_offset }` — no cursor/scroll mirror. The
  parent emits `viewport_anchor(viewport_height)` (mod.rs:189) and the receiver
  restores via `apply_viewport_anchor` (mod.rs:218), which selects the target
  and stashes a one-shot `pending_anchor` applied at the next paint.
- `re_anchor` semantics: an ordinary `set_content` keeps the component's
  divergent local cursor; the shell re-anchors explicitly only at a navigation
  event / active-destination pointer flip. `MusicWorkspaceComponent::re_anchor`
  is documented as mirroring this.
- Movies compose the plain `WideMediaList`; the TV series rail composes
  `InlineMediaBrowser` (selected-row inline replacement). Episodes remain
  parent-owned on the component.

### Grouped Music

- Wide entry: `MusicWorkspaceComponent::view` →
  `render_wide_music_group_with_ctx` (`src/app/render/components/music_wide.rs`)
  → `render_wide_right_album_browser_with_ctx`
  (`src/app/render/components/music_wide_browser.rs:16`) → bespoke row loop
  (music_wide_browser.rs:59-105). **Does NOT use `WideMediaList` /
  `render_wide_media_list`.** Music search mode (music_wide.rs) does use the
  canonical `render_plain_rows`.
- Narrow entry: `MusicWorkspaceComponent::view` (narrow branch,
  `shared_hero_presentation(area).is_none()`) → `render_narrow_music_group_with_ctx`.
  Since #613/task 3.8 the mounted `MusicWorkspaceComponent` is the sole painter
  in BOTH presentations; the legacy `render_library` arm paints no grouped-album
  rows at any breakpoint (see `tests_music_characterization.rs`).
  **Recon correction:** the recon's `render_grouped_album_rows_with_ctx` /
  `GroupedAlbumRenderCtx` narrow path is the older shape; the live narrow path
  is `render_narrow_music_group_with_ctx` driven off the same
  `MusicWideRenderCtx`.
- Grouping: `GroupedAlbumDisplayRow` (`ArtistHeader`, `ArtistGroupSpacer`,
  `Album(idx)`, plus narrow-only detail rows) lives in
  `src/app/render/screens/album_plan.rs`, not `music_wide_browser.rs`. The wide
  rail rebuilds a header/album/spacer sequence at paint time in
  `wide_album_display_rows` (music_wide_browser.rs:136) from `album_info` +
  `album_order`; the narrow path builds the full plan via
  `build_grouped_album_display_plan_with_ctx`. Order comes from the settled
  `GroupedAlbumCatalog` or `sorted_group_album_order` (natural sort key of the
  artist label; stable within an artist). Grouping is derived, NOT shell-mirrored.
- Cursor / scroll / selected-target ownership: `MusicWorkspaceComponent`
  (`src/app/components/music_workspace.rs`): `album_cursor` (25), `album_scroll`
  (28, written back from the painter's returned `final_scroll` each `view`),
  `track_cursor` (29, wide-only inline track focus). `re_anchor(cursor, scroll)`
  (115) adopts the shell's resting value unconditionally. `set_content` (76)
  clamps but never adopts the shell cursor; the one content-driven reset is
  `track_cursor = None` on selected-album identity change.
- Shell wiring: `src/app/shell_music_workspace.rs`. `push_music_workspace_content`
  mirrors the browse snapshot + geometry event-driven; `music_workspace_reanchor`
  is the one-shot trigger consumed at the next push (mount / group switch /
  recursive activation / saved-position restore). The shell owns only
  `App.music_levels` + the resting `BrowseLevel` cursor/scroll.
- **No `ViewportAnchor` hand-off exists for Music today** — the breakpoint
  transition relies on the component simply keeping `album_cursor`/`album_scroll`
  across a presentation flip (no re-push, or an ordinary push that does not
  touch the cursor). §2.5 introduces the stable target/offset hand-off.

### Audiobookshelf Podcast

- Entry: `render_audiobookshelf_podcast_content`
  (`src/app/render/components/audiobookshelf_podcast.rs:119`).
- Wide branch (line 130): `hero_left::shared_hero_presentation(area)` → paints
  `render_podcast_hero` into `hero_panel` + `render_show_rows` straight into
  `right_panel` (line 172). It does **not** route the right pane through
  `hero_on_left_right_pane` / `pill_bar_areas`, so Wide has **no pill row**.
- Narrow branch (`render_narrow_podcast`, line 177): uses
  `hero_left::pill_bar_areas(area)` and paints surname/title-bucket pills
  (line 198+).
- Component `AudiobookshelfPodcastComponent`
  (`src/app/render/components/audiobookshelf_podcast.rs:27`) owns `state`,
  `episode_filter`, `episode_selection`, `scroll`. No `re_anchor`.
  File is 663 lines — near the 800 cap; §2.7 splits it.
- Out-of-list ownership retained by the parent: selected-show episode
  workspace, episode / played filter, images, provider playback authority.

### Audiobookshelf Book

- Entry: `render_audiobookshelf_book_content`
  (`src/app/render/components/audiobookshelf_book.rs:55`).
- Wide branch (line 83+): `library_arrangement::wide_library_panes(area, 0,
  PANE_PAD_Y)` splits panes; left = hero + chapters, right = `render_book_browser`
  (line ~142, via `padded_rect(right_pane.list_panel, PANE_PAD_X, PANE_PAD_Y)`).
- `render_book_browser` contains the `InlineDisplayRow::Replacement` case that
  paints a hero + chapter detail inline in the RIGHT rail — this is Narrow logic
  reused in Wide. §2.3 removes it for Wide: the Wide Book contract becomes the
  persistent provider detail workspace on the LEFT and ordinary fixed-height
  one-column rows on the RIGHT, no Wide selected-row replacement, no Inline hero
  in the right rail.
- Left framing bug: `padded_rect(left_area, PANE_PAD_X, 0)` (line 88) — should be
  `PANE_PAD_Y` (as Music does). §2.4 corrects it through the shared hero-on-left
  policy.
- Component `AudiobookshelfBookComponent`
  (`src/app/render/components/audiobookshelf_book.rs:18`) owns `state`,
  `chapter_selection`, `selected_bucket`, `browser_offset`, `chapters_visible`;
  surname-bucket re-anchor at lines ~87-98. No `ViewportAnchor` hand-off.
- Parent-owned: book detail, chapter / audio-file authority, images, surname
  buckets, absolute chapter seek intents.

### Shared modules the three destinations will touch later

`src/app/render/components/media_list.rs`,
`src/app/render/arrangements/hero_left.rs`,
`src/app/render/arrangements/library.rs`,
`src/app/components/media_list/` (`wide.rs`, `inline.rs`, `anchor.rs`, `mod.rs`).

## Out of scope (explicit)

- The **Emby podcast channel list** — already composes canonically through the
  #623 feed-picker dedup.
- The **Emby homevideos feed view** — distinct from the Feeds Service; unchanged.
- The **Feeds Service** itself.
- **#623 / #634 / #637**.
- Episodes, chapters, tracks, and the provider workspaces (show/book detail,
  episode/chapter/track authority, playback) — parent-owned, not canonicalized.
- Mouse / `HitRegions` / `*HitRegion` / `MouseGestureState` — deferred to #638,
  which lands after every canonical slice.

## 1.3 Grouped Music re-anchor behavior (pre-replacement reference)

Captured by the characterization test
`src/app/render/tests_music_wide_reanchor_characterization.rs`
(`grouped_music_wide_reanchor_characterization`), green at `819dbd0c`. It is the
reference the §2.1 replacement is checked against. Observed behavior:

- **Wide**, local `album_cursor` move (down ×4, App resting cursor left at 0):
  `MusicWorkspaceComponent::album_cursor()` diverges to 4; the wide right rail
  publishes exactly one `LibraryRowTarget::Album(4)` in `left_row_targets`; the
  selected screen-row offset (index of that target, and
  `selected_item_rect.y - wide_music_browser_area.y`) is 5 — one artist-header
  row plus `Album(0..=3)`.
- **Wide**, `End`: `album_cursor` jumps to the last album; the painter settles a
  non-zero `album_scroll` (`final_scroll` write-back) so the selected row is the
  bottom visible row.
- **Wide → Narrow**: `is_wide_music_active()` flips false; `album_cursor` and the
  selected target are unchanged (no re-anchor on a bare presentation flip); the
  narrow hero paints the selected album and still publishes one `Album(last)`
  target.
- **Narrow → Wide**: `album_cursor` unchanged; `album_scroll` is recomputed to
  the identical bottom-anchored offset; one `Album(last)` target.
- **Shell re-anchor** (`music_workspace_reanchor = true` +
  `push_music_workspace_content`): `album_cursor` and `album_scroll` both reset
  to the shell resting position (0, 0), regardless of the prior local move.
